import "server-only";
import { SCENE_MAX_BYTES, SCENE_MAX_INSTANCES } from "./scene-limits";
import { randomUUID } from "node:crypto";
import { mkdir, readdir, readFile, realpath, rm, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { parse } from "smol-toml";
import { settingsDirectory } from "./ai-settings";
import { cameraSettings } from "./camera-settings";
import type { ComponentPatch } from "./components";
import { projectImages } from "./image-library";
import { inspectionSessions } from "./inspection-session";
import { lightingSettings } from "./lighting-settings";
import { containedFile } from "./object-files";
import type { ProjectSnapshot } from "./project";
import { publishProjectEvent } from "./project-events";
import { projectIndex, RefError, resolveFieldRefs, resolveRef, type InstanceEntry, type ProjectIndex, type SceneEntry } from "./project-index";
import { replaceSceneLighting, replaceSceneWind, sceneWind } from "./scene-lighting";
import { addedComponentType, replaceInstanceComponent,
  commitSceneEdit, materialEdit, physicsSettings, replaceInstanceCamera, replaceInstanceName, replaceInstanceParent,
  replaceInstancePhysics, replaceInstanceTransform, replaceSceneSubtree, scenePlacement, transformEdit, type TransformValues,
} from "./scene-placement";
import { parsePrefab, placePrefab } from "./prefabs";
import { componentRegistry } from "./scene-preview";
import { validateProjectScene } from "./scene-preview";
import { applyChanges } from "./statement-files";

/**
 * Guarded project edits for AI tools. Every edit names durable refs, requires the scene revision the caller read,
 * validates through the same Rust checks as the IDE, records an undo journal entry and notifies open IDE pages.
 */
export type FileChange = { path: string; before: string | null; content: string };
export type OpResult = { applied: boolean; dryRun: boolean; changeId: string | null; summary: string; diff: string; revision?: string; ref?: string; live: boolean };
type JournalEntry = { id: string; projectId: string; root: string; at: string; tool: string; summary: string; changes: FileChange[]; undoneBy?: string; undoOf?: string };

const JOURNAL_LIMIT = 200;
const vector = (value: unknown, name: string) => {
  if (value === undefined) return undefined;
  if (!Array.isArray(value) || value.length !== 3 || !value.every(item => typeof item === "number" && Number.isFinite(item))) throw new RefError(`${name} must be three finite numbers.`);
  return value as number[];
};

function instanceTarget(index: ProjectIndex, ref: string) {
  const entry = resolveRef(index, ref);
  if (entry.type !== "instance") throw new RefError(`${ref} is not a scene instance ref.`);
  return { entry, scene: index.scenes.find(scene => scene.ref === entry.scene)! };
}
function sceneTarget(index: ProjectIndex, ref: string) {
  const entry = resolveRef(index, ref);
  if (entry.type !== "scene") throw new RefError(`${ref} is not a scene ref.`);
  return entry;
}
async function currentInstance(project: ProjectSnapshot, scene: SceneEntry, id: string, space: "local" | "global" = "local") {
  const placement = await scenePlacement(project, scene.path, space);
  const instance = placement.instances.find(item => item.id === id);
  if (!instance) throw new RefError(`instance:${scene.id}/${id} no longer exists; call find again.`);
  return { placement, instance };
}

/** Runs one scene edit; on success journals it, pushes `live` controls to the running inspection world and notifies pages. */
async function sceneEdit(project: ProjectSnapshot, scene: SceneEntry, revision: string, tool: string, summary: string, edit: (source: string) => string | Promise<string>, options: { dryRun?: boolean; check?: () => Promise<void>; live?: (sessionId: string) => Promise<void>; ref?: string; source?: "mcp" | "ide"; removal?: boolean } = {}): Promise<OpResult> {
  if (typeof revision !== "string" || !/^[0-9a-f]{64}$/.test(revision)) throw new RefError("Pass the sceneRevision/revision from describe; edits are refused against an unknown revision.");
  const { before, content, changed, warning } = await commitSceneEdit(project, scene.path, revision, edit, { check: options.check, dryRun: options.dryRun, removal: options.removal });
  if (warning) summary = `${summary} (the scene still has a problem: ${warning})`;
  if (!changed) return { applied: false, dryRun: options.dryRun === true, changeId: null, summary: `${summary}: no change.`, diff: "", revision, ref: options.ref, live: false };
  const diff = textDiff(before, content);
  if (options.dryRun) return { applied: false, dryRun: true, changeId: null, summary: `Would ${summary}. Validated; nothing written.`, diff, revision, ref: options.ref, live: false };
  const changeId = await journal(project, tool, summary, [{ path: scene.path, before, content }]);
  const live = await pushLive(project, scene.path, options.live);
  publishProjectEvent({ type: "changed", projectId: project.id, changeId, source: options.source ?? "mcp", summary, paths: [scene.path], scenes: [scene.path], live });
  const fresh = await scenePlacement(project, scene.path);
  return { applied: true, dryRun: false, changeId, summary: `${summary}.`, diff, revision: fresh.revision, ref: options.ref, live };
}

async function pushLive(project: ProjectSnapshot, scenePath: string, push?: (sessionId: string) => Promise<void>) {
  const session = push ? inspectionSessions.activeFor(project, scenePath) : null;
  if (!session || !push) return false;
  try { await push(session); return true; } catch { return false; }
}

/** One journalled, Rust-validated, undoable edit of a scene or prefab by path. */
export async function editScenePath(project: ProjectSnapshot, scenePath: string, revision: string, tool: string, summary: string, edit: (source: string) => string | Promise<string>) {
  if (scenePath.endsWith(".prefab.toml")) {
    if (!project.files.some(file => file.path === scenePath && project.assetRoots.some(root => scenePath.startsWith(`${root}/`)))) throw new RefError("Choose a prefab in this project.");
    const { before, content, changed } = await commitSceneEdit(project, scenePath, revision, edit);
    if (!changed) return { applied: false, revision };
    const changeId = await journal(project, tool, summary, [{ path: scenePath, before, content }]);
    publishProjectEvent({ type: "changed", projectId: project.id, changeId, source: "ide", summary, paths: [scenePath], scenes: [], live: false });
    return { applied: true, revision: (await scenePlacement(project, scenePath)).revision };
  }
  const scene = (await projectIndex(project)).scenes.find(entry => entry.path === scenePath);
  if (!scene) throw new RefError(`${scenePath} is not a scene in this project.`);
  return sceneEdit(project, scene, revision, tool, summary, edit, { source: "ide" });
}

export type TransformInput = { ref: string; revision: string; translation?: unknown; rotation_degrees?: unknown; scale?: unknown; space?: "local" | "global"; dryRun?: boolean };
export async function setTransform(project: ProjectSnapshot, input: TransformInput) {
  const index = await projectIndex(project), { entry, scene } = instanceTarget(index, input.ref), space = input.space ?? "local";
  const patch = { translation: vector(input.translation, "translation"), rotation_degrees: vector(input.rotation_degrees, "rotation_degrees"), scale: vector(input.scale, "scale") };
  if (!patch.translation && !patch.rotation_degrees && !patch.scale) throw new RefError("Pass at least one of translation, rotation_degrees or scale.");
  const { instance } = await currentInstance(project, scene, entry.id, space);
  const base = space === "global" ? instance.display ?? instance.transform : instance.transform;
  const positionOnly = space === "global" && instance.globalTranslationOnly === true;
  if (positionOnly && (patch.rotation_degrees || patch.scale)) throw new RefError(`${entry.ref} sits under a sheared parent; only its global translation can be set.`);
  const next: TransformValues = { translation: patch.translation ?? base.translation, rotation_degrees: patch.rotation_degrees ?? base.rotation_degrees, scale: patch.scale ?? base.scale };
  return sceneEdit(project, scene, input.revision, "set_transform", `set ${space} transform of ${entry.name} (${entry.ref})`, transformEdit(project, scene.path, entry.id, next, space, positionOnly), {
    dryRun: input.dryRun, ref: entry.ref,
    live: async session => { const local = (await currentInstance(project, scene, entry.id)).instance.transform; await inspectionSessions.command(project, session, { op: "transform", id: entry.id, ...local }); },
  });
}

export async function renameInstance(project: ProjectSnapshot, input: { ref: string; revision: string; name: string; dryRun?: boolean }) {
  const index = await projectIndex(project), { entry, scene } = instanceTarget(index, input.ref);
  return sceneEdit(project, scene, input.revision, "rename", `rename ${entry.name} (${entry.ref}) to "${input.name.trim()}"`, source => replaceInstanceName(source, entry.id, input.name), { dryRun: input.dryRun, ref: entry.ref });
}

export async function reparentInstance(project: ProjectSnapshot, input: { ref: string; revision: string; parent: string | null; dryRun?: boolean }) {
  const index = await projectIndex(project), { entry, scene } = instanceTarget(index, input.ref);
  let parent: InstanceEntry | null = null;
  if (input.parent !== null) {
    const target = instanceTarget(index, input.parent);
    if (target.scene.ref !== scene.ref) throw new RefError("Parents must be in the same scene.");
    parent = target.entry;
  }
  return sceneEdit(project, scene, input.revision, "reparent", `move ${entry.name} (${entry.ref}) under ${parent ? `${parent.name} (${parent.ref})` : "the scene root"}, keeping its local transform`, source => replaceInstanceParent(source, entry.id, parent?.id ?? null), {
    dryRun: input.dryRun, ref: entry.ref,
    live: async session => { await inspectionSessions.command(project, session, { op: "reparent", id: entry.id, parent: parent?.id ?? null }); },
  });
}

export async function deleteInstance(project: ProjectSnapshot, input: { ref: string; revision: string; dryRun?: boolean }) {
  const index = await projectIndex(project), { entry, scene } = instanceTarget(index, input.ref);
  const removed: string[] = [], pending = [entry.ref];
  while (pending.length) { const ref = pending.pop()!; removed.push(ref); pending.push(...(index.byRef.get(ref) as InstanceEntry | undefined)?.children ?? []); }
  const label = removed.length > 1 ? `delete ${entry.name} (${entry.ref}) and its ${removed.length - 1} descendant(s): ${removed.slice(1).join(", ")}` : `delete ${entry.name} (${entry.ref})`;
  return sceneEdit(project, scene, input.revision, "delete", label, source => replaceSceneSubtree(source, entry.id), { dryRun: input.dryRun, removal: true });
}

export type PlaceInput = { asset: string; scene: string; revision: string; parent?: string | null; name?: string; translation?: unknown; rotation_degrees?: unknown; scale?: unknown; dryRun?: boolean; source?: "mcp" | "ide" };
export async function placeAsset(project: ProjectSnapshot, input: PlaceInput) {
  const index = await projectIndex(project), scene = sceneTarget(index, input.scene), asset = resolveRef(index, input.asset);
  if (asset.type !== "asset" || !/\.(entity|prefab)\.toml$/.test(asset.path)) throw new RefError(`${input.asset} is not a placeable entity asset. Use list with type "assets" to find entity assets.`);
  let parent: InstanceEntry | null = null;
  if (input.parent) { const target = instanceTarget(index, input.parent); if (target.scene.ref !== scene.ref) throw new RefError("Parents must be in the same scene."); parent = target.entry; }
  const patch = { translation: vector(input.translation, "translation"), rotation_degrees: vector(input.rotation_degrees, "rotation_degrees"), scale: vector(input.scale, "scale") };
  const prefabSource = asset.path.endsWith(".prefab.toml") ? await readFile(await containedFile(project.root, asset.path, 256 * 1024), "utf8") : null;
  const prefab = prefabSource === null ? null : parsePrefab(prefabSource);
  const types = prefab ? await componentRegistry(project) : [];
  const id = randomUUID(), ref = `instance:${scene.id}/${id}`;
  const prefabTransform = prefab?.entities.find(item => item.id === prefab.root)?.transform as Partial<TransformValues> | undefined;
  const base = assetDefaults(project, prefab?.entities.find(item => item.id === prefab.root)?.asset ?? asset.path);
  const defaults = { translation: prefabTransform?.translation ?? base.translation, rotation_degrees: prefabTransform?.rotation_degrees ?? base.rotation_degrees, scale: prefabTransform?.scale ?? base.scale };
  return sceneEdit(project, scene, input.revision, "place_asset", `place ${asset.name} (${asset.ref}) in ${scene.name} as ${ref}${parent ? ` under ${parent.name}` : ""}`, source => {
    const document = parse(source) as { entities?: unknown[] };
    if ((document.entities?.length ?? 0) >= SCENE_MAX_INSTANCES) throw new RefError(`This scene already has the maximum of ${SCENE_MAX_INSTANCES} instances.`);
    let content = prefab ? placePrefab(source, prefab, types, local => local === prefab.root ? id : randomUUID()).content : `${source.replace(/\s*$/, "")}\n\n[[entities]]\nid = ${JSON.stringify(id)}\nasset = ${JSON.stringify(asset.path)}\n`;
    if (input.name !== undefined) content = replaceInstanceName(content, id, input.name);
    if (parent) content = replaceInstanceParent(content, id, parent.id);
    if (patch.translation || patch.rotation_degrees || patch.scale) content = replaceInstanceTransform(content, id, { translation: patch.translation ?? defaults.translation, rotation_degrees: patch.rotation_degrees ?? defaults.rotation_degrees, scale: patch.scale ?? defaults.scale });
    return content;
  }, { dryRun: input.dryRun, ref, source: input.source, check: prefabSource === null ? undefined : async () => {
    if (await readFile(await containedFile(project.root, asset.path, 256 * 1024), "utf8") !== prefabSource) throw new Error("Prefab changed during placement. Reload and retry.");
  } });
}

function assetDefaults(project: ProjectSnapshot, assetPath: string): TransformValues {
  const content = project.files.find(file => file.path === assetPath)?.content;
  const transform = (content ? (parse(content) as { transform?: Partial<TransformValues> }).transform : undefined) ?? {};
  return { translation: transform.translation ?? [0, 0, 0], rotation_degrees: transform.rotation_degrees ?? [0, 0, 0], scale: transform.scale ?? [1, 1, 1] };
}

type Settings = Record<string, unknown>;
const merge = (current: Settings, patch: Settings) => {
  const next: Settings = { ...current };
  for (const [key, value] of Object.entries(patch)) next[key] = value && typeof value === "object" && !Array.isArray(value) && current[key] && typeof current[key] === "object" && !Array.isArray(current[key]) ? { ...(current[key] as Settings), ...(value as Settings) } : value;
  return next;
};
function patchOrReset(value: unknown, name: string): Settings | null {
  if (value === null) return null;
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new RefError(`${name} must be an object of fields to change, or null to use the asset defaults.`);
  return value as Settings;
}

export async function setComponent(project: ProjectSnapshot, input: { ref: string; revision: string; component: "material" | "physics" | "camera"; settings: unknown; dryRun?: boolean }) {
  const index = await projectIndex(project), { entry, scene } = instanceTarget(index, input.ref);
  const settings = patchOrReset(input.settings, input.component);
  // A shader may be given as an asset ref; materials store its path.
  const patch = settings && input.component === "material" ? resolveFieldRefs(index, settings) as typeof settings : settings;
  const { instance } = await currentInstance(project, scene, entry.id);
  const label = patch === null ? `reset ${input.component} of ${entry.name} (${entry.ref}) to asset defaults` : `set ${input.component} ${Object.keys(patch).join(", ")} on ${entry.name} (${entry.ref})`;
  const options = { dryRun: input.dryRun, ref: entry.ref };
  if (input.component === "material") {
    if (instance.renderable === false) throw new RefError(`${entry.ref} is not renderable, so it has no material.`);
    const { edit, check } = await materialEdit(project, entry.id, patch === null ? null : merge((instance.material ?? {}) as Settings, patch));
    return sceneEdit(project, scene, input.revision, "set_material", label, edit, { ...options, check });
  }
  if (input.component === "physics") {
    const next = patch === null ? null : physicsSettings(merge((instance.physics ?? { enabled: true }) as Settings, patch));
    return sceneEdit(project, scene, input.revision, "set_physics", label, source => replaceInstancePhysics(source, entry.id, next), options);
  }
  if (!instance.camera) throw new RefError(`${entry.ref} is not a camera.`);
  const next = patch === null ? null : cameraSettings(merge(instance.camera as Settings, patch));
  return sceneEdit(project, scene, input.revision, "set_camera", label, source => replaceInstanceCamera(source, entry.id, next), options);
}

/** Add, change, disable or remove one registry component (e.g. emergent.light) on an instance; the Rust loader validates fields. */
export async function setObjectComponent(project: ProjectSnapshot, input: { ref: string; revision: string; type: string; fields: Record<string, unknown> | null; enabled?: boolean; dryRun?: boolean }) {
  const index = await projectIndex(project), { entry, scene } = instanceTarget(index, input.ref);
  if (input.fields !== null && (!input.fields || typeof input.fields !== "object" || Array.isArray(input.fields))) throw new RefError("fields must be an object of field values, or null to remove the component override.");
  // Brains, profiles and targets may be given as refs; scene files store paths and instance IDs.
  const patch = input.fields === null ? null : { ...resolveFieldRefs(index, input.fields), ...(input.enabled === undefined ? {} : { enabled: input.enabled }) } as ComponentPatch;
  const label = patch === null ? `remove ${input.type} override from ${entry.name} (${entry.ref})` : `set ${input.type} ${Object.keys(patch).join(", ") || "defaults"} on ${entry.name} (${entry.ref})`;
  const keepEmpty = await addedComponentType(project, input.type);
  return sceneEdit(project, scene, input.revision, "set_component", label, source => replaceInstanceComponent(source, entry.id, input.type, patch, keepEmpty), { dryRun: input.dryRun, ref: entry.ref });
}

export async function setEnvironment(project: ProjectSnapshot, input: { scene: string; revision: string; settings: unknown; dryRun?: boolean }) {
  const index = await projectIndex(project), scene = sceneTarget(index, input.scene);
  const patch = patchOrReset(input.settings, "environment") ?? {};
  const source = await readFile(await containedFile(project.root, scene.path, SCENE_MAX_BYTES), "utf8");
  const next = lightingSettings(merge(lightingSettings((parse(source) as Settings).environment) as Settings, patch));
  if (next.skybox && !projectImages(project).some(image => image.path === next.skybox && image.kind === "skybox")) throw new RefError("Choose a skybox image from this project's image library (kind skybox).");
  return sceneEdit(project, scene, input.revision, "set_environment", `set environment ${Object.keys(patch).join(", ") || "defaults"} on ${scene.name} (${scene.ref})`, current => replaceSceneLighting(current, next), { dryRun: input.dryRun, ref: scene.ref });
}

/** Sets the scene's uniform wind (speed m/s, from a compass bearing), or removes it with null. */
export async function setWind(project: ProjectSnapshot, input: { scene: string; revision: string; wind: unknown; dryRun?: boolean; source?: "ide" }) {
  const index = await projectIndex(project), scene = sceneTarget(index, input.scene);
  const wind = sceneWind(input.wind);
  const label = wind ? `set wind ${wind.speed} m/s from ${wind.from}° on ${scene.name} (${scene.ref})` : `remove wind from ${scene.name} (${scene.ref})`;
  return sceneEdit(project, scene, input.revision, "set_wind", label, current => replaceSceneWind(current, wind), { dryRun: input.dryRun, ref: scene.ref, ...(input.source ? { source: input.source } : {}) });
}

/** Gives durable IDs to files the index reports as missing-id: an `id` in TOML definitions, a descriptor for loose media. */
export async function assignIds(project: ProjectSnapshot, input: { dryRun?: boolean }): Promise<OpResult> {
  const index = await projectIndex(project);
  const changes: FileChange[] = [], notes: string[] = [], media = new Map<string, string[]>();
  for (const diagnostic of index.diagnostics.filter(item => item.code === "missing-id")) {
    const file = diagnostic.paths[0], content = project.files.find(item => item.path === file)?.content;
    if (/\.toml$/.test(file) && content) {
      const id = randomUUID(), header = content.match(/^schema_version\s*=.*$/m);
      const next = header ? content.replace(header[0], `${header[0]}\nid = "${id}"`) : `id = "${id}"\n${content}`;
      const before = parse(content) as Settings, after = parse(next) as Settings;
      if (JSON.stringify({ ...before, id }) !== JSON.stringify({ ...after, id }) || after.id !== id) { notes.push(`${file}: skipped; could not add an id safely.`); continue; }
      changes.push({ path: file, before: content, content: next }); notes.push(`${file} → asset:${id}`);
    } else if (/\.(png|jpe?g|webp|glb|mp4|webm)$/i.test(file)) {
      const directory = path.posix.dirname(file);
      media.set(directory, [...(media.get(directory) ?? []), file]);
    } else notes.push(`${file}: skipped; ${diagnostic.message}`);
  }
  for (const [directory, files] of media) {
    if (project.files.some(item => path.posix.dirname(item.path) === directory && item.path.endsWith(".asset.json"))) { notes.push(`${directory}: skipped; it already has a descriptor without an id. Add one in the TOML/JSON editor.`); continue; }
    const id = randomUUID(), descriptor = `${directory}/media.asset.json`, name = path.posix.basename(files[0]).replace(/\.[^.]+$/, "");
    changes.push({ path: descriptor, before: null, content: `${JSON.stringify({ schema_version: 1, id, kind: "media", name, files }, null, 2)}\n` });
    notes.push(`${files.join(", ")} → asset:${id} (${descriptor})`);
  }
  if (!changes.length) return { applied: false, dryRun: input.dryRun === true, changeId: null, summary: notes.length ? notes.join("\n") : "Every asset and scene already has a durable id.", diff: "", live: false };
  const summary = `assign durable ids to ${changes.length} file(s)`;
  const diff = changes.map(change => change.before === null ? `+++ ${change.path} (new)\n${change.content}` : `--- ${change.path}\n${textDiff(change.before, change.content)}`).join("\n");
  if (input.dryRun) return { applied: false, dryRun: true, changeId: null, summary: `Would ${summary}:\n${notes.join("\n")}`, diff, live: false };
  await withSceneLock(async () => {
    // applyChanges only publishes Rust/TOML for AI writes; new media descriptors use a create-only path instead.
    const descriptors = changes.filter(change => change.before === null), edits = changes.filter(change => change.before !== null);
    const created: string[] = [];
    try {
      for (const change of descriptors) { await createDescriptor(project, change.path, change.content); created.push(change.path); }
      if (edits.length) await applyChanges(project, edits);
    } catch (error) {
      for (const file of created) await unlink(path.join(project.root, file)).catch(() => undefined);
      throw error;
    }
  });
  const changeId = await journal(project, "assign_ids", summary, changes);
  publishProjectEvent({ type: "changed", projectId: project.id, changeId, source: "mcp", summary, paths: changes.map(change => change.path), scenes: [], live: false });
  return { applied: true, dryRun: false, changeId, summary: `${summary}:\n${notes.join("\n")}`, diff, live: false };
}

/** Reverts one journalled change, refusing if any of its files changed since. */
export async function undoChange(project: ProjectSnapshot, input: { changeId?: string; dryRun?: boolean }): Promise<OpResult> {
  const entries = await readJournal(project);
  const target = input.changeId ? entries.find(entry => entry.id === input.changeId) : entries.find(entry => !entry.undoneBy && !entry.undoOf);
  if (!target) throw new RefError(input.changeId ? `No change ${input.changeId} in this project's MCP history.` : "There is no MCP change to undo.");
  if (target.undoneBy) throw new RefError(`${target.id} was already undone by ${target.undoneBy}.`);
  if (target.undoOf) throw new RefError(`${target.id} is itself an undo; make the change again instead of undoing the undo.`);
  const reverse = target.changes.map(change => ({ path: change.path, before: change.content, content: change.before }));
  for (const change of reverse) {
    const target = await containedFile(project.root, change.path, 16 * 1024 * 1024).catch(() => null);
    const current = target ? await readFile(target, "utf8").catch(() => null) : null;
    if (current !== change.before) throw new RefError(`${change.path} changed after ${input.changeId ?? "the last change"}; undo refused so newer work is not overwritten.`);
    if (change.content !== null && (change.path.endsWith(".scene.toml") || change.path.endsWith(".prefab.toml"))) await validateProjectScene(project, change.path, change.content);
  }
  const summary = `undo "${target.summary}"`;
  const diff = reverse.map(change => change.content === null ? `--- ${change.path} (removed)` : `--- ${change.path}\n${textDiff(change.before, change.content)}`).join("\n");
  if (input.dryRun) return { applied: false, dryRun: true, changeId: null, summary: `Would ${summary}. Files are unchanged since; nothing written.`, diff, live: false };
  await withSceneLock(async () => {
    const restores = reverse.flatMap(change => change.content === null ? [] : [{ path: change.path, before: change.before, content: change.content }]);
    if (restores.length) await applyChanges(project, restores);
    for (const created of reverse.filter(change => change.content === null)) await unlink(await containedFile(project.root, created.path, 16 * 1024 * 1024));
  });
  const changeId = await journal(project, "undo", summary, reverse.map(change => ({ path: change.path, before: change.before, content: change.content ?? "" })), target.id);
  await markUndone(project, target.id, changeId);
  const scenes = reverse.map(change => change.path).filter(item => item.endsWith(".scene.toml"));
  publishProjectEvent({ type: "changed", projectId: project.id, changeId, source: "mcp", summary, paths: reverse.map(change => change.path), scenes, live: false });
  return { applied: true, dryRun: false, changeId, summary: `${summary}.`, diff, live: false };
}

async function createDescriptor(project: ProjectSnapshot, relative: string, content: string) {
  const parts = relative.split("/");
  if (path.posix.basename(relative) !== "media.asset.json" || !project.assetRoots.some(root => relative.startsWith(`${root}/`)) || parts.some(part => !part || part === "." || part === "..")) throw new RefError(`Refusing to create ${relative}.`);
  const root = await realpath(project.root), directory = await realpath(path.join(root, path.posix.dirname(relative)));
  if (!directory.startsWith(`${root}${path.sep}`)) throw new RefError(`${relative} is outside the project.`);
  JSON.parse(content);
  await writeFile(path.join(directory, "media.asset.json"), content, { flag: "wx", mode: 0o644 });
}

export async function changeHistory(project: ProjectSnapshot, limit = 20) {
  return (await readJournal(project)).slice(0, Math.min(Math.max(limit, 1), 100)).map(entry => ({ changeId: entry.id, at: entry.at, tool: entry.tool, summary: entry.summary, paths: entry.changes.map(change => change.path), undoneBy: entry.undoneBy ?? null, undoOf: entry.undoOf ?? null }));
}

const lockState = globalThis as typeof globalThis & { emergentScenePlacementActive?: boolean };
export async function withSceneLock<T>(action: () => Promise<T>) {
  if (lockState.emergentScenePlacementActive) throw new RefError("Another scene edit is being saved. Try again shortly.");
  lockState.emergentScenePlacementActive = true;
  try { return await action(); } finally { lockState.emergentScenePlacementActive = false; }
}

function journalDirectory(project: ProjectSnapshot) { return path.join(settingsDirectory(), "mcp-changes", project.id.replace(/[^a-z0-9_.-]/gi, "_")); }
/** Records an applied MCP change so history and undo cover it. */
export async function journal(project: ProjectSnapshot, tool: string, summary: string, changes: FileChange[], undoOf?: string) {
  const id = randomUUID(), at = new Date().toISOString(), directory = journalDirectory(project);
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const entry: JournalEntry = { id, projectId: project.id, root: project.root, at, tool, summary, changes, undoOf };
  await writeFile(path.join(directory, `${at.replace(/[:.]/g, "-")}_${id}.json`), JSON.stringify(entry), { mode: 0o600, flag: "wx" });
  const files = (await readdir(directory)).filter(name => name.endsWith(".json")).sort();
  for (const old of files.slice(0, Math.max(0, files.length - JOURNAL_LIMIT))) await rm(path.join(directory, old), { force: true });
  return id;
}
async function readJournal(project: ProjectSnapshot): Promise<(JournalEntry & { file: string })[]> {
  const directory = journalDirectory(project);
  const files = (await readdir(directory).catch(() => [] as string[])).filter(name => name.endsWith(".json")).sort().reverse();
  const entries: (JournalEntry & { file: string })[] = [];
  for (const file of files) {
    try { const entry = JSON.parse(await readFile(path.join(directory, file), "utf8")) as JournalEntry; if (entry.projectId === project.id && entry.root === project.root) entries.push({ ...entry, file }); }
    catch { /* A damaged journal entry is skipped; it cannot be undone safely. */ }
  }
  return entries;
}
async function markUndone(project: ProjectSnapshot, id: string, by: string) {
  const entry = (await readJournal(project)).find(item => item.id === id);
  if (!entry) return;
  const { file, ...stored } = entry;
  await writeFile(path.join(journalDirectory(project), file), JSON.stringify({ ...stored, undoneBy: by }), { mode: 0o600 });
}

/** Compact line diff: unchanged head/tail trimmed, at most 80 changed lines per side. */
export function textDiff(before: string, after: string) {
  const a = before.split("\n"), b = after.split("\n");
  let start = 0; while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length, endB = b.length; while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) { endA--; endB--; }
  const removed = a.slice(start, endA), added = b.slice(start, endB), clip = (lines: string[], mark: string) => [...lines.slice(0, 80).map(line => `${mark} ${line}`), ...(lines.length > 80 ? [`${mark} … ${lines.length - 80} more line(s)`] : [])];
  return [`@@ line ${start + 1}`, ...clip(removed, "-"), ...clip(added, "+")].join("\n");
}

export type { ProjectIndex };
