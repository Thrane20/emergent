import "server-only";
import { createHash } from "node:crypto";
import path from "node:path";
import type { ProjectFile, ProjectSnapshot } from "./project";
import { parse } from "smol-toml";

/**
 * Durable, name-addressable view of a linked project for AI tools.
 * Refs use authored IDs only (never file paths or runtime SlotMap keys), so moving a file keeps its ref.
 *   asset:<asset id>   scene:<scene id>   instance:<scene id>/<instance id>
 * Names are not unique: callers resolve names with `findEntries`, then act on refs.
 */
export type AssetEntry = { type: "asset"; ref: string; id: string; name: string; kind: string; path: string; files: string[]; revision: string };
export type SceneEntry = { type: "scene"; ref: string; id: string; name: string; path: string; revision: string | null; instances: number };
export type InstanceEntry = {
  type: "instance"; ref: string; id: string; name: string; scene: string; sceneName: string; hierarchy: string;
  parent: string | null; children: string[]; asset: string | null; assetPath: string; kind: string; translation: number[];
};
type AuthoredInstance = { id: string; name: string; asset: string; parent: string | null; translation: number[] };
export type IndexEntry = AssetEntry | SceneEntry | InstanceEntry;
export type IndexDiagnostic = { code: "duplicate-id" | "missing-id" | "missing-asset" | "invalid-scene"; message: string; paths: string[] };
export type ProjectIndex = { projectId: string; projectName: string; entryScene: string | null; assets: AssetEntry[]; scenes: SceneEntry[]; instances: InstanceEntry[]; diagnostics: IndexDiagnostic[]; byRef: Map<string, IndexEntry> };

const SIDECAR = /\.(heights|paint)\.toml$|\.library\.json$/;
const MEDIA = /\.(png|jpe?g|webp|glb|mp4|webm)$/i;
const DEFINITIONS: [RegExp, string][] = [[/\.entity\.toml$/, "entity"], [/\.material\.toml$/, "material"], [/\.noise\.toml$/, "filter"], [/\.tween\.toml$/, "tween"], [/\.structure\.toml$/, "structure"]];
const state = globalThis as typeof globalThis & { emergentProjectIndex?: { key: string; index: ProjectIndex } };

export function assetRef(id: string) { return `asset:${id}`; }
export function sceneRef(id: string) { return `scene:${id}`; }
export function instanceRef(sceneId: string, id: string) { return `instance:${sceneId}/${id}`; }

function header(content: string) { return content.split(/^\s*\[/m)[0]; }
function field(text: string, key: string) { return text.match(new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, "m"))?.[1]; }
function digest(text: string) { return createHash("sha256").update(text).digest("hex"); }
function inAssets(project: ProjectSnapshot, file: ProjectFile) { return project.assetRoots.some(root => file.path.startsWith(`${root}/`)); }

/** Builds (or reuses) the index; the cache key covers every file's path, size and mtime. */
export async function projectIndex(project: ProjectSnapshot): Promise<ProjectIndex> {
  const key = digest(JSON.stringify([project.id, project.root, project.files.map(file => [file.path, file.bytes, file.modifiedAt])]));
  if (state.emergentProjectIndex?.key === key) return state.emergentProjectIndex.index;
  const index = await buildIndex(project);
  state.emergentProjectIndex = { key, index };
  return index;
}

async function buildIndex(project: ProjectSnapshot): Promise<ProjectIndex> {
  const diagnostics: IndexDiagnostic[] = [];
  const assets: AssetEntry[] = [];
  const owned = new Set<string>();
  const files = project.files.filter(file => inAssets(project, file));
  for (const file of files) {
    if (SIDECAR.test(file.path) || file.path.endsWith(".scene.toml") || !file.content) continue;
    const definition = DEFINITIONS.find(([pattern]) => pattern.test(file.path));
    if (definition) {
      const top = header(file.content), id = field(top, "id");
      owned.add(file.path);
      if (!id) { diagnostics.push({ code: "missing-id", message: `${file.path} has no top-level id.`, paths: [file.path] }); continue; }
      assets.push({ type: "asset", ref: assetRef(id), id, name: field(top, "name") ?? id, kind: field(top, "kind") ?? definition[1], path: file.path, files: [file.path, ...sidecars(files, file.path)], revision: digest(file.content) });
    } else if (file.path.endsWith(".asset.json")) {
      let descriptor: Record<string, unknown>;
      try { descriptor = JSON.parse(file.content); } catch { diagnostics.push({ code: "missing-id", message: `${file.path} is not valid JSON.`, paths: [file.path] }); continue; }
      const directory = path.posix.dirname(file.path);
      // Generated bundles keep one descriptor per directory; it owns that directory's media.
      const members = files.filter(item => path.posix.dirname(item.path) === directory && item.path !== file.path).map(item => item.path);
      owned.add(file.path); for (const member of members) owned.add(member);
      if (typeof descriptor.id !== "string" || !descriptor.id) { diagnostics.push({ code: "missing-id", message: `${file.path} has no id.`, paths: [file.path] }); continue; }
      const kind = typeof descriptor.kind === "string" ? descriptor.kind : typeof descriptor.output === "string" ? descriptor.output : path.basename(file.path, ".asset.json");
      const name = typeof descriptor.name === "string" ? descriptor.name : path.basename(members.find(item => MEDIA.test(item)) ?? directory).replace(/\.[^.]+$/, "");
      assets.push({ type: "asset", ref: assetRef(descriptor.id), id: descriptor.id, name, kind, path: file.path, files: [file.path, ...members], revision: digest(file.content) });
    }
  }
  for (const file of files) if (MEDIA.test(file.path) && !owned.has(file.path)) diagnostics.push({ code: "missing-id", message: `${file.path} has no descriptor with a durable id.`, paths: [file.path] });

  const scenes: SceneEntry[] = [];
  const instances: InstanceEntry[] = [];
  const assetByPath = new Map(assets.map(asset => [asset.path, asset]));
  const contents = new Map(files.map(file => [file.path, file.content]));
  for (const file of files.filter(item => item.path.endsWith(".scene.toml") && item.content)) {
    const top = header(file.content!), id = field(top, "id");
    if (!id) { diagnostics.push({ code: "missing-id", message: `${file.path} has no top-level id.`, paths: [file.path] }); continue; }
    const scene: SceneEntry = { type: "scene", ref: sceneRef(id), id, name: field(top, "name") ?? id, path: file.path, revision: null, instances: 0 };
    scenes.push(scene);
    try {
      // A light authored read: full scenePlacement hashes every texture/heightfield and is reserved for describe/edits.
      const authored = authoredInstances(file.content!, path => contents.get(path));
      scene.revision = digest(file.content!); scene.instances = authored.length;
      instances.push(...sceneInstances(scene, authored, assetByPath, diagnostics));
    } catch (error) {
      diagnostics.push({ code: "invalid-scene", message: `${file.path}: ${error instanceof Error ? error.message : "could not be read."}`, paths: [file.path] });
    }
  }

  const byRef = new Map<string, IndexEntry>();
  const seen = new Map<string, string[]>();
  for (const entry of [...assets, ...scenes, ...instances]) {
    const location = entry.type === "instance" ? `${entry.scene} ${entry.id}` : entry.path;
    seen.set(entry.ref, [...(seen.get(entry.ref) ?? []), location]);
    byRef.set(entry.ref, entry);
  }
  for (const [ref, locations] of seen) if (locations.length > 1) {
    byRef.delete(ref); // An ambiguous durable ID must never silently resolve to one of its owners.
    diagnostics.push({ code: "duplicate-id", message: `${ref} is declared ${locations.length} times; it cannot be used until the IDs are unique.`, paths: locations });
  }
  const entryScene = scenes.find(scene => scene.path === project.entryScene)?.ref ?? null;
  return { projectId: project.id, projectName: project.name, entryScene, assets, scenes, instances, diagnostics, byRef };
}

function sidecars(files: ProjectFile[], definition: string) {
  const stem = definition.replace(/\.entity\.toml$/, "");
  return files.filter(file => file.path !== definition && file.path.startsWith(`${stem}.`) && SIDECAR.test(file.path)).map(file => file.path);
}

function sceneInstances(scene: SceneEntry, source: AuthoredInstance[], assetByPath: Map<string, AssetEntry>, diagnostics: IndexDiagnostic[]): InstanceEntry[] {
  const byId = new Map(source.map(item => [item.id, item]));
  const children = new Map<string, string[]>();
  for (const item of source) if (item.parent) children.set(item.parent, [...(children.get(item.parent) ?? []), item.id]);
  return source.map(item => {
    const names: string[] = [];
    // Bounded walk: the loader already rejects cycles, and depth cannot exceed the instance count.
    for (let cursor: AuthoredInstance | undefined = item, depth = 0; cursor && depth <= source.length; cursor = cursor.parent ? byId.get(cursor.parent) : undefined, depth++) names.unshift(cursor.name);
    const asset = assetByPath.get(item.asset);
    if (!asset) diagnostics.push({ code: "missing-asset", message: `${scene.path} instance ${item.id} references ${item.asset}, which has no durable asset id.`, paths: [scene.path, item.asset] });
    return {
      type: "instance", ref: instanceRef(scene.id, item.id), id: item.id, name: item.name, scene: scene.ref, sceneName: scene.name, hierarchy: names.join("/"),
      parent: item.parent ? instanceRef(scene.id, item.parent) : null, children: (children.get(item.id) ?? []).map(child => instanceRef(scene.id, child)),
      asset: asset?.ref ?? null, assetPath: item.asset, kind: asset?.kind ?? "entity", translation: item.translation,
    } satisfies InstanceEntry;
  });
}

/** Names follow the IDE: instance name override, then asset name, then instance ID. */
function authoredInstances(source: string, read: (path: string) => string | null | undefined): AuthoredInstance[] {
  const document = parse(source) as { schema_version?: unknown; entities?: unknown };
  if (document.schema_version !== 1 || !Array.isArray(document.entities) || document.entities.length > 1024) throw new Error("Scene must use schema 1 and at most 1024 instances.");
  const assets = new Map<string, Record<string, unknown> | null>();
  return document.entities.map(value => {
    const item = value as Record<string, unknown>;
    if (typeof item.id !== "string" || typeof item.asset !== "string") throw new Error("Scene instances need IDs and asset references.");
    if (!assets.has(item.asset)) { const text = read(item.asset); assets.set(item.asset, text ? parse(text) as Record<string, unknown> : null); }
    const asset = assets.get(item.asset), local = (item.transform ?? asset?.transform) as { translation?: unknown } | undefined;
    const name = typeof item.name === "string" ? item.name : typeof asset?.name === "string" && asset.name.trim() ? asset.name : item.id;
    const translation = Array.isArray(local?.translation) && local.translation.length === 3 ? local.translation.map(Number) : [0, 0, 0];
    return { id: item.id, name, asset: item.asset, parent: typeof item.parent === "string" ? item.parent : null, translation };
  });
}

export class RefError extends Error {}

/** Resolves a durable ref; stale, malformed or ambiguous refs fail rather than matching a replacement. */
export function resolveRef(index: ProjectIndex, ref: string): IndexEntry {
  if (!/^(asset|scene):[^\s/]{1,160}$|^instance:[^\s/]{1,160}\/[a-z0-9_.-]{1,128}$/i.test(ref)) throw new RefError(`"${ref}" is not a ref. Use find to get one (asset:<id>, scene:<id> or instance:<scene id>/<instance id>).`);
  const entry = index.byRef.get(ref);
  if (entry) return entry;
  if (index.diagnostics.some(item => item.code === "duplicate-id" && item.message.startsWith(`${ref} `))) throw new RefError(`${ref} is ambiguous because its ID is duplicated. Fix the duplicate first.`);
  throw new RefError(`${ref} does not exist in this project. It may have been deleted or renamed; use find again.`);
}

export type FindOptions = { type?: "asset" | "scene" | "instance" | "any"; scene?: string; limit?: number };
export type FindResult = { entry: IndexEntry; score: number };

/** Ranked name lookup. Exact names beat prefixes, words, substrings and subsequences; IDs and kinds also match. */
export function findEntries(index: ProjectIndex, query: string, options: FindOptions = {}): FindResult[] {
  const needle = normalize(query);
  if (!needle) return [];
  const pool: IndexEntry[] = [...index.instances, ...index.assets, ...index.scenes]
    .filter(entry => options.type === undefined || options.type === "any" || entry.type === options.type)
    .filter(entry => !options.scene || (entry.type === "instance" && entry.scene === options.scene));
  const results: FindResult[] = [];
  for (const entry of pool) {
    const score = Math.max(match(needle, normalize(entry.name)), match(needle, normalize(entry.id)) - 5, entry.type === "instance" ? match(needle, normalize(entry.hierarchy)) - 10 : 0, entry.type !== "scene" && needle === normalize(entry.kind) ? 30 : 0);
    if (score > 0 && index.byRef.has(entry.ref)) results.push({ entry, score: score + (entry.type === "instance" ? 2 : 0) });
  }
  return results.sort((a, b) => b.score - a.score || a.entry.name.localeCompare(b.entry.name)).slice(0, Math.min(Math.max(options.limit ?? 10, 1), 20));
}

function normalize(text: string) { return text.toLowerCase().replace(/[_\-.]+/g, " ").replace(/\s+/g, " ").trim(); }
function match(needle: string, value: string) {
  if (!value) return 0;
  if (value === needle) return 100;
  if (value.startsWith(needle)) return 80;
  if (value.split(" ").some(word => word.startsWith(needle))) return 70;
  if (value.includes(needle)) return 55;
  let position = 0;
  for (const character of value) if (character === needle[position]) position++;
  return position === needle.length && needle.length >= 3 ? 25 : 0;
}
