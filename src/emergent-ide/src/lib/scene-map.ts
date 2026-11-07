import "server-only";
import { randomUUID } from "node:crypto";
import { access, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { parse } from "smol-toml";
import { authoredComponents, type AuthoredComponent } from "./components";
import { editScenePath } from "./project-ops";
import { readProjectSnapshot, type ProjectSnapshot } from "./project";
import { replaceInstanceComponent, replaceInstanceName, replaceInstanceParent, replaceInstanceTransform, replaceSceneSubtree, scenePlacement } from "./scene-placement";
import { gameTool } from "./scene-preview";
import { invertAffine, localZone, transformPoint, type AirportView, type MapItemInput, type MapLayoutInstance, type MapTopDown, type MapWind, type SceneMapModel } from "./scene-map-types";
import { projectIndex } from "./project-index";
import { setWind } from "./project-ops";

type MapLayout = { instances: MapLayoutInstance[]; airports: AirportView[]; wind: MapWind | null; topDown?: MapTopDown };
const layouts = new Map<string, Promise<MapLayout>>();
/** World matrices and top-down outlines from the engine, cached per scene revision. */
async function mapLayout(project: ProjectSnapshot, scenePath: string, revision: string) {
  const key = `${project.root}\0${scenePath}\0${revision}`;
  let cached = layouts.get(key);
  if (!cached) {
    cached = gameTool(project, ["--map-layout", scenePath], 16 * 1024 * 1024).then(text => {
      const parsed = parse(text) as { instances?: unknown[]; airports?: Record<string, unknown>[]; wind?: { speed?: unknown; from?: unknown }; top_down?: { min?: unknown; max?: unknown; image?: unknown } };
      const point = (value: unknown) => (Array.isArray(value) ? [Number(value[0]), Number(value[1])] : undefined) as [number, number] | undefined;
      const airports: AirportView[] = (parsed.airports ?? []).map(airport => ({
        aerodrome: typeof airport.aerodrome === "string" ? airport.aerodrome : undefined, towered: airport.towered === true, holdingFix: point(airport.holding_fix),
        runways: ((airport.runways as Record<string, unknown>[]) ?? []).map(runway => ({
          id: String(runway.id), heading: Number(runway.heading), active: runway.active === true, rightCircuit: runway.right_circuit === true,
          threshold: point(runway.threshold)!, farEnd: point(runway.far_end)!, circuit: ((runway.circuit as unknown[]) ?? []).map(p => point(p)!),
          patternAltitude: typeof runway.pattern_altitude === "number" ? runway.pattern_altitude : undefined,
        })),
      }));
      const wind = parsed.wind && typeof parsed.wind.speed === "number" ? { speed: parsed.wind.speed, from: Number(parsed.wind.from ?? 0) } : null;
      const instances = (parsed.instances ?? []).map(value => {
        const item = value as Record<string, unknown>;
        return { id: String(item.id), matrix: (item.matrix as number[]).map(Number), outline: ((item.outline as number[][]) ?? []).map(point => [Number(point[0]), Number(point[1])] as [number, number]), top: typeof item.top === "number" ? item.top : undefined };
      });
      const top = parsed.top_down, min = point(top?.min), max = point(top?.max);
      const topDown = min && max && typeof top?.image === "string" && top.image.startsWith("data:image/png;base64,") ? { min, max, image: top.image } : undefined;
      return { instances, airports, wind, topDown };
    });
    if (layouts.size > 16) layouts.delete(layouts.keys().next().value!);
    layouts.set(key, cached);
    cached.catch(() => layouts.delete(key));
  }
  return cached;
}

/** Everything the map editor draws for one scene. */
export async function sceneMap(project: ProjectSnapshot, scenePath: string): Promise<SceneMapModel> {
  const placement = await scenePlacement(project, scenePath);
  const layout = await mapLayout(project, scenePath, placement.revision);
  const byId = new Map(layout.instances.map(item => [item.id, item]));
  return {
    scene: scenePath,
    revision: placement.revision,
    root: scenePath.endsWith(".prefab.toml") ? placement.instances.find(instance => instance.parent === null)?.id ?? null : null,
    airports: layout.airports,
    wind: layout.wind,
    topDown: layout.topDown,
    instances: placement.instances.map(instance => ({ id: instance.id, name: instance.name ?? instance.id, parent: instance.parent, asset: instance.asset, objectKind: instance.objectKind, components: instance.components ?? [], transform: instance.transform, matrix: byId.get(instance.id)?.matrix ?? [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], outline: byId.get(instance.id)?.outline ?? [], top: byId.get(instance.id)?.top })),
  };
}

/** Transform-only entity assets the map places; written once per project. */
const MAP_ASSETS = { waypoint: ["map-waypoint", "Waypoint"], zone: ["map-zone", "Zone"], group: ["map-group", "Waypoint group"] } as const;
async function mapAsset(project: ProjectSnapshot, kind: keyof typeof MAP_ASSETS) {
  const [id, name] = MAP_ASSETS[kind], root = project.assetRoots[0];
  if (!root) throw new Error("This project has no asset root for map items.");
  const relative = `${root}/entities/${id}.entity.toml`, file = path.join(project.root, relative);
  try { await access(file); } catch {
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, `schema_version = 1\nid = ${JSON.stringify(id)}\nname = ${JSON.stringify(name)}\nkind = "entity"\ntemplate = "marker.point"\nruntime_status = "available"\n\n# A map item: position only. Its waypoint or zone component is on the scene instance.\n[transform]\ntranslation = [0.0, 0.0, 0.0]\nrotation_degrees = [0.0, 0.0, 0.0]\nscale = [1.0, 1.0, 1.0]\n`, { flag: "wx" }).catch(error => { if (error?.code !== "EEXIST") throw error; });
  }
  return relative;
}

const round = (n: number) => Math.round(n * 1000) / 1000;
function instanceIn(model: SceneMapModel, id: string) {
  const instance = model.instances.find(item => item.id === id);
  if (!instance) throw new Error("That map item no longer exists. Reload the map.");
  return instance;
}
/** A world XZ point (at world height `y`) in `parent`'s local space, or world space without one. */
function localPoint(model: SceneMapModel, parent: string | null, x: number, y: number, z: number) {
  if (!parent) return [x, y, z];
  return transformPoint(invertAffine(instanceIn(model, parent).matrix), [x, y, z]);
}
function appendEntity(source: string, id: string, asset: string) {
  return `${source.replace(/\s*$/, "")}\n\n[[entities]]\nid = ${JSON.stringify(id)}\nasset = ${JSON.stringify(asset)}\n`;
}
function components(source: string, id: string) {
  const document = parse(source) as { entities?: Record<string, unknown>[] };
  return authoredComponents(document.entities?.find(entity => entity.id === id)?.components);
}

/**
 * Adds waypoints and zones in one undoable edit. A `path` of two or more waypoints is put
 * under a new group object and linked in order.
 */
export async function addMapItems(project: ProjectSnapshot, input: { scene: string; revision: string; parent: string | null; items: MapItemInput[]; path?: boolean; name?: string }) {
  if (!input.items.length || input.items.length > 256) throw new Error("Place 1-256 map items at a time.");
  const model = await sceneMap(project, input.scene);
  if (model.revision !== input.revision) throw new Error("The scene changed. Reload the map and try again.");
  const parent = input.parent ?? model.root;
  if (parent) instanceIn(model, parent);
  const assets = { waypoint: await mapAsset(project, "waypoint"), zone: await mapAsset(project, "zone"), group: input.path ? await mapAsset(project, "group") : "" };
  // New assets must be in the project snapshot before the engine validates the scene.
  project = await readProjectSnapshot();
  const ids = input.items.map(() => randomUUID());
  const group = input.path ? randomUUID() : null;
  const ground = (item: MapItemInput) => item.y ?? 0;
  const summary = input.path ? `add a path of ${input.items.length} waypoints` : input.items.length === 1 ? `add a ${input.items[0].kind}` : `add ${input.items.length} map items`;
  const result = await editScenePath(project, input.scene, input.revision, "map", summary, source => {
    let content = source;
    if (group) {
      const first = input.items[0], local = localPoint(model, parent, first.x, ground(first), first.z);
      content = appendEntity(content, group, assets.group);
      content = replaceInstanceName(content, group, input.name?.trim() || "Waypoint path");
      if (parent) content = replaceInstanceParent(content, group, parent);
      content = replaceInstanceTransform(content, group, { translation: local.map(round), rotation_degrees: [0, 0, 0], scale: [1, 1, 1] });
    }
    input.items.forEach((item, index) => {
      const id = ids[index];
      content = appendEntity(content, id, item.kind === "zone" ? assets.zone : assets.waypoint);
      content = replaceInstanceName(content, id, item.name?.trim() || (item.kind === "zone" ? "Zone" : group ? `Waypoint ${index + 1}` : "Waypoint"));
      const itemParent = group ?? parent;
      if (itemParent) content = replaceInstanceParent(content, id, itemParent);
      let local: number[];
      if (group) {
        const first = input.items[0];
        local = [item.x - first.x, ground(item) - ground(first), item.z - first.z];
        if (parent) {
          // Express the offset in the group's frame (it inherits the parent's rotation and scale).
          const parentMatrix = instanceIn(model, parent).matrix, inverse = invertAffine(parentMatrix);
          const origin = transformPoint(inverse, [first.x, ground(first), first.z]), here = transformPoint(inverse, [item.x, ground(item), item.z]);
          local = [here[0] - origin[0], here[1] - origin[1], here[2] - origin[2]];
        }
      } else local = localPoint(model, parent, item.x, ground(item), item.z);
      // Sizes and yaw are drawn in world units; under a scaled or turned parent they are stored local.
      const zone = localZone(parent ? instanceIn(model, parent).matrix : null, { width: item.width ?? 10, depth: item.depth ?? 10, radius: item.radius ?? 5, yaw: item.yaw });
      content = replaceInstanceTransform(content, id, { translation: local.map(round), rotation_degrees: [0, round(zone.yaw), 0], scale: [1, 1, 1] });
      if (item.kind === "zone") {
        const fields: Record<string, number | string> = item.shape === "rect" ? { shape: "rect", width: round(Math.max(0.01, zone.width!)), depth: round(Math.max(0.01, zone.depth!)) } : { radius: round(Math.max(0.01, zone.radius!)) };
        content = replaceInstanceComponent(content, id, "emergent.zone", fields, true);
      } else {
        const next: Record<string, string[]> = group && index + 1 < ids.length ? { links: [ids[index + 1]] } : {};
        content = replaceInstanceComponent(content, id, "emergent.waypoint", next, true);
      }
      if (item.metadata && Object.keys(item.metadata).length) content = replaceInstanceComponent(content, id, "emergent.metadata", { values: item.metadata }, true);
    });
    return content;
  });
  return { ...result, ids, group };
}

/** Moves a map item to a world XZ position, keeping its height, rotation and scale. */
export async function moveMapItem(project: ProjectSnapshot, input: { scene: string; revision: string; id: string; x: number; z: number }) {
  const model = await sceneMap(project, input.scene);
  if (model.revision !== input.revision) throw new Error("The scene changed. Reload the map and try again.");
  const item = instanceIn(model, input.id), worldY = item.matrix[13];
  const local = localPoint(model, item.parent, input.x, worldY, input.z);
  return editScenePath(project, input.scene, input.revision, "map", `move ${item.name}`, source => replaceInstanceTransform(source, input.id, { ...item.transform, translation: [round(local[0]), item.transform.translation[1], round(local[2])] }));
}

/** Adds or removes a link from waypoint `from` to waypoint `to`. */
export async function toggleWaypointLink(project: ProjectSnapshot, input: { scene: string; revision: string; from: string; to: string }) {
  if (input.from === input.to) throw new Error("Link two different waypoints.");
  const model = await sceneMap(project, input.scene);
  const [from, to] = [instanceIn(model, input.from), instanceIn(model, input.to)];
  if (![from, to].every(item => item.components.some(component => component.type === "emergent.waypoint"))) throw new Error("Links join two waypoints.");
  return editScenePath(project, input.scene, input.revision, "map", `link ${from.name} and ${to.name}`, source => {
    const current = components(source, input.from).find(component => component.type === "emergent.waypoint");
    const links = Array.isArray(current?.fields.links) ? current.fields.links.filter((link): link is string => typeof link === "string") : [];
    // The engine treats two-way links in either direction as the same link.
    const reverse = components(source, input.to).find(component => component.type === "emergent.waypoint");
    const backLinks = Array.isArray(reverse?.fields.links) ? reverse.fields.links.filter((link): link is string => typeof link === "string") : [];
    if (backLinks.includes(input.from)) return replaceInstanceComponent(source, input.to, "emergent.waypoint", { links: backLinks.length > 1 ? backLinks.filter(link => link !== input.from) : null }, true);
    const next = links.includes(input.to) ? links.filter(link => link !== input.to) : [...links, input.to];
    return replaceInstanceComponent(source, input.from, "emergent.waypoint", { links: next.length ? next : null }, true);
  });
}

/** Deletes a map item (and anything under it); links pointing at deleted waypoints are removed too. */
export async function deleteMapItem(project: ProjectSnapshot, input: { scene: string; revision: string; id: string }) {
  const model = await sceneMap(project, input.scene), item = instanceIn(model, input.id);
  if (input.id === model.root) throw new Error("The prefab root cannot be deleted.");
  const doomed = new Set([input.id]);
  for (let grew = true; grew;) { grew = false; for (const instance of model.instances) if (instance.parent && doomed.has(instance.parent) && !doomed.has(instance.id)) { doomed.add(instance.id); grew = true; } }
  return editScenePath(project, input.scene, input.revision, "map", `delete ${item.name}`, source => {
    let content = replaceSceneSubtree(source, input.id);
    for (const instance of model.instances) {
      if (doomed.has(instance.id)) continue;
      const waypoint = instance.components.find((component: AuthoredComponent) => component.type === "emergent.waypoint");
      const links = Array.isArray(waypoint?.fields.links) ? (waypoint.fields.links as string[]) : [];
      if (links.some(link => doomed.has(link))) {
        const kept = links.filter(link => !doomed.has(link));
        content = replaceInstanceComponent(content, instance.id, "emergent.waypoint", { links: kept.length ? kept : null }, true);
      }
    }
    return content;
  });
}

/** Sets one component's fields on a map item (null resets a field; `fields: null` removes the component). */
export async function setMapComponent(project: ProjectSnapshot, input: { scene: string; revision: string; id: string; type: string; fields: Record<string, unknown> | null }) {
  if (!/^emergent\.(waypoint|zone|metadata)$/.test(input.type) && !input.type.startsWith("project.")) throw new Error("Choose a map component.");
  const model = await sceneMap(project, input.scene), item = instanceIn(model, input.id);
  return editScenePath(project, input.scene, input.revision, "map", `edit ${input.type.replace(/^emergent\./, "")} on ${item.name}`, source => replaceInstanceComponent(source, input.id, input.type, input.fields as Parameters<typeof replaceInstanceComponent>[3], true));
}

export async function renameMapItem(project: ProjectSnapshot, input: { scene: string; revision: string; id: string; name: string }) {
  const model = await sceneMap(project, input.scene), item = instanceIn(model, input.id);
  return editScenePath(project, input.scene, input.revision, "map", `rename ${item.name} to ${input.name}`, source => replaceInstanceName(source, input.id, input.name));
}

/** Sets the scene's wind from the map editor (null removes it); validated by the engine like any scene edit. */
export async function setMapWind(project: ProjectSnapshot, input: { scene: string; revision: string; wind: unknown }) {
  const scene = (await projectIndex(project)).scenes.find(entry => entry.path === input.scene);
  if (!scene) throw new Error("Choose a scene from this project.");
  return setWind(project, { scene: scene.ref, revision: input.revision, wind: input.wind, source: "ide" });
}
