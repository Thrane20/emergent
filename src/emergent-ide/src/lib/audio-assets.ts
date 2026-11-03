import "server-only";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, mkdtemp, readFile, readdir, rm, rmdir, stat, unlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse } from "smol-toml";
import { containedFile, ideDataDirectory } from "./object-files";
import { createProjectAsset } from "./project-assets";
import { publishProjectEvent } from "./project-events";
import { projectIndex } from "./project-index";
import { placeAsset, setObjectComponent } from "./project-ops";
import { scenePlacement } from "./scene-placement";
import { gameTool } from "./scene-preview";
import { readProjectSnapshot, type ProjectSnapshot } from "./project";
import { AUDIO_CATEGORIES, AUDIO_DIRECTORY, MAX_AUDIO_BYTES, audioExtension, audioSettings, type AudioCategory, type AudioClipSettings, type AudioClipSummary, type AudioInfo } from "./audio-types";

export const clipRevision = (source: string) => createHash("sha256").update(source).digest("hex");
const clipFiles = (project: ProjectSnapshot) => project.files.filter(file => file.path.endsWith(".audio.toml") && project.assetRoots.some(root => file.path.startsWith(`${root}/`)));
/** Scenes and prefabs that name this clip. */
const usersOf = (project: ProjectSnapshot, clip: string) => project.files.filter(file => /\.(scene|prefab)\.toml$/.test(file.path) && file.content?.includes(clip)).map(file => file.path);

/** TOML tables arrive as parser objects; pages and JSON need plain text, numbers and booleans. */
const plain = (table: Record<string, unknown>) => Object.fromEntries(Object.entries(table).map(([key, value]) => [key, typeof value === "number" || typeof value === "boolean" ? value : String(value)]));

function summary(project: ProjectSnapshot, file: { path: string; content: string | null }): AudioClipSummary {
  try {
    const value = parse(file.content ?? "") as Record<string, unknown>;
    const source = String(value.source ?? "");
    return { path: file.path, id: String(value.id ?? ""), revision: clipRevision(file.content ?? ""), source, sourcePath: path.posix.join(path.posix.dirname(file.path), source), settings: audioSettings(value), ...(value.generation && typeof value.generation === "object" ? { generation: plain(value.generation as Record<string, unknown>) } : {}), usedBy: usersOf(project, file.path) };
  } catch (error) {
    return { path: file.path, id: "", revision: clipRevision(file.content ?? ""), source: "", sourcePath: "", settings: audioSettings({ name: path.posix.basename(file.path, ".audio.toml") }), usedBy: [], error: error instanceof Error ? error.message : "Invalid clip." };
  }
}

export function listClips(project: ProjectSnapshot) {
  return clipFiles(project).map(file => summary(project, file)).sort((a, b) => a.settings.name.localeCompare(b.settings.name));
}

export function readClip(project: ProjectSnapshot, clipPath: string) {
  const file = clipFiles(project).find(item => item.path === clipPath);
  if (!file?.content) throw new Error("Choose an audio clip in this project.");
  return summary(project, file);
}

const number = (n: number) => Number.isInteger(n) ? `${n}.0` : String(Number(n.toFixed(4)));
const range = (pair: [number, number]) => `[${number(pair[0])}, ${number(pair[1])}]`;
const quoted = (text: string) => JSON.stringify(text);
function tomlValue(value: unknown): string {
  if (typeof value === "string") return quoted(value);
  if (typeof value === "number") return Number.isFinite(value) ? String(value) : "0";
  if (typeof value === "boolean") return String(value);
  throw new Error("Generation provenance holds only text, numbers and booleans.");
}

/** Writes the clip file in a stable field order. Provenance is kept as written. */
export function serializeClip(id: string, source: string, settings: AudioClipSettings, generation?: Record<string, unknown>) {
  const lines = ["schema_version = 1", `id = ${quoted(id)}`, `name = ${quoted(settings.name)}`, 'kind = "audio"', `description = ${quoted(settings.description)}`, `source = ${quoted(source)}`, `category = ${quoted(settings.category)}`, `gain_db = ${number(settings.gain_db)}`];
  if (settings.trim) lines.push(`trim = ${range(settings.trim)}`);
  if (settings.loop) lines.push(`loop = ${range(settings.loop)}`);
  if (settings.fade_in) lines.push(`fade_in = ${number(settings.fade_in)}`);
  if (settings.fade_out) lines.push(`fade_out = ${number(settings.fade_out)}`);
  if (settings.stream !== null) lines.push(`stream = ${settings.stream}`);
  if (generation && Object.keys(generation).length) lines.push("", "[generation]", ...Object.entries(generation).filter(([key]) => /^[a-z_]{1,32}$/.test(key)).map(([key, value]) => `${key} = ${tomlValue(value)}`));
  return `${lines.join("\n")}\n`;
}

/** Validates and decodes through the engine (`--inspect-audio`), so the IDE reports what the game will play. */
export async function inspectClip(project: ProjectSnapshot, clipPath: string, candidate?: string): Promise<AudioInfo> {
  let directory: string | undefined;
  try {
    const args = ["--inspect-audio", clipPath];
    if (candidate !== undefined) { directory = await mkdtemp(path.join(os.tmpdir(), "emergent-audio-")); const file = path.join(directory, "candidate.audio.toml"); await writeFile(file, candidate); args.push("--candidate", file); }
    const info = parse(await gameTool(project, args)) as Record<string, unknown>;
    const waveform = Array.isArray(info.waveform) ? info.waveform.map(Number).filter(Number.isFinite) : [];
    return { channels: Number(info.channels), sample_rate: Number(info.sample_rate), frames: Number(info.frames), duration_seconds: Number(info.duration_seconds), peak: Number(info.peak), waveform };
  } finally { if (directory) await rm(directory, { recursive: true, force: true }); }
}

/** Waveform cache key: the clip file and its audio file's size and time, so any edit to either misses. */
async function waveformKey(project: ProjectSnapshot, clipPath: string) {
  const clipFile = await containedFile(project.root, clipPath, 64 * 1024), content = await readFile(clipFile, "utf8");
  const source = String((parse(content) as Record<string, unknown>).source ?? "");
  const audio = await stat(path.join(path.dirname(clipFile), path.basename(source))).catch(() => null);
  return createHash("sha256").update(`${clipPath}\0${content}\0${audio?.size}\0${audio?.mtimeMs}`).digest("hex");
}
const waveformFile = (key: string) => path.join(ideDataDirectory(), "audio-waveforms", `${key}.json`);
async function rememberWaveform(project: ProjectSnapshot, clipPath: string, info: AudioInfo) {
  const file = waveformFile(await waveformKey(project, clipPath));
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, JSON.stringify(info.waveform));
}
// One engine run at a time: a page of tiles would otherwise start a cargo run per clip at once.
let waveformQueue: Promise<unknown> = Promise.resolve();
/** The engine's waveform outline of a saved clip, from cache or by inspecting it once. */
export async function clipWaveform(project: ProjectSnapshot, clipPath: string): Promise<number[]> {
  readClip(project, clipPath);
  const file = waveformFile(await waveformKey(project, clipPath));
  try { return JSON.parse(await readFile(file, "utf8")); } catch { /* not cached yet */ }
  const next = waveformQueue.catch(() => {}).then(async () => {
    try { return JSON.parse(await readFile(file, "utf8")) as number[]; } catch { /* still missing */ }
    const info = await inspectClip(project, clipPath);
    await rememberWaveform(project, clipPath, info);
    return info.waveform;
  });
  waveformQueue = next;
  return next;
}

function changed(project: ProjectSnapshot, summary: string, paths: string[]) {
  publishProjectEvent({ type: "changed", projectId: project.id, changeId: randomUUID(), source: "ide", summary, paths, scenes: [], live: false });
}

export async function saveClip(project: ProjectSnapshot, input: { path: string; revision: string; settings: unknown }) {
  const current = readClip(project, input.path), file = await containedFile(project.root, input.path, 64 * 1024);
  // Compare against the file itself: the snapshot may predate another save.
  if (clipRevision(await readFile(file, "utf8")) !== input.revision) throw new Error("This clip changed on disk. Reload it before saving.");
  if (current.error) throw new Error(`Fix the clip file first: ${current.error}`);
  const settings = audioSettings(input.settings, true);
  const source = serializeClip(current.id, current.source, settings, current.generation);
  const info = await inspectClip(project, input.path, source);
  await writeFile(file, source);
  await rememberWaveform(project, input.path, info);
  changed(project, `saved audio clip ${settings.name}`, [input.path]);
  return { ...readClip(await readProjectSnapshot(), input.path), info };
}

const slug = (name: string) => name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40) || "clip";

/** Saves audio bytes as a new clip in `assets/audio/<id>/`. The engine decodes it before the import is kept. */
export async function importClip(project: ProjectSnapshot, input: { name: string; category: AudioCategory; bytes: Buffer; generation?: Record<string, unknown>; description?: string }) {
  if (input.bytes.length > MAX_AUDIO_BYTES) throw new Error("Audio files must be 50 MiB or smaller.");
  const extension = audioExtension(input.bytes);
  if (!extension) throw new Error("Choose a WAV, MP3, OGG Vorbis or FLAC file.");
  if (!AUDIO_CATEGORIES.includes(input.category)) throw new Error("Choose a clip category.");
  const settings = audioSettings({ name: input.name, category: input.category, description: input.description ?? "" }, true);
  const id = randomUUID(), base = slug(settings.name), directory = `${AUDIO_DIRECTORY}/${id}`, clipPath = `${directory}/${base}.audio.toml`;
  if (!project.assetRoots.some(root => directory.startsWith(`${root}/`))) throw new Error("This project's asset roots do not include assets/audio.");
  const folder = path.join(project.root, directory);
  await mkdir(folder, { recursive: true });
  try {
    await writeFile(path.join(folder, `${base}.${extension}`), input.bytes, { flag: "wx" });
    await writeFile(path.join(folder, `${base}.audio.toml`), serializeClip(id, `${base}.${extension}`, settings, input.generation), { flag: "wx" });
    const info = await inspectClip(project, clipPath);
    await rememberWaveform(project, clipPath, info);
    changed(project, `imported audio clip ${settings.name}`, [clipPath]);
    return { ...readClip(await readProjectSnapshot(), clipPath), info };
  } catch (error) {
    await rm(folder, { recursive: true, force: true });
    throw error;
  }
}

/** Deletes a clip and its audio file. Refused while a scene or prefab still uses it. */
export async function deleteClip(project: ProjectSnapshot, clipPath: string) {
  const clip = readClip(project, clipPath);
  if (clip.usedBy.length) throw new Error(`This clip is still used by ${clip.usedBy.join(", ")}. Remove those audio sources first.`);
  const folder = path.dirname(await containedFile(project.root, clipPath, 64 * 1024));
  await unlink(path.join(folder, path.posix.basename(clipPath)));
  if (clip.source && !clip.source.includes("/")) await unlink(path.join(folder, clip.source)).catch(() => {});
  // Remove the clip's own folder only once nothing else is left in it.
  if (path.posix.dirname(clipPath).startsWith(`${AUDIO_DIRECTORY}/`) && !(await readdir(folder)).length) await rmdir(folder);
  changed(project, `deleted audio clip ${clip.settings.name}`, [clipPath]);
  return { path: clipPath };
}

/** Adds a Point to a scene that plays the clip from `translation`. Reuses the project's Point asset, or creates one. */
export async function placeClip(project: ProjectSnapshot, input: { clip: string; scene: string; translation: [number, number, number] }) {
  const clip = readClip(project, input.clip);
  if (!input.translation.every(n => Number.isFinite(n) && Math.abs(n) <= 1e6)) throw new Error("Choose a finite position.");
  let point = project.files.find(file => file.path.endsWith(".entity.toml") && /^template\s*=\s*"marker\.point"/m.test(file.content ?? ""))?.path;
  if (!point) { point = (await createProjectAsset(project, "marker.point", "Sound point")).path; project = await readProjectSnapshot(); }
  const index = await projectIndex(project);
  const scene = index.scenes.find(entry => entry.path === input.scene), asset = index.assets.find(entry => entry.path === point);
  if (!scene || !asset) throw new Error("Choose a scene in this project.");
  const placed = await placeAsset(project, { asset: asset.ref, scene: scene.ref, revision: (await scenePlacement(project, scene.path)).revision, name: clip.settings.name, translation: input.translation, source: "ide" });
  if (!placed.ref || !placed.revision) throw new Error("The point could not be placed.");
  const result = await setObjectComponent(await readProjectSnapshot(), { ref: placed.ref, revision: placed.revision, type: "emergent.audio_source", fields: { clip: clip.path } });
  return { ref: placed.ref, scene: scene.path, revision: result.revision };
}
