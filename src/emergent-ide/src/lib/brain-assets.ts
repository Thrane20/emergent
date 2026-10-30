import "server-only";
import { createHash, randomUUID } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse } from "smol-toml";
import { nodeCatalogue, parseBrain, serializeBrain, type BrainDocument, type BrainSummary, type NodeCatalogue } from "./brains";
import type { ProjectSnapshot } from "./project";
import { gameTool } from "./scene-preview";
import { applyChanges } from "./statement-files";

export const BRAIN_DIRECTORY = "assets/brains";
const catalogues = new Map<string, Promise<NodeCatalogue>>();

/** Node catalogue from the linked game binary (the engine's single source). */
export function brainCatalogue(project: ProjectSnapshot) {
  // shortcut: cached for the IDE process; the catalogue only changes with the engine, which needs an IDE restart to rebuild anyway.
  let cached = catalogues.get(project.root);
  if (!cached) { cached = gameTool(project, ["--export-brain-nodes"]).then(text => nodeCatalogue(parse(text))); catalogues.set(project.root, cached); cached.catch(() => catalogues.delete(project.root)); }
  return cached;
}

export const brainRevision = (source: string) => createHash("sha256").update(source).digest("hex");

export function listBrains(project: ProjectSnapshot): BrainSummary[] {
  return project.files.filter(file => file.path.endsWith(".brain.toml") && project.assetRoots.some(root => file.path.startsWith(`${root}/`))).map(file => {
    try { const brain = parseBrain(file.content ?? ""); return { path: file.path, id: brain.id, name: brain.name, description: brain.description, nodes: brain.nodes.length }; }
    catch (error) { return { path: file.path, id: "", name: file.name, description: "", nodes: 0, error: error instanceof Error ? error.message : "Invalid brain." }; }
  }).sort((a, b) => a.name.localeCompare(b.name));
}

export function readBrain(project: ProjectSnapshot, brainPath: string) {
  const file = project.files.find(item => item.path === brainPath && item.path.endsWith(".brain.toml"));
  if (!file?.content) throw new Error("Choose a brain in this project.");
  return { path: file.path, revision: brainRevision(file.content), brain: parseBrain(file.content) };
}

function checkedDocument(value: unknown): BrainDocument {
  const brain = value as BrainDocument;
  if (!brain || typeof brain.name !== "string" || !brain.name.trim() || brain.name.length > 64 || typeof brain.description !== "string" || brain.description.length > 500 || !Array.isArray(brain.nodes) || brain.nodes.length > 256) throw new Error("A brain needs a name (1-64 characters), a description up to 500 characters and at most 256 nodes.");
  // Round-trip so only well-formed TOML values reach Rust validation.
  return parseBrain(serializeBrain({ ...brain, id: typeof brain.id === "string" && brain.id ? brain.id : randomUUID() }));
}

async function withCandidate<T>(source: string, run: (file: string) => Promise<T>) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-brain-"));
  try { const file = path.join(directory, "candidate.brain.toml"); await writeFile(file, source); return await run(file); }
  finally { await rm(directory, { recursive: true, force: true }); }
}

/** Validates with Rust, then publishes atomically; `revision` guards against overwriting newer edits. */
export async function saveBrain(project: ProjectSnapshot, input: { path?: string; revision?: string; brain: unknown }) {
  const brain = checkedDocument(input.brain);
  const target = input.path ?? `${BRAIN_DIRECTORY}/${brain.id}.brain.toml`;
  if (!target.endsWith(".brain.toml") || !project.assetRoots.some(root => target.startsWith(`${root}/`)) || target.split("/").some(part => part === ".." || !part)) throw new Error("Brains live under a project asset root.");
  const existing = project.files.find(file => file.path === target);
  if (input.path && (!existing?.content || brainRevision(existing.content) !== input.revision)) throw new Error("This brain changed on disk since you opened it. Reload it before saving.");
  if (!input.path && existing) throw new Error("A brain with this ID already exists.");
  const content = serializeBrain(brain);
  await withCandidate(content, file => gameTool(project, ["--validate-brain", target, "--candidate", file]));
  await applyChanges(project, [{ path: target, before: existing?.content ?? null, content }]);
  return { path: target, revision: brainRevision(content), brain };
}

export type PreviewOptions = { agents: number; targets: number; targetSpeed: number; seconds: number; agentGroup: string; targetGroup: string; maxSpeed: number; seed: number };
const groupName = (value: unknown, fallback: string) => typeof value === "string" && /^[\w .-]{1,64}$/.test(value) ? value.trim() : fallback;

/** Runs the (unsaved) brain in the Rust runtime and returns the raw trace text. */
export async function previewBrain(project: ProjectSnapshot, brainPath: string | undefined, document: unknown, options: Partial<PreviewOptions>) {
  const brain = checkedDocument(document);
  const target = brainPath ?? `${BRAIN_DIRECTORY}/${brain.id}.brain.toml`;
  const clamp = (value: unknown, min: number, max: number, fallback: number) => typeof value === "number" && Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : fallback;
  const args = [
    `agents=${Math.round(clamp(options.agents, 1, 256, 20))}`, `targets=${Math.round(clamp(options.targets, 0, 32, 1))}`, `target_speed=${clamp(options.targetSpeed, 0, 20, 3)}`,
    `seconds=${clamp(options.seconds, 1, 30, 20)}`, `max_speed=${clamp(options.maxSpeed, 0.1, 50, 3)}`, `seed=${Math.round(clamp(options.seed, 1, 1e9, 1))}`,
    `agent_group=${groupName(options.agentGroup, "agents")}`, `target_group=${groupName(options.targetGroup, "target")}`,
  ];
  return withCandidate(serializeBrain(brain), file => gameTool(project, ["--simulate-brain", target, "--candidate", file, ...args], 32 * 1024 * 1024));
}
