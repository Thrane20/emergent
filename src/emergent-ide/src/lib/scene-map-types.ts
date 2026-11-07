// Scene map editor: shared types and the top-down maths (no server code).
import type { AuthoredComponent } from "./components";
import type { TransformValues } from "./scene-placement";

export type MapLayoutInstance = { id: string; matrix: number[]; outline: [number, number][]; top?: number };
export type MapInstance = { id: string; name: string; parent: string | null; asset: string; objectKind?: string; components: AuthoredComponent[]; transform: TransformValues; matrix: number[]; outline: [number, number][]; top?: number };
/** Airport overlay from the engine (`--map-layout`): world [x, z] points, compass degrees. */
export type AirportRunwayView = { id: string; heading: number; active: boolean; rightCircuit: boolean; threshold: [number, number]; farEnd: [number, number]; circuit: [number, number][]; patternAltitude?: number };
export type AirportView = { aerodrome?: string; towered: boolean; holdingFix?: [number, number]; runways: AirportRunwayView[] };
/** Wind as authored: metres per second, blowing from a compass bearing. */
export type MapWind = { speed: number; from: number };
/** Engine-rendered top-down image of every mesh, covering world XZ `min`..`max` (north edge first). */
export type MapTopDown = { min: [number, number]; max: [number, number]; image: string };
export type SceneMapModel = { scene: string; revision: string; root: string | null; instances: MapInstance[]; airports: AirportView[]; wind: MapWind | null; topDown?: MapTopDown };
export type MapItemInput = { kind: "waypoint" | "zone"; x: number; z: number; y?: number; shape?: "circle" | "rect"; radius?: number; width?: number; depth?: number; yaw?: number; name?: string; metadata?: Record<string, string | number | boolean> };

/** Applies a column-major affine 4×4 to a point. */
export function transformPoint(m: number[], [x, y, z]: number[]) {
  return [m[0] * x + m[4] * y + m[8] * z + m[12], m[1] * x + m[5] * y + m[9] * z + m[13], m[2] * x + m[6] * y + m[10] * z + m[14]];
}

/** Inverse of a column-major affine 4×4 (rotation, scale and translation; no projection). */
export function invertAffine(m: number[]) {
  const [a, b, c, d, e, f, g, h, i] = [m[0], m[4], m[8], m[1], m[5], m[9], m[2], m[6], m[10]];
  const det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
  if (!Number.isFinite(det) || Math.abs(det) < 1e-12) throw new Error("The parent's transform can't be inverted (zero scale).");
  const r = [(e * i - f * h) / det, (c * h - b * i) / det, (b * f - c * e) / det, (f * g - d * i) / det, (a * i - c * g) / det, (c * d - a * f) / det, (d * h - e * g) / det, (b * g - a * h) / det, (a * e - b * d) / det];
  const t = [m[12], m[13], m[14]];
  const tx = -(r[0] * t[0] + r[1] * t[1] + r[2] * t[2]), ty = -(r[3] * t[0] + r[4] * t[1] + r[5] * t[2]), tz = -(r[6] * t[0] + r[7] * t[1] + r[8] * t[2]);
  // Back to column-major: rows of r become columns.
  return [r[0], r[3], r[6], 0, r[1], r[4], r[7], 0, r[2], r[5], r[8], 0, tx, ty, tz, 1];
}

/** A top-down view: `center` is the world XZ point in the middle, `height` the camera's height above it. */
export type MapView = { x: number; z: number; height: number };
/** The view behaves like a 60° camera looking straight down: visible height = 2·h·tan 30°. */
export const VIEW_SPAN = 2 * Math.tan(Math.PI / 6);
export const pixelsPerMetre = (view: MapView, viewportHeight: number) => viewportHeight / (view.height * VIEW_SPAN);
/** World XZ to screen; north (−Z) is up. */
export function toScreen(view: MapView, size: { width: number; height: number }, x: number, z: number) {
  const scale = pixelsPerMetre(view, size.height);
  return [size.width / 2 + (x - view.x) * scale, size.height / 2 + (z - view.z) * scale];
}
export function toWorld(view: MapView, size: { width: number; height: number }, sx: number, sy: number) {
  const scale = pixelsPerMetre(view, size.height);
  return [view.x + (sx - size.width / 2) / scale, view.z + (sy - size.height / 2) / scale];
}
/** Zooms by `factor` (above 1 goes up, out) keeping the world point under the cursor fixed. */
export function zoomAt(view: MapView, size: { width: number; height: number }, sx: number, sy: number, factor: number): MapView {
  const height = Math.min(Math.max(view.height * factor, 2), 20000);
  const [wx, wz] = toWorld(view, size, sx, sy), next = { ...view, height };
  const [ax, az] = toWorld(next, size, sx, sy);
  return { x: next.x + wx - ax, z: next.z + wz - az, height };
}
/** A view that fits world XZ bounds with a margin. */
export function fitView(points: [number, number][], size: { width: number; height: number }): MapView {
  if (!points.length) return { x: 0, z: 0, height: 60 };
  const xs = points.map(p => p[0]), zs = points.map(p => p[1]);
  const [minX, maxX, minZ, maxZ] = [Math.min(...xs), Math.max(...xs), Math.min(...zs), Math.max(...zs)];
  const spanZ = Math.max(maxZ - minZ, (maxX - minX) * size.height / Math.max(size.width, 1), 10) * 1.2;
  return { x: (minX + maxX) / 2, z: (minZ + maxZ) / 2, height: spanZ / VIEW_SPAN };
}

/** The world-space zone a map item draws, from its component fields and world matrix. */
export function zoneShape(instance: MapInstance) {
  const zone = instance.components.find(component => component.type === "emergent.zone" && component.enabled);
  if (!zone) return null;
  const m = instance.matrix, f = zone.fields;
  const sx = Math.hypot(m[0], m[2]) || 1, sz = Math.hypot(m[8], m[10]) || 1;
  const yaw = Math.atan2(-m[2], m[0]);
  const number = (name: string, fallback: number) => typeof f[name] === "number" ? f[name] as number : fallback;
  const blocked = f.blocked === true, cost = number("cost", 1);
  const layers = typeof f.layers === "string" ? f.layers : "";
  return f.shape === "rect"
    ? { kind: "rect" as const, x: m[12], z: m[14], yaw, width: number("width", 10) * sx, depth: number("depth", 10) * sz, cost, blocked, layers }
    : { kind: "circle" as const, x: m[12], z: m[14], yaw, radius: number("radius", 5) * Math.max(sx, sz), cost, blocked, layers };
}

/** A zone drawn at a world size and yaw, as the local fields and yaw under a parent whose world
 * matrix is `parent` (the inverse of `zoneShape`): drawn sizes stay true under a scaled parent. */
export function localZone(parent: number[] | null, item: { width?: number; depth?: number; radius?: number; yaw?: number }) {
  if (!parent) return { width: item.width, depth: item.depth, radius: item.radius, yaw: item.yaw ?? 0 };
  const sx = Math.hypot(parent[0], parent[2]) || 1, sz = Math.hypot(parent[8], parent[10]) || 1;
  const yaw = Math.atan2(-parent[2], parent[0]) * 180 / Math.PI;
  return {
    width: item.width === undefined ? undefined : item.width / sx,
    depth: item.depth === undefined ? undefined : item.depth / sz,
    radius: item.radius === undefined ? undefined : item.radius / Math.max(sx, sz),
    yaw: (item.yaw ?? 0) - yaw,
  };
}

export function waypointLinks(instance: MapInstance) {
  const waypoint = instance.components.find(component => component.type === "emergent.waypoint" && component.enabled);
  if (!waypoint) return null;
  const links = Array.isArray(waypoint.fields.links) ? (waypoint.fields.links as string[]) : [];
  return { links, twoWay: waypoint.fields.two_way !== false };
}

export function metadataOf(instance: MapInstance): Record<string, string | number | boolean> {
  const metadata = instance.components.find(component => component.type === "emergent.metadata" && component.enabled);
  const values = metadata?.fields.values;
  return values && typeof values === "object" && !Array.isArray(values) ? values as Record<string, string | number | boolean> : {};
}
