import { SCENE_MAX_BYTES, SCENE_MAX_INSTANCES } from "./scene-limits";
import { parse, stringify } from "smol-toml";
import { sceneHierarchy } from "./scene-hierarchy";
import type { ComponentType } from "./components";

type Entry = Record<string, unknown> & { id: string; asset: string; parent?: string; components?: Record<string, unknown>[]; brain_bindings?: Record<string, string> };
export type Prefab = { schema_version: 1; id: string; name: string; root: string; entities: Entry[] };
const identifier = (value: unknown): value is string => typeof value === "string" && /^[a-z0-9_.-]{1,128}$/i.test(value);
export function prefabName(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || [...value.trim()].length > 64 || /\p{Cc}/u.test(value)) throw new Error("Name the prefab using 1–64 printable characters.");
  return value.trim();
}
function entries(value: unknown, limit = 1024): Entry[] {
  if (!Array.isArray(value) || !value.length || value.length > limit) throw new Error(`A hierarchy needs 1–${limit} entities.`);
  for (const item of value) if (!item || typeof item !== "object" || !identifier(item.id) || typeof item.asset !== "string" || !item.asset.endsWith(".entity.toml") || item.parent !== undefined && !identifier(item.parent)) throw new Error("Prefab members need IDs and entity assets.");
  sceneHierarchy(value.map(item => ({ id: item.id, parent: item.parent ?? null })));
  return structuredClone(value) as Entry[];
}
export function parsePrefab(source: string): Prefab {
  if (new TextEncoder().encode(source).length > 256 * 1024) throw new Error("Prefab exceeds 256 KiB.");
  const value = parse(source);
  if (value.schema_version !== 1 || !identifier(value.id) || !identifier(value.root) || Object.keys(value).some(key => !["schema_version", "id", "name", "root", "entities"].includes(key))) throw new Error("Unsupported prefab schema.");
  const members = entries(value.entities), roots = members.filter(item => !item.parent);
  if (roots.length !== 1 || roots[0].id !== value.root) throw new Error("Prefab requires exactly one root matching its root ID.");
  return { schema_version: 1, id: value.id, name: prefabName(value.name), root: value.root, entities: members };
}

/** Resolve only registry-declared entity fields; paths, names and group labels remain unchanged. */
function remap(members: Entry[], ids: Map<string, string>, types: ComponentType[], externalIds?: Set<string>): Entry[] {
  const target = (id: unknown): string => {
    if (typeof id === "string" && externalIds?.has(id) && !ids.has(id)) return id;
    if (typeof id !== "string" || !ids.has(id)) throw new Error(`Prefab references external object '${String(id)}'. Include it under the selected root first.`);
    return ids.get(id)!;
  };
  return members.map(member => {
    const item = structuredClone(member);
    item.id = target(member.id);
    if (item.parent) item.parent = target(item.parent);
    for (const component of item.components ?? []) {
      const schema = types.find(type => type.id === component.type);
      if (!schema) throw new Error(`Unknown component '${component.type}'. Reload the component registry.`);
      for (const field of schema.fields) {
        const value = component[field.name] ?? field.default;
        if (value === undefined) continue;
        if (field.kind === "entity") component[field.name] = target(value);
        if (field.kind === "entities") {
          if (!Array.isArray(value)) throw new Error(`Invalid entity list ${field.name}.`);
          component[field.name] = value.map(target);
        }
      }
    }
    const hasBrain = item.components?.some(component => component.type === "emergent.brain");
    if (member.brain_bindings) item.brain_bindings = Object.fromEntries(Object.entries(member.brain_bindings).map(([local, id]) => [local, target(id)]));
    else if (hasBrain) item.brain_bindings = Object.fromEntries([...(externalIds ? [...externalIds].map(id => [id, id]) : []), ...ids]);
    return item;
  });
}

/** Copy a scene subtree; internal links follow the copy, external links keep their targets. */
export function duplicateSceneObject(source: string, id: string, types: ComponentType[], freshId: (local: string) => string) {
  const document = parse(source), all = entries(document.entities, SCENE_MAX_INSTANCES);
  const ordered = sceneHierarchy(all.map(item => ({ ...item, parent: item.parent ?? null })));
  const start = ordered.findIndex(row => row.instance.id === id);
  if (start < 0) throw new Error("Scene instance no longer exists.");
  let end = start + 1;
  while (end < ordered.length && ordered[end].depth > ordered[start].depth) end++;
  const selected = ordered.slice(start, end).map(row => all.find(item => item.id === row.instance.id)!);
  if (all.length + selected.length > SCENE_MAX_INSTANCES) throw new Error(`Duplication would exceed ${SCENE_MAX_INSTANCES} scene entities.`);
  const ids = new Map(selected.map(item => [item.id, freshId(item.id)])), existing = new Set(all.map(item => item.id));
  if (new Set(ids.values()).size !== ids.size || [...ids.values()].some(id => !identifier(id) || existing.has(id))) throw new Error("Duplicate instance IDs must be fresh and unique.");
  const members = remap(selected, ids, types, existing);
  const label = typeof selected[0].name === "string" ? selected[0].name : id;
  const base = label.replace(/ \(\d+\)$/, "");
  let number = 0;
  for (const item of all) {
    const name = typeof item.name === "string" ? item.name : item.id;
    const suffix = name.match(/^(.*) \((\d+)\)$/);
    if (suffix?.[1] === base) number = Math.max(number, Number(suffix[2]));
  }
  if (!Number.isSafeInteger(number + 1)) throw new Error("Copy number exceeds the supported range.");
  members[0].name = `${base} (${number + 1})`;
  const content = source.trimEnd() + "\n\n" + stringify({ entities: members });
  if (new TextEncoder().encode(content).length > SCENE_MAX_BYTES) throw new Error("Duplicated scene exceeds the scene size limit.");
  return { content, id: ids.get(id)! };
}

export function capturePrefab(source: string, root: string, name: unknown, id: string, types: ComponentType[]): Prefab {
  const scene = parse(source), all = entries(scene.entities);
  const ordered = sceneHierarchy(all.map(item => ({ ...item, parent: item.parent ?? null })));
  const start = ordered.findIndex(row => row.instance.id === root);
  if (start < 0) throw new Error("Select a scene object first.");
  let end = start + 1;
  while (end < ordered.length && ordered[end].depth > ordered[start].depth) end++;
  const selected = ordered.slice(start, end).map(row => { const member = all.find(item => item.id === row.instance.id)!; return structuredClone(member); });
  delete selected[0].parent;
  const members = remap(selected, new Map(selected.map(item => [item.id, item.id])), types);
  members[0].name = prefabName(name);
  members[0].transform = { ...(members[0].transform as object ?? {}), translation: [0, 0, 0], rotation_degrees: [0, 0, 0] };
  return parsePrefab(stringify({ schema_version: 1, id, name: prefabName(name), root, entities: members }));
}

export function placePrefab(source: string, prefab: Prefab, types: ComponentType[], freshId: (local: string) => string) {
  const document = parse(source);
  if (!Array.isArray(document.entities) || document.entities.length + prefab.entities.length > SCENE_MAX_INSTANCES) throw new Error(`Placement would exceed ${SCENE_MAX_INSTANCES} scene entities.`);
  const ids = new Map(prefab.entities.map(item => [item.id, freshId(item.id)]));
  const fresh = new Set(ids.values());
  if (fresh.size !== ids.size || [...fresh].some(id => !identifier(id)) || document.entities.some(item => fresh.has((item as {id:string}).id))) throw new Error("Prefab instance IDs must be fresh and unique.");
  const members = remap(prefab.entities, ids, types);
  const root = members.find(item => item.id === ids.get(prefab.root))!;
  root.name = prefab.name;
  root.prefab_label = prefab.name;
  // Append only entity tables, preserving the scene's comments and trailing environment.
  const content = source.replace(/^entities\s*=\s*\[\s*\][^\S\r\n]*(?:#[^\r\n]*)?$/m, "").trimEnd() + "\n\n" + stringify({ entities: members });
  if (new TextEncoder().encode(content).length > SCENE_MAX_BYTES) throw new Error("Destination scene exceeds 2 MiB.");
  return { content, root: root.id };
}
