import assert from "node:assert/strict";
import test from "node:test";
import { cp, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse, stringify } from "smol-toml";
import { clipWaveform, deleteClip, importClip, listClips, placeClip, readClip, saveClip } from "../src/lib/audio-assets";
import { audioExtension, audioSettings } from "../src/lib/audio-types";
import { readProjectSnapshot } from "../src/lib/project";

/** A 16-bit mono WAV of a 440 Hz tone. */
function wav(seconds: number, rate = 8000) {
  const frames = Math.round(seconds * rate), data = Buffer.alloc(frames * 2);
  for (let i = 0; i < frames; i++) data.writeInt16LE(Math.round(Math.sin(i / rate * 440 * Math.PI * 2) * 12000), i * 2);
  const header = Buffer.alloc(44);
  header.write("RIFF", 0); header.writeUInt32LE(36 + data.length, 4); header.write("WAVEfmt ", 8); header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20); header.writeUInt16LE(1, 22); header.writeUInt32LE(rate, 24); header.writeUInt32LE(rate * 2, 28);
  header.writeUInt16LE(2, 32); header.writeUInt16LE(16, 34); header.write("data", 36); header.writeUInt32LE(data.length, 40);
  return Buffer.concat([header, data]);
}

test("audio containers are sniffed and clip settings validated", () => {
  assert.equal(audioExtension(wav(0.01)), "wav");
  assert.equal(audioExtension(Buffer.from("OggS\0\0\0\0")), "ogg");
  assert.equal(audioExtension(Buffer.from("fLaC\0\0\0\0")), "flac");
  assert.equal(audioExtension(Buffer.from([0x49, 0x44, 0x33, 4, 0])), "mp3");
  assert.equal(audioExtension(Buffer.from("<html>")), null);
  assert.deepEqual(audioSettings({ name: "Hum", trim: [0, 1] }, true).trim, [0, 1]);
  for (const [value, message] of [[{ name: "" }, /Name/], [{ name: "A", gain_db: 40 }, /Gain/], [{ name: "A", trim: [1, 0.5] }, /Trim/], [{ name: "A", trim: [0, 1], loop: [0, 2] }, /loop/], [{ name: "A", category: "noise" }, /sfx/]] as const)
    assert.throws(() => audioSettings(value, true), message);
});

test("clips import, save, place and delete through engine validation", { timeout: 300000 }, async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-audio-test-"));
  const root = path.join(directory, "game"), foundation = path.resolve(process.cwd(), "../..");
  const previousRoot = process.env.EMERGENT_PROJECT_PATH, previousData = process.env.EMERGENT_IDE_DATA_DIR;
  t.after(async () => {
    if (previousRoot === undefined) delete process.env.EMERGENT_PROJECT_PATH; else process.env.EMERGENT_PROJECT_PATH = previousRoot;
    if (previousData === undefined) delete process.env.EMERGENT_IDE_DATA_DIR; else process.env.EMERGENT_IDE_DATA_DIR = previousData;
    await rm(directory, { recursive: true, force: true });
  });
  await cp(path.join(foundation, "templates/standard"), root, { recursive: true });
  const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8"));
  (cargo.dependencies as Record<string, { path: string }>).emergent.path = foundation;
  cargo.workspace = {};
  // Its own package name gives this fixture its own binary in the shared target directory, so parallel test files never run each other's build.
  (cargo.package as Record<string, unknown>).name = "emergent-audio-assets-fixture";
  await writeFile(path.join(root, "Cargo.toml"), stringify(cargo));
  await cp(path.join(foundation, "Cargo.lock"), path.join(root, "Cargo.lock"));
  process.env.EMERGENT_PROJECT_PATH = root; process.env.EMERGENT_IDE_DATA_DIR = path.join(directory, "data");
  let project = await readProjectSnapshot();

  await assert.rejects(importClip(project, { name: "Page", category: "sfx", bytes: Buffer.from("<html></html>") }), /WAV, MP3/);
  // Bytes that look like a WAV but don't decode are rejected by the engine, and nothing is left behind.
  await assert.rejects(importClip(project, { name: "Broken", category: "sfx", bytes: Buffer.concat([Buffer.from("RIFF\0\0\0\0WAVE"), Buffer.alloc(32)]) }));
  project = await readProjectSnapshot();
  assert.equal(listClips(project).length, 0);

  const imported = await importClip(project, { name: "Engine Hum", category: "sfx", bytes: wav(1) });
  assert.equal(imported.info?.duration_seconds, 1);
  assert.equal(imported.info?.waveform.length, 64);
  assert.ok(imported.info!.waveform.every(level => level > 0.3 && level <= 0.4), "a steady tone has a flat outline");
  assert.match(imported.path, /^assets\/audio\/[0-9a-f-]{36}\/engine-hum\.audio\.toml$/);
  project = await readProjectSnapshot();
  const clip = readClip(project, imported.path);

  const saved = await saveClip(project, { path: clip.path, revision: clip.revision, settings: { ...clip.settings, name: "Hum", category: "music", trim: [0.25, 0.75], loop: [0, 0.5], fade_in: 0.05 } });
  assert.equal(saved.info?.duration_seconds, 0.5);
  // Saving re-keys the cached outline: the trimmed, faded clip starts from silence.
  const outline = await clipWaveform(await readProjectSnapshot(), clip.path);
  assert.deepEqual(outline, saved.info?.waveform);
  assert.ok(outline[0] < outline[32], "fade in shows at the start");
  assert.equal(saved.settings.category, "music");
  const text = await readFile(path.join(root, clip.path), "utf8");
  assert.match(text, /trim = \[0\.25, 0\.75\]/);
  assert.match(text, /source = "engine-hum\.wav"/);
  assert.doesNotMatch(text, /stream =/, "auto streaming is not written");
  await assert.rejects(saveClip(project, { path: clip.path, revision: clip.revision, settings: clip.settings }), /changed on disk/);
  project = await readProjectSnapshot();
  const fresh = readClip(project, clip.path);
  await assert.rejects(saveClip(project, { path: clip.path, revision: fresh.revision, settings: { ...fresh.settings, trim: [0, 5] } }));
  assert.equal(await readFile(path.join(root, clip.path), "utf8"), text, "a rejected save leaves the file untouched");

  const placed = await placeClip(project, { clip: clip.path, scene: "assets/scenes/main.scene.toml", translation: [2, 1, 0] });
  assert.match(placed.ref, /^instance:/);
  const scene = await readFile(path.join(root, "assets/scenes/main.scene.toml"), "utf8");
  assert.match(scene, /type = "emergent.audio_source"/);
  assert.ok(scene.includes(clip.path));
  project = await readProjectSnapshot();
  assert.deepEqual(readClip(project, clip.path).usedBy, ["assets/scenes/main.scene.toml"]);
  await assert.rejects(deleteClip(project, clip.path), /still used/);

  await writeFile(path.join(root, "assets/scenes/main.scene.toml"), scene.replace(/\n\[\[entities\]\][^[]*Hum[\s\S]*$/, "\n"));
  project = await readProjectSnapshot();
  if (readClip(project, clip.path).usedBy.length === 0) {
    await deleteClip(project, clip.path);
    await assert.rejects(stat(path.join(root, path.dirname(clip.path))), /ENOENT/);
  }
});
