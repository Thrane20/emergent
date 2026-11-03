import "server-only";
import { SCENE_MAX_BYTES, SCENE_MAX_INSTANCES } from "./scene-limits";
import { randomUUID } from "node:crypto";
import { mkdir, readFile, rename, rm, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { parse, stringify } from "smol-toml";
import { containedFile, missing, validateGlb, within } from "./object-files";
import { MAX_MODEL_BYTES, ObjectCreatorError } from "./object-types";
import { foundationRoot } from "./project-code-context";
import { projectScenes } from "./project-scenes";
import { readProjectBinding, type ProjectSnapshot } from "./project";
import { applyChanges, checkCandidate, type FileChange } from "./statement-files";
import { capturePrefab, parsePrefab } from "./prefabs";

export const MAX_PARTS = 64;
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const partName = /^[\p{L}\p{N}][\p{L}\p{N} _.-]{0,63}$/u;

export type ModelPart = { id: string; name: string; parent: string | null; pivot: [number, number, number] };
export type PartsDocument = { schema_version: 1; model: string; root: string; parts: (ModelPart & { asset: string })[] };
export type SavedPart = ModelPart & { asset: string };
export type GeneratedModel = { descriptor: string; id: string; name: string; model: string; parts: string | null; saved: SavedPart[] };

/** Saved Object Creator models, with their part set when one has been saved. */
export function generatedModels(project: ProjectSnapshot): GeneratedModel[] {
  return project.files.flatMap(file => {
    if (!file.path.endsWith("/object.asset.json") || !file.content) return [];
    try {
      const value = JSON.parse(file.content);
      if (value.schema_version !== 1 || value.kind !== "generated-model" || typeof value.model !== "string") return [];
      const parts = `${path.posix.dirname(file.path)}/parts/parts.toml`, source = project.files.find(f => f.path === parts)?.content;
      let saved: SavedPart[] = [];
      // A damaged part set reopens as the whole model rather than hiding the model.
      try { if (source) saved = partsDocument(source).parts; } catch { saved = []; }
      return [{ descriptor: file.path, id: String(value.id), name: String(value.name), model: value.model, parts: saved.length ? parts : null, saved }];
    } catch { return []; }
  });
}

/** Model part sets enter the same Object Library as structures, using the existing prefab contract. */
export function modelPartsLibrary(project: ProjectSnapshot): { path: string; name: string; modelDescriptor: string }[] {
  return generatedModels(project).flatMap(model => {
    if (!model.saved.length) return [];
    const path = `${model.descriptor.slice(0, model.descriptor.lastIndexOf("/"))}/parts/model.prefab.toml`;
    const source = project.files.find(file => file.path === path)?.content;
    if (!source) return [];
    try { return [{ path, name: parsePrefab(source).name, modelDescriptor: model.descriptor }]; }
    catch { return []; }
  });
}

/** Validates an editor part list: bounded, unique durable IDs, acyclic parents, finite pivots. */
export function modelParts(value: unknown): ModelPart[] {
  if (!Array.isArray(value) || !value.length || value.length > MAX_PARTS) throw new ObjectCreatorError(`A model needs 1–${MAX_PARTS} parts.`);
  const parts = value.map(item => {
    const part = item as Partial<ModelPart>;
    if (!part || typeof part.id !== "string" || !uuid.test(part.id)) throw new ObjectCreatorError("Each part needs a durable ID.");
    if (typeof part.name !== "string" || !partName.test(part.name.trim())) throw new ObjectCreatorError(`Name parts with 1–64 letters, numbers, spaces, dots, underscores or hyphens (${String(part.name)}).`);
    if (part.parent !== null && typeof part.parent !== "string") throw new ObjectCreatorError("A part's parent must be another part or none.");
    if (!Array.isArray(part.pivot) || part.pivot.length !== 3 || !part.pivot.every(n => typeof n === "number" && Number.isFinite(n) && Math.abs(n) <= 10_000)) throw new ObjectCreatorError("Part pivots must be three finite numbers.");
    return { id: part.id, name: part.name.trim(), parent: part.parent ?? null, pivot: part.pivot as [number, number, number] };
  });
  const ids = new Set(parts.map(part => part.id));
  if (ids.size !== parts.length) throw new ObjectCreatorError("Part IDs must be unique.");
  const parents = new Map(parts.map(part => [part.id, part.parent]));
  for (const part of parts) {
    if (part.parent !== null && !ids.has(part.parent)) throw new ObjectCreatorError(`${part.name} has a missing parent.`);
    let cursor = part.parent, steps = 0;
    while (cursor !== null) {
      if (cursor === part.id || ++steps > parts.length) throw new ObjectCreatorError("Part parents cannot form a cycle.");
      cursor = parents.get(cursor) ?? null;
    }
  }
  return parts;
}

/** Parents before children, so scene files read top-down. */
function ordered<T extends ModelPart>(parts: T[]) {
  const result: T[] = [], placed = new Set<string>();
  while (result.length < parts.length) for (const part of parts) if (!placed.has(part.id) && (part.parent === null || placed.has(part.parent))) { result.push(part); placed.add(part.id); }
  return result;
}

const local = (part: ModelPart, byId: Map<string, ModelPart>) => {
  const parent = part.parent ? byId.get(part.parent)!.pivot : [0, 0, 0];
  return part.pivot.map((value, axis) => value - parent[axis]);
};

async function text(project: ProjectSnapshot, relative: string, limit: number) {
  try { return await readFile(await containedFile(project.root, relative, limit), "utf8"); }
  catch (error) { if (missing(error) || error instanceof ObjectCreatorError && error.status === 404) return null; throw error; }
}

export async function readParts(project: ProjectSnapshot, relative: string): Promise<{ source: string; document: PartsDocument }> {
  if (!relative.endsWith("/parts/parts.toml") || !project.assetRoots.some(root => relative.startsWith(`${root}/`))) throw new ObjectCreatorError("Choose a model part set.");
  const source = await text(project, relative, 256 * 1024);
  if (source === null) throw new ObjectCreatorError("This model has no saved parts.", 404);
  return { source, document: partsDocument(source) };
}

function partsDocument(source: string): PartsDocument {
  const raw = parse(source) as unknown as PartsDocument;
  if (raw.schema_version !== 1 || typeof raw.model !== "string" || typeof raw.root !== "string" || !Array.isArray(raw.parts)) throw new ObjectCreatorError("Invalid part set.");
  const parts = modelParts(raw.parts.map(part => ({ ...part, parent: part.parent ?? null })));
  return { ...raw, parts: parts.map((part, i) => ({ ...part, asset: String(raw.parts[i].asset) })) };
}

const placement = globalThis as typeof globalThis & { emergentScenePlacementActive?: boolean };
async function exclusive<T>(action: () => Promise<T>) {
  if (placement.emergentScenePlacementActive) throw new ObjectCreatorError("Another scene edit is being saved.", 409);
  placement.emergentScenePlacementActive = true;
  try { return await action(); } finally { placement.emergentScenePlacementActive = false; }
}

/**
 * Saves a part set beside its generated model: one re-centred GLB and one `kind = "model"`
 * entity per part, an empty group entity, and `parts/parts.toml`. Every file is validated by
 * the engine in a staged copy before anything in the project changes.
 */
export async function saveModelParts(project: ProjectSnapshot, input: { descriptor: unknown; parts: unknown; files: Map<string, Buffer> }) {
  return exclusive(async () => {
    if (typeof input.descriptor !== "string" || !generatedModels(project).some(model => model.descriptor === input.descriptor)) throw new ObjectCreatorError("Choose a saved generated model.");
    const descriptor = input.descriptor, model = generatedModels(project).find(item => item.descriptor === descriptor)!;
    const parts = modelParts(input.parts), byId = new Map(parts.map(part => [part.id, part]));
    const folder = `${path.posix.dirname(descriptor)}/parts`, partsPath = `${folder}/parts.toml`, libraryPath = `${folder}/model.prefab.toml`;
    const previous = model.parts ? await readParts(project, model.parts) : null;
    let total = 0;
    const binaries = parts.map(part => {
      const bytes = input.files.get(part.id);
      if (!bytes) throw new ObjectCreatorError(`${part.name} has no model data.`);
      validateGlb(bytes);
      if ((total += bytes.length) > 2 * MAX_MODEL_BYTES) throw new ObjectCreatorError("The part set exceeds 200 MiB.");
      return { path: `${folder}/${part.id}.glb`, bytes };
    });
    const rootId = previous ? path.posix.basename(previous.document.root, ".entity.toml") : randomUUID();
    const rootAsset = `${folder}/${rootId}.entity.toml`;
    const changes: FileChange[] = [];
    const change = async (file: string, content: string) => { const before = await text(project, file, SCENE_MAX_BYTES); if (before !== content) changes.push({ path: file, before, content }); };
    await change(rootAsset, stringify({ schema_version: 1, id: rootId, name: model.name, kind: "entity" }));
    for (const part of parts) await change(`${folder}/${part.id}.entity.toml`, stringify({ schema_version: 1, id: part.id, name: part.name, kind: "model", model: `${folder}/${part.id}.glb` }));
    const document = { schema_version: 1, model: descriptor, root: rootAsset, parts: ordered(parts).map(part => ({ id: part.id, name: part.name, ...(part.parent ? { parent: part.parent } : {}), pivot: part.pivot, asset: `${folder}/${part.id}.entity.toml` })) };
    await change(partsPath, stringify(document));
    const removed = (previous?.document.parts ?? []).filter(part => !byId.has(part.id));
    // These two owned manifests are replaced in this save; external scene/prefab references must still block deletion.
    const referenced = removed.filter(part => project.connections.some(connection => connection.target === part.asset && connection.source !== partsPath && connection.source !== libraryPath));
    if (referenced.length) throw new ObjectCreatorError(`Remove ${referenced.map(part => part.name).join(", ")} from scenes before deleting ${referenced.length === 1 ? "it" : "them"} from the part set.`, 409);
    const scenes = projectScenes(project).map(scene => scene.path);
    if (scenes.length > 126) throw new ObjectCreatorError("Part validation supports at most 126 project scenes.");
    const probe = `${folder}/${randomUUID()}.scene.toml`;
    const probeScene = stringify({ schema_version: 1, id: "probe", entities: [{ id: "model", asset: rootAsset }, ...ordered(parts).map(part => ({ id: part.id, asset: `${folder}/${part.id}.entity.toml`, parent: part.parent ?? "model", transform: { translation: local(part, byId) } }))] });
    const libraryBefore = await text(project, libraryPath, 256 * 1024);
    const prefab = capturePrefab(probeScene, "model", model.name, libraryBefore ? parsePrefab(libraryBefore).id : randomUUID(), []);
    await change(libraryPath, stringify(prefab));
    await checkCandidate(project, [...changes, { path: probe, before: null, content: probeScene }], { root: await foundationRoot(project) }, [...scenes, probe, libraryPath], binaries);
    const binding = await readProjectBinding();
    if (binding.id !== project.id || binding.root !== project.root) throw new ObjectCreatorError("Linked project changed during validation.", 409);
    // GLBs publish first with a rollback copy, then the TOML batch goes through the guarded writer.
    const root = project.root, published: { target: string; backup: string | null }[] = [];
    try {
      for (const binary of binaries) {
        const target = path.join(root, binary.path);
        if (!within(root, target)) throw new ObjectCreatorError("Part path escapes the project.", 403);
        const backup = await readFile(target).then(() => `${target}.${randomUUID()}.bak`, () => null);
        if (backup) await rename(target, backup);
        published.push({ target, backup });
        await mkdir(path.dirname(target), { recursive: true });
        await writeFile(`${target}.tmp`, binary.bytes, { mode: 0o644 });
        await rename(`${target}.tmp`, target);
      }
      await applyChanges(project, changes);
    } catch (error) {
      for (const { target, backup } of published.reverse()) { if (backup) await rename(backup, target); else await rm(target, { force: true }); }
      throw error;
    }
    for (const { backup } of published) if (backup) await rm(backup, { force: true });
    for (const part of removed) for (const file of [part.asset, part.asset.replace(/\.entity\.toml$/, ".glb")]) await unlink(path.join(root, file)).catch(() => undefined);
    return { parts: partsPath, root: rootAsset, library: libraryPath };
  });
}

/** Adds the part set to a scene as a group instance with one child instance per part. */
export async function placeModelParts(project: ProjectSnapshot, input: { parts: unknown; scenePath: unknown }) {
  return exclusive(async () => {
    if (typeof input.parts !== "string") throw new ObjectCreatorError("Choose a model part set.");
    if (typeof input.scenePath !== "string" || !projectScenes(project).some(scene => scene.path === input.scenePath)) throw new ObjectCreatorError("Choose a scene in this project.");
    const { document } = await readParts(project, input.parts), byId = new Map(document.parts.map(part => [part.id, part]));
    const before = await text(project, input.scenePath, SCENE_MAX_BYTES);
    if (before === null) throw new ObjectCreatorError("Scene was not found.", 404);
    const scene = parse(before);
    if (scene.schema_version !== 1 || Array.isArray(scene.entities) && scene.entities.length + document.parts.length >= SCENE_MAX_INSTANCES) throw new ObjectCreatorError("Choose a schema-1 scene with room for these instances.");
    const group = randomUUID(), instances = new Map(document.parts.map(part => [part.id, randomUUID()]));
    const blocks = [`[[entities]]\nid = ${JSON.stringify(group)}\nasset = ${JSON.stringify(document.root)}\n`, ...ordered(document.parts).map(part => `[[entities]]\nid = ${JSON.stringify(instances.get(part.id))}\nasset = ${JSON.stringify(part.asset)}\nparent = ${JSON.stringify(part.parent ? instances.get(part.parent) : group)}\n\n[entities.transform]\ntranslation = ${JSON.stringify(local(part, byId))}\n`)];
    const first = before.search(/^\[/m), header = first < 0 ? before : before.slice(0, first), tail = first < 0 ? "" : before.slice(first);
    const content = header.replace(/^entities\s*=\s*\[\s*\]\s*$/m, "") + tail + "\n" + blocks.join("\n");
    parse(content);
    if (Buffer.byteLength(content) > SCENE_MAX_BYTES) throw new ObjectCreatorError("Scene exceeds 2 MiB.");
    const changes: FileChange[] = [{ path: input.scenePath, before, content }];
    await checkCandidate(project, changes, { root: await foundationRoot(project) }, [input.scenePath]);
    await applyChanges(project, changes);
    return { scenePath: input.scenePath, instance: group, parts: Object.fromEntries([...instances].map(([part, instance]) => [byId.get(part)!.name, instance])) };
  });
}
