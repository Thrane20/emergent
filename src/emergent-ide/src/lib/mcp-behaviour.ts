import "server-only";
// MCP behaviour authoring: brains, locomotion profiles and their engine previews, with the same guarantees as
// scene edits (Rust validation before writing, revision guard, dry run, journal for history/undo, IDE refresh).
import { randomUUID } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { brainCatalogue, brainRevision, previewBrain } from "./brain-assets";
import { addNode, moveNode, parseBrain, parseTrace, removeNode, serializeBrain, setNodeField, shiftNode, type BrainDocument, type BrainNode } from "./brains";
import type { ComponentValue } from "./components";
import { locomotionCatalogue, previewProfile, profileRevision } from "./locomotion-assets";
import { changeType, parseLocomotionTrace, parseProfile, serializeProfile, type LocomotionProfile } from "./locomotion";
import type { ProjectSnapshot } from "./project";
import { publishProjectEvent } from "./project-events";
import { RefError, resolveFieldRefs, resolveRef, type ProjectIndex } from "./project-index";
import { journal, textDiff, withSceneLock, type OpResult } from "./project-ops";
import { gameTool } from "./scene-preview";
import { applyChanges } from "./statement-files";


export async function validateAsset(project: ProjectSnapshot, flag: string, target: string, content: string) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-mcp-behaviour-"));
  try { const file = path.join(directory, path.basename(target)); await writeFile(file, content); await gameTool(project, [flag, target, "--candidate", file]); }
  finally { await rm(directory, { recursive: true, force: true }); }
}

export async function publishAsset(project: ProjectSnapshot, tool: string, summary: string, target: string, before: string | null, content: string, flag: string, dryRun: boolean | undefined, ref: string): Promise<OpResult & { path: string }> {
  await validateAsset(project, flag, target, content);
  const diff = textDiff(before ?? "", content);
  const revision = brainRevision(content);
  if (before === content) return { applied: false, dryRun: dryRun === true, changeId: null, summary: `${summary}: no change.`, diff: "", revision, ref, live: false, path: target };
  if (dryRun) return { applied: false, dryRun: true, changeId: null, summary: `Would ${summary}. Validated by the engine; nothing written.`, diff, revision, ref, live: false, path: target };
  return withSceneLock(async () => {
    await applyChanges(project, [{ path: target, before, content }]);
    const changeId = await journal(project, tool, summary, [{ path: target, before, content }]);
    publishProjectEvent({ type: "changed", projectId: project.id, changeId, source: "mcp", summary, paths: [target], scenes: [], live: false });
    return { applied: true, dryRun: false, changeId, summary: `${summary}.`, diff, revision, ref, live: false, path: target };
  });
}

function existing(project: ProjectSnapshot, index: ProjectIndex, ref: string, kind: "brain" | "locomotion", revision: string) {
  const entry = resolveRef(index, ref);
  if (entry.type !== "asset" || entry.kind !== kind) throw new RefError(`${ref} is not a ${kind} asset. Use find with type "asset".`);
  const content = project.files.find(file => file.path === entry.path)?.content;
  if (!content) throw new RefError(`${ref} has no readable file.`);
  if (typeof revision !== "string" || brainRevision(content) !== revision) throw new RefError(`${ref} changed since you described it (or the revision is missing). describe it again and retry.`);
  return { entry, content };
}

type NodeInput = { id?: string; type: string; parent?: string | null; fields?: Record<string, unknown> };
export type BrainOperation =
  | { op: "add"; type: string; parent?: string | null; id?: string; fields?: Record<string, unknown> }
  | { op: "update"; id: string; fields: Record<string, unknown> }
  | { op: "remove"; id: string }
  | { op: "move"; id: string; parent: string }
  | { op: "reorder"; id: string; by: -1 | 1 };
const nodeId = () => `n-${randomUUID().slice(0, 8)}`;

async function applyOperations(project: ProjectSnapshot, index: ProjectIndex, brain: BrainDocument, operations: BrainOperation[]) {
  const catalogue = await brainCatalogue(project);
  const added: string[] = [];
  for (const [i, operation] of operations.entries()) {
    const where = `operation ${i + 1} (${operation.op})`;
    try {
      if (operation.op === "add") {
        const id = operation.id ?? nodeId();
        if (brain.nodes.some(node => node.id === id)) throw new RefError(`node id ${id} already exists`);
        brain = addNode(brain, catalogue, operation.type, operation.parent ?? undefined, id, resolveFieldRefs(index, operation.fields ?? {}) as Record<string, ComponentValue>);
        added.push(id);
      } else if (operation.op === "update") {
        if (!brain.nodes.some(node => node.id === operation.id)) throw new RefError(`no node ${operation.id}`);
        for (const [name, value] of Object.entries(resolveFieldRefs(index, operation.fields ?? {}))) brain = setNodeField(brain, operation.id, name, value === null ? undefined : value as ComponentValue);
      } else if (operation.op === "remove") brain = removeNode(brain, operation.id);
      else if (operation.op === "move") brain = moveNode(brain, catalogue, operation.id, operation.parent);
      else if (operation.op === "reorder") brain = shiftNode(brain, operation.id, operation.by);
      else throw new RefError("unknown op; use add, update, remove, move or reorder");
    } catch (error) { throw new RefError(`${where}: ${error instanceof Error ? error.message : error}`); }
  }
  return { brain, added };
}

export async function createBrain(project: ProjectSnapshot, index: ProjectIndex, input: { name: string; description?: string; nodes?: NodeInput[]; dryRun?: boolean }) {
  const id = randomUUID();
  // Nodes arrive parent-first; ids may be chosen by the caller so later nodes can name their parent.
  const operations: BrainOperation[] = (input.nodes ?? []).map(node => ({ op: "add", type: node.type, parent: node.parent ?? undefined, id: node.id, fields: node.fields }));
  const { brain, added } = await applyOperations(project, index, { id, name: input.name.trim(), description: input.description ?? "", nodes: [] }, operations);
  if (!brain.nodes.length) throw new RefError("A brain needs at least one node.");
  const target = `assets/brains/${id}.brain.toml`;
  const result = await publishAsset(project, "create_brain", `create brain ${brain.name} (asset:${id})`, target, null, serializeBrain(brain), "--validate-brain", input.dryRun, `asset:${id}`);
  return { ...result, nodes: added };
}

export async function editBrain(project: ProjectSnapshot, index: ProjectIndex, input: { ref: string; revision: string; name?: string; description?: string; operations?: BrainOperation[]; dryRun?: boolean }) {
  const { entry, content } = existing(project, index, input.ref, "brain", input.revision);
  let brain = parseBrain(content);
  if (input.name !== undefined) brain = { ...brain, name: input.name.trim() };
  if (input.description !== undefined) brain = { ...brain, description: input.description };
  const applied = await applyOperations(project, index, brain, input.operations ?? []);
  const changes = (input.operations ?? []).map(op => op.op).join(", ") || "details";
  const result = await publishAsset(project, "edit_brain", `edit brain ${applied.brain.name} (${entry.ref}): ${changes}`, entry.path, content, serializeBrain(applied.brain), "--validate-brain", input.dryRun, entry.ref);
  return { ...result, nodes: applied.added };
}

export async function writeLocomotion(project: ProjectSnapshot, index: ProjectIndex, input: { ref?: string; revision?: string; name?: string; description?: string; type?: string; fields?: Record<string, unknown>; dryRun?: boolean }) {
  const catalogue = await locomotionCatalogue(project);
  const current = input.ref ? existing(project, index, input.ref, "locomotion", input.revision ?? "") : null;
  let profile: LocomotionProfile = current ? parseProfile(current.content) : { id: randomUUID(), name: "", description: "", type: input.type ?? "", fields: {} };
  if (input.type && input.type !== profile.type) {
    const type = catalogue.types.find(item => item.id === input.type);
    if (!type) throw new RefError(`Unknown locomotion type ${input.type}; see list_locomotion_types.`);
    profile = changeType(profile, type);
  }
  if (!profile.type) throw new RefError("A new profile needs a type; see list_locomotion_types.");
  if (input.name !== undefined) profile.name = input.name.trim();
  if (input.description !== undefined) profile.description = input.description;
  for (const [name, value] of Object.entries(input.fields ?? {})) {
    if (value === null) delete profile.fields[name]; else profile.fields[name] = value as ComponentValue;
  }
  if (!profile.name) throw new RefError("Give the profile a name.");
  const target = current?.entry.path ?? `assets/locomotion/${profile.id}.locomotion.toml`;
  const verb = current ? "update" : "create";
  return publishAsset(project, current ? "update_locomotion" : "create_locomotion", `${verb} locomotion ${profile.name} (${profile.type}, asset:${profile.id})`, target, current?.content ?? null, serializeProfile(profile), "--validate-locomotion", input.dryRun, current?.entry.ref ?? `asset:${profile.id}`);
}

const round = (n: number, places = 2) => Math.round(n * 10 ** places) / 10 ** places;
const vec = (p: { x: number; y?: number; z: number }) => p.y === undefined ? [round(p.x), round(p.z)] : [round(p.x), round(p.y), round(p.z)];

/** Runs a saved brain in the engine and summarises the motion for an AI (no raw frames). */
export async function previewBrainSummary(project: ProjectSnapshot, index: ProjectIndex, input: { ref: string; agents?: number; targets?: number; obstacles?: number; target_speed?: number; seconds?: number; agent_group?: string; target_group?: string; max_speed?: number; seed?: number }) {
  const entry = resolveRef(index, input.ref);
  if (entry.type !== "asset" || entry.kind !== "brain") throw new RefError(`${input.ref} is not a brain asset.`);
  const brain = parseBrain(project.files.find(file => file.path === entry.path)?.content ?? "");
  const options = { agents: input.agents, targets: input.targets, obstacles: input.obstacles, targetSpeed: input.target_speed, seconds: input.seconds, agentGroup: input.agent_group, targetGroup: input.target_group, maxSpeed: input.max_speed, seed: input.seed };
  const trace = parseTrace(await previewBrain(project, entry.path, brain, options));
  const point = (frame: number[], i: number) => ({ x: frame[i * 3], y: frame[i * 3 + 1], z: frame[i * 3 + 2] });
  const distance = (a: { x: number; y: number; z: number }, b: { x: number; y: number; z: number }) => Math.hypot(a.x - b.x, a.y - b.y, a.z - b.z);
  const sample = (f: number) => {
    const at = Math.min(trace.frames.length - 1, Math.round(f * (trace.frames.length - 1))), frame = trace.frames[at], previous = trace.frames[Math.max(0, at - 1)];
    const agents = Array.from({ length: trace.agents }, (_, i) => point(frame, i)), targets = Array.from({ length: trace.targets }, (_, i) => point(frame, trace.agents + i));
    const centroid = agents.reduce((sum, p) => ({ x: sum.x + p.x / agents.length, y: sum.y + p.y / agents.length, z: sum.z + p.z / agents.length }), { x: 0, y: 0, z: 0 });
    const speed = at ? agents.reduce((sum, p, i) => sum + distance(p, point(previous, i)), 0) / agents.length / trace.frameSeconds : 0;
    let closest = Infinity;
    for (let i = 0; i < Math.min(agents.length, 64); i++) for (let j = i + 1; j < Math.min(agents.length, 64); j++) closest = Math.min(closest, distance(agents[i], agents[j]));
    return {
      seconds: round(at * trace.frameSeconds, 1), centroid: vec(centroid), meanSpeed: round(speed),
      spread: round(agents.reduce((sum, p) => sum + distance(p, centroid), 0) / agents.length),
      meanDistanceToNearestTarget: targets.length ? round(agents.reduce((sum, p) => sum + Math.min(...targets.map(t => distance(p, t))), 0) / agents.length) : null,
      closestPair: Number.isFinite(closest) ? round(closest) : null,
    };
  };
  const last = trace.frames.at(-1)!;
  return {
    ref: entry.ref, name: brain.name, agents: trace.agents, targets: trace.targets, obstacles: trace.obstacles.map(o => [...vec(o), round(o.radius)]),
    samples: [0, 0.25, 0.5, 0.75, 1].map(sample),
    finalAgents: Array.from({ length: Math.min(trace.agents, 16) }, (_, i) => vec(point(last, i))),
    note: "Agents spawn 2-10 m from the origin; targets circle at 12 m. Distances in metres, speeds in m/s.",
  };
}

/** Runs a saved locomotion profile along a course in the engine and summarises it. */
export async function previewLocomotionSummary(project: ProjectSnapshot, index: ProjectIndex, input: { ref: string; course?: string; seconds?: number }) {
  const entry = resolveRef(index, input.ref);
  if (entry.type !== "asset" || entry.kind !== "locomotion") throw new RefError(`${input.ref} is not a locomotion asset.`);
  const profile = parseProfile(project.files.find(file => file.path === entry.path)?.content ?? "");
  const trace = parseLocomotionTrace(await previewProfile(project, entry.path, profile, { course: input.course, seconds: input.seconds }));
  const frames = trace.frames, deg = (r: number) => round(r * 180 / Math.PI, 1);
  let travelled = 0;
  for (let i = 1; i < frames.length; i++) travelled += Math.hypot(frames[i].x - frames[i - 1].x, frames[i].y - frames[i - 1].y, frames[i].z - frames[i - 1].z);
  const max = (pick: (f: (typeof frames)[number]) => number) => round(Math.max(...frames.map(pick)));
  const end = frames.at(-1)!;
  return {
    ref: entry.ref, name: profile.name, type: trace.type, course: trace.course.map(vec), seconds: round(frames.length * trace.frameSeconds, 1),
    distanceTravelled: round(travelled, 1), maxSpeed: max(f => f.speed), height: { min: round(Math.min(...frames.map(f => f.y))), max: max(f => f.y) },
    maxSteerDegrees: deg(Math.max(...frames.map(f => Math.abs(f.steer)))), maxBankDegrees: deg(Math.max(...frames.map(f => Math.abs(f.bank)))), maxPitchDegrees: deg(Math.max(...frames.map(f => Math.abs(f.pitch)))),
    brakingFraction: round(frames.filter(f => f.brake > 0).length / frames.length), final: { position: vec(end), speed: round(end.speed), landed: end.landed, fuel: round(end.fuel) },
    // Physical aircraft: the phases flown, in order, and where the wheels first touched after flying.
    ...(frames.some(f => f.phase) ? { phases: frames.reduce<string[]>((list, f) => (f.phase && list.at(-1) !== f.phase ? [...list, f.phase] : list), []), touchdown: (() => { const i = frames.findIndex((f, j) => j > 0 && f.phase === "Rollout" && frames[j - 1].phase !== "Rollout"); return i > 0 ? { seconds: round(i * trace.frameSeconds, 1), position: vec(frames[i]) } : null; })() } : {}),
    samples: [0.25, 0.5, 0.75].map(f => { const p = frames[Math.round(f * (frames.length - 1))]; return { seconds: round(Math.round(f * (frames.length - 1)) * trace.frameSeconds, 1), position: vec(p), speed: round(p.speed), headingDegrees: deg(p.yaw) }; }),
  };
}

/** The brain node and locomotion type catalogues, compacted for prompts. */
export async function behaviourCatalogues(project: ProjectSnapshot) {
  const [brain, locomotion] = await Promise.all([brainCatalogue(project), locomotionCatalogue(project)]);
  const fields = (list: { name: string; kind: string; default?: unknown; min?: number; max?: number; unit?: string; options?: string[]; help?: string }[]) => list.filter(field => field.name !== "label").map(({ name, kind, default: value, min, max, unit, options, help }) => ({ name, kind, default: value, min, max, unit, options, help: help || undefined }));
  return {
    brainNodes: { categories: brain.categories, nodes: brain.nodes.map(node => ({ type: node.id, label: node.label, category: node.category, role: node.role, status: node.status, description: node.description, fields: fields(node.fields) })) },
    locomotionTypes: { categories: locomotion.categories, types: locomotion.types.map(type => ({ type: type.id, label: type.label, category: type.category, status: type.status, description: type.description, manoeuvres: type.manoeuvres, fields: fields(type.fields) })) },
  };
}

export const describeBrain = (content: string) => { const brain = parseBrain(content); return { brain: { name: brain.name, description: brain.description, nodes: brain.nodes.map((node: BrainNode) => ({ id: node.id, type: node.type, parent: node.parent ?? null, fields: node.fields })) }, revision: brainRevision(content) }; };
export const describeLocomotion = (content: string) => { const profile = parseProfile(content); return { locomotion: { name: profile.name, description: profile.description, type: profile.type, fields: profile.fields }, revision: profileRevision(content) }; };
