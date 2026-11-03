import "server-only";
import { createHash, randomUUID } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse } from "smol-toml";
import { locomotionCatalogue as parseCatalogue, parseProfile, serializeProfile, type LocomotionCatalogue, type LocomotionProfile, type LocomotionSummary } from "./locomotion";
import type { ProjectSnapshot } from "./project";
import { gameTool } from "./scene-preview";
import { applyChanges } from "./statement-files";

export const LOCOMOTION_DIRECTORY = "assets/locomotion";
const catalogues = new Map<string, Promise<LocomotionCatalogue>>();

/** Type catalogue from the linked game binary (the engine's single source). */
export function locomotionCatalogue(project: ProjectSnapshot) {
  // shortcut: cached for the IDE process like the brain catalogue; restart the IDE after engine changes.
  let cached = catalogues.get(project.root);
  if (!cached) { cached = gameTool(project, ["--export-locomotion"]).then(text => parseCatalogue(parse(text))); catalogues.set(project.root, cached); cached.catch(() => catalogues.delete(project.root)); }
  return cached;
}

export const profileRevision = (source: string) => createHash("sha256").update(source).digest("hex");

export function listProfiles(project: ProjectSnapshot): LocomotionSummary[] {
  return project.files.filter(file => file.path.endsWith(".locomotion.toml") && project.assetRoots.some(root => file.path.startsWith(`${root}/`))).map(file => {
    try { const profile = parseProfile(file.content ?? ""); return { path: file.path, id: profile.id, name: profile.name, description: profile.description, type: profile.type }; }
    catch (error) { return { path: file.path, id: "", name: file.name, description: "", type: "", error: error instanceof Error ? error.message : "Invalid profile." }; }
  }).sort((a, b) => a.name.localeCompare(b.name));
}

export function readProfile(project: ProjectSnapshot, profilePath: string) {
  const file = project.files.find(item => item.path === profilePath && item.path.endsWith(".locomotion.toml"));
  if (!file?.content) throw new Error("Choose a locomotion profile in this project.");
  return { path: file.path, revision: profileRevision(file.content), profile: parseProfile(file.content) };
}

function checked(value: unknown): LocomotionProfile {
  const profile = value as LocomotionProfile;
  if (!profile || typeof profile.name !== "string" || !profile.name.trim() || profile.name.length > 64 || typeof profile.description !== "string" || profile.description.length > 500 || typeof profile.type !== "string" || !profile.fields || typeof profile.fields !== "object") throw new Error("A profile needs a name (1-64 characters), a type, fields and a description up to 500 characters.");
  return parseProfile(serializeProfile({ ...profile, id: typeof profile.id === "string" && profile.id ? profile.id : randomUUID() }));
}

async function withCandidate<T>(source: string, run: (file: string) => Promise<T>) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-locomotion-"));
  try { const file = path.join(directory, "candidate.locomotion.toml"); await writeFile(file, source); return await run(file); }
  finally { await rm(directory, { recursive: true, force: true }); }
}

/** Validates with Rust, then publishes atomically; `revision` guards against overwriting newer edits. */
export async function saveProfile(project: ProjectSnapshot, input: { path?: string; revision?: string; profile: unknown }) {
  const profile = checked(input.profile);
  const target = input.path ?? `${LOCOMOTION_DIRECTORY}/${profile.id}.locomotion.toml`;
  if (!target.endsWith(".locomotion.toml") || !project.assetRoots.some(root => target.startsWith(`${root}/`)) || target.split("/").some(part => part === ".." || !part)) throw new Error("Locomotion profiles live under a project asset root.");
  const existing = project.files.find(file => file.path === target);
  if (input.path && (!existing?.content || profileRevision(existing.content) !== input.revision)) throw new Error("This profile changed on disk since you opened it. Reload it before saving.");
  if (!input.path && existing) throw new Error("A profile with this ID already exists.");
  const content = serializeProfile(profile);
  await withCandidate(content, file => gameTool(project, ["--validate-locomotion", target, "--candidate", file]));
  await applyChanges(project, [{ path: target, before: existing?.content ?? null, content }]);
  return { path: target, revision: profileRevision(content), profile };
}

const COURSES = ["waypoints", "orbit", "stop", "launch", "circuit"];
/** Runs the (unsaved) profile along a course in the Rust runtime; returns the raw trace text. */
export async function previewProfile(project: ProjectSnapshot, profilePath: string | undefined, document: unknown, options: { course?: unknown; seconds?: unknown }) {
  const profile = checked(document);
  const target = profilePath ?? `${LOCOMOTION_DIRECTORY}/${profile.id}.locomotion.toml`;
  // Physical aircraft fly whole circuits (several minutes); everything else previews up to a minute.
  const longest = profile.type === "fixed_wing_physics" ? 300 : 60;
  const seconds = typeof options.seconds === "number" && Number.isFinite(options.seconds) ? Math.min(longest, Math.max(1, options.seconds)) : 20;
  const args = [`seconds=${seconds}`, ...(typeof options.course === "string" && COURSES.includes(options.course) ? [`course=${options.course}`] : [])];
  return withCandidate(serializeProfile(profile), file => gameTool(project, ["--simulate-locomotion", target, "--candidate", file, ...args], 16 * 1024 * 1024));
}
