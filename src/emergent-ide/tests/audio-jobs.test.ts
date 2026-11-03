import assert from "node:assert/strict";
import test from "node:test";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse, stringify } from "smol-toml";
import { AudioJobs, generateAudioClip, type AudioProvider } from "../src/lib/audio-jobs";
import { listClips } from "../src/lib/audio-assets";
import { audioModelAdapter, audioModelInput } from "../src/lib/fal-models";
import { readProjectSnapshot } from "../src/lib/project";

type Props = Record<string, unknown>;
function model(input: Props, output: Props, category = "text-to-audio", required: string[] = []) {
  const id = "fal-ai/test/audio";
  return { endpoint_id: id, metadata: { category, status: "active" }, openapi: {
    components: { schemas: { Input: { type: "object", properties: input, required }, Output: { type: "object", properties: output }, File: { type: "object", properties: { url: { type: "string" } } } } },
    paths: { [`/${id}`]: { post: { requestBody: { content: { "application/json": { schema: { $ref: "#/components/schemas/Input" } } } } } }, [`/${id}/requests/{request_id}`]: { get: { responses: { "200": { content: { "application/json": { schema: { $ref: "#/components/schemas/Output" } } } } } } } },
  } } as unknown as Parameters<typeof audioModelAdapter>[0];
}
const audioOut = { audio: { $ref: "#/components/schemas/File" } };

test("audio models are adapted from their schemas or refused", () => {
  const adapter = audioModelAdapter(model({ prompt: { type: "string" }, seconds_total: { type: "integer", minimum: 1, maximum: 10 }, seed: { type: "integer" }, output_format: { type: "string", enum: ["hex", "url"] } }, audioOut), "music");
  assert.deepEqual(audioModelInput(adapter, { text: "drums", seed: 7, duration: 30 }), { prompt: "drums", seed: 7, seconds_total: 10, output_format: "url" });
  const ms = audioModelAdapter(model({ text: { type: "string" }, music_length_ms: { anyOf: [{ type: "integer" }, { type: "null" }] }, voice: { type: "string" } }, audioOut), "speech");
  assert.deepEqual(audioModelInput(ms, { text: "hi", seed: 1, duration: 12.3, voice: "Aria" }), { text: "hi", music_length_ms: 12300, voice: "Aria" });
  assert.throws(() => audioModelAdapter(model({ prompt: { type: "string" } }, { video: { $ref: "#/components/schemas/File" } }), "sound"), /audio file/);
  assert.throws(() => audioModelAdapter(model({ prompt: { type: "string" }, reference_audio_url: { type: "string" } }, audioOut, "text-to-audio", ["prompt", "reference_audio_url"]), "sound"), /reference_audio_url/);
  assert.throws(() => audioModelAdapter(model({ prompt: { type: "string" } }, audioOut, "text-to-image"), "sound"), /not an active audio/);
});

test("prefixed schema paths and nested voice settings are adapted", () => {
  const nested = model({ prompt: { type: "string" }, voice_setting: { $ref: "#/components/schemas/Voice" }, output_format: { type: "string", enum: ["hex", "url"] } }, audioOut, "text-to-speech");
  (nested.openapi!.components!.schemas as Record<string, unknown>).Voice = { type: "object", properties: { voice_id: { type: "string", examples: ["Wise_Woman", "Deep_Voice_Man"] }, speed: { type: "number", default: 1 } } };
  const adapter = audioModelAdapter(nested, "speech");
  assert.deepEqual(adapter.voices, ["Wise_Woman", "Deep_Voice_Man"]);
  assert.deepEqual(audioModelInput(adapter, { text: "Halt", seed: 1, voice: "Deep_Voice_Man" }), { prompt: "Halt", voice_setting: { voice_id: "Deep_Voice_Man" }, output_format: "url" });
  // The catalogue ID lacks the "fal-ai/" owner that its schema paths carry.
  const prefixed = model({ text: { type: "string" }, voice: { type: "string", enum: ["Aria"] } }, audioOut, "text-to-speech");
  const paths = prefixed.openapi!.paths as Record<string, unknown>;
  for (const key of Object.keys(paths)) { paths[key.replace("/fal-ai/test/audio", "/fal-ai/fal-ai/test/audio")] = paths[key]; delete paths[key]; }
  prefixed.endpoint_id = "fal-ai/test/audio";
  assert.deepEqual(audioModelAdapter(prefixed, "speech").voices, ["Aria"]);
});

function wav(seconds: number, rate = 8000) {
  const frames = Math.round(seconds * rate), data = Buffer.alloc(frames * 2);
  for (let i = 0; i < frames; i++) data.writeInt16LE(Math.round(Math.sin(i / rate * 440 * Math.PI * 2) * 12000), i * 2);
  const header = Buffer.alloc(44);
  header.write("RIFF", 0); header.writeUInt32LE(36 + data.length, 4); header.write("WAVEfmt ", 8); header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20); header.writeUInt16LE(1, 22); header.writeUInt32LE(rate, 24); header.writeUInt32LE(rate * 2, 28);
  header.writeUInt16LE(2, 32); header.writeUInt16LE(16, 34); header.write("data", 36); header.writeUInt32LE(data.length, 40);
  return Buffer.concat([header, data]);
}

test("a generated sound submits once, downloads, and is kept as a clip with provenance", { timeout: 300000 }, async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-audio-jobs-"));
  const root = path.join(directory, "game"), foundation = path.resolve(process.cwd(), "../..");
  const previousRoot = process.env.EMERGENT_PROJECT_PATH;
  t.after(async () => { if (previousRoot === undefined) delete process.env.EMERGENT_PROJECT_PATH; else process.env.EMERGENT_PROJECT_PATH = previousRoot; await rm(directory, { recursive: true, force: true }); });
  await cp(path.join(foundation, "templates/standard"), root, { recursive: true });
  const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8"));
  (cargo.dependencies as Record<string, { path: string }>).emergent.path = foundation;
  cargo.workspace = {};
  // Its own package name gives this fixture its own binary in the shared target directory, so parallel test files never run each other's build.
  (cargo.package as Record<string, unknown>).name = "emergent-audio-jobs-fixture";
  await writeFile(path.join(root, "Cargo.toml"), stringify(cargo));
  await cp(path.join(foundation, "Cargo.lock"), path.join(root, "Cargo.lock"));
  process.env.EMERGENT_PROJECT_PATH = root;
  const project = await readProjectSnapshot();

  const submitted: { endpoint: string; input: Record<string, unknown> }[] = [];
  let ready = false;
  const provider: AudioProvider = {
    async submit(endpoint, input) { submitted.push({ endpoint, input }); return "request-1"; },
    async status() { return ready ? "ready" : "generating"; },
    async result(_endpoint, _id, field) { assert.equal(field, "audio"); return wav(0.5); },
  };
  const jobs = new AudioJobs(path.join(directory, "data"), async () => provider, async () => { throw new Error("defaults need no catalogue"); });
  const id = "6f1c2c0e-9b7a-4c1d-8e2f-0a1b2c3d4e5f", request = { mode: "sound", text: "Door creak", seed: 4, duration: 30, loop: true };
  await assert.rejects(jobs.create(project, id, request), /duration of 0.5-22/);
  const job = await jobs.create(project, id, { ...request, duration: 2 });
  assert.equal(job.state, "queued");
  assert.deepEqual(submitted, [{ endpoint: "fal-ai/elevenlabs/sound-effects/v2", input: { text: "Door creak", duration_seconds: 2, loop: true } }]);
  await jobs.create(project, id, { ...request, duration: 2 });
  assert.equal(submitted.length, 1, "a repeated request never submits twice");
  await assert.rejects(jobs.create(project, id, { ...request, text: "Other", duration: 2 }), /another request/);

  assert.equal((await jobs.refresh(project, id)).state, "generating");
  await assert.rejects(jobs.keep(project, id, "Door"), /not ready/);
  ready = true;
  const done = await jobs.refresh(project, id);
  assert.equal(done.state, "ready");
  assert.equal(done.file, "candidate.wav");

  const { clip } = await jobs.keep(project, id, "Door creak");
  assert.equal(clip.settings.category, "sfx");
  assert.equal(clip.info?.duration_seconds, 0.5);
  const text = await readFile(path.join(root, clip.path), "utf8");
  assert.match(text, /\[generation\]\nmodel = "fal-ai\/elevenlabs\/sound-effects\/v2"/);
  assert.match(text, /prompt = "Door creak"/);
  assert.doesNotMatch(text, /seed =/, "the default model takes no seed, so none is recorded");
  const listed = listClips(await readProjectSnapshot());
  assert.equal(listed.length, 1);
  // Pages receive this summary, so provenance must be a plain object.
  assert.equal(Object.getPrototypeOf(listed[0].generation), Object.prototype);
  assert.equal(listed[0].generation?.prompt, "Door creak");
  await assert.rejects(jobs.keep(project, id, "Again"), /Already saved/);
  await jobs.discard(project, id);
  assert.equal((await jobs.list(project)).length, 0);

  // The MCP tool refuses without explicit consent to pay, then generates, waits and keeps.
  await assert.rejects(generateAudioClip(project, { mode: "sound", text: "Hit", name: "Hit" }, { jobs }), /paid fal request/);
  assert.equal(submitted.length, 1, "nothing was submitted without consent");
  const made = await generateAudioClip(project, { mode: "speech", text: "Halt there", name: "Guard halt", voice: "Roger", confirm_paid: true }, { jobs, pollMs: 0 });
  assert.equal(made.generated, true);
  assert.match(String(made.ref), /^asset:[0-9a-f-]{36}$/);
  assert.equal(made.category, "voice");
  assert.deepEqual(submitted[1], { endpoint: "fal-ai/elevenlabs/tts/eleven-v3", input: { text: "Halt there", voice: "Roger" } });
  ready = false;
  const slow = await generateAudioClip(project, { mode: "sound", text: "Slow one", name: "Slow", duration: 1, confirm_paid: true }, { jobs, pollMs: 0, timeoutMs: 0 });
  assert.equal(slow.generated, false);
});
