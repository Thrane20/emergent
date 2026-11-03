import * as THREE from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

/** One editable part: geometry in model space, its material, and a pivot it rotates about. */
export type EditorPart = { id: string; name: string; parent: string | null; pivot: [number, number, number]; geometry: THREE.BufferGeometry; material: THREE.Material };
export type Axis = 0 | 1 | 2;

/** Indexed, with exactly position, normal and uv, so parts merge and export uniformly. */
export function normalized(source: THREE.BufferGeometry) {
  const geometry = new THREE.BufferGeometry();
  const position = source.getAttribute("position") as THREE.BufferAttribute;
  geometry.setAttribute("position", position.clone());
  const uv = source.getAttribute("uv") as THREE.BufferAttribute | undefined;
  geometry.setAttribute("uv", uv ? uv.clone() : new THREE.BufferAttribute(new Float32Array(position.count * 2), 2));
  geometry.setIndex(source.index ? source.index.clone() : [...Array(position.count).keys()]);
  const normal = source.getAttribute("normal") as THREE.BufferAttribute | undefined;
  if (normal) geometry.setAttribute("normal", normal.clone()); else geometry.computeVertexNormals();
  return geometry;
}

export function center(geometry: THREE.BufferGeometry): [number, number, number] {
  geometry.computeBoundingBox();
  return geometry.boundingBox!.getCenter(new THREE.Vector3()).toArray() as [number, number, number];
}

const material = (value: THREE.Material | THREE.Material[]) => Array.isArray(value) ? value[0] : value;

/** Every mesh in a loaded glTF scene becomes a part, with its world transform baked in. */
export function partsFromScene(scene: THREE.Object3D, newId: () => string): EditorPart[] {
  scene.updateMatrixWorld(true);
  const parts: EditorPart[] = [];
  scene.traverse(object => {
    const mesh = object as THREE.Mesh;
    if (!mesh.isMesh) return;
    const geometry = normalized(mesh.geometry).applyMatrix4(mesh.matrixWorld);
    // GLTFLoader turns spaces in node names into underscores; show them as spaces again.
    const name = (mesh.name || mesh.parent?.name || "").replace(/_/g, " ").replace(/[^\p{L}\p{N} .-]/gu, "").trim().slice(0, 64);
    parts.push({ id: newId(), name: name && /^[\p{L}\p{N}]/u.test(name) ? name : `Part ${parts.length + 1}`, parent: null, pivot: center(geometry), geometry, material: material(mesh.material) });
  });
  return parts;
}

/** The triangles listed in `keep`, with only the vertices they use. */
function subset(geometry: THREE.BufferGeometry, keep: number[]) {
  const index = geometry.index!, remap = new Map<number, number>(), order: number[] = [], indices: number[] = [];
  for (const triangle of keep) for (let corner = 0; corner < 3; corner++) {
    const vertex = index.getX(triangle * 3 + corner);
    let mapped = remap.get(vertex);
    if (mapped === undefined) { mapped = order.length; remap.set(vertex, mapped); order.push(vertex); }
    indices.push(mapped);
  }
  const result = new THREE.BufferGeometry();
  for (const name of ["position", "normal", "uv"]) {
    const source = geometry.getAttribute(name) as THREE.BufferAttribute, size = source.itemSize;
    const values = new Float32Array(order.length * size);
    order.forEach((vertex, i) => { for (let k = 0; k < size; k++) values[i * size + k] = source.array[vertex * size + k]; });
    result.setAttribute(name, new THREE.BufferAttribute(values, size));
  }
  result.setIndex(indices);
  return result;
}

/**
 * Splits geometry with the plane `axis = offset`, assigning each triangle by its centroid.
 * shortcut: triangles are not clipped, so the cut edge follows existing triangle edges; add exact clipping with UV interpolation if cut seams look ragged on coarse meshes.
 */
export function cut(geometry: THREE.BufferGeometry, axis: Axis, offset: number): [THREE.BufferGeometry, THREE.BufferGeometry] | null {
  const index = geometry.index!, position = geometry.getAttribute("position");
  const below: number[] = [], above: number[] = [];
  for (let triangle = 0; triangle < index.count / 3; triangle++) {
    let sum = 0;
    for (let corner = 0; corner < 3; corner++) sum += position.getComponent(index.getX(triangle * 3 + corner), axis);
    (sum / 3 < offset ? below : above).push(triangle);
  }
  return below.length && above.length ? [subset(geometry, below), subset(geometry, above)] : null;
}

/** Combines parts that share a material; different textures cannot share one engine model. */
export function merge(parts: EditorPart[]): THREE.BufferGeometry {
  const first = parts[0].material as THREE.MeshStandardMaterial;
  const same = parts.every(part => part.material === first || (part.material as THREE.MeshStandardMaterial).map?.source === first.map?.source);
  if (!same) throw new Error("These parts use different textures, so they cannot be merged.");
  const merged = mergeGeometries(parts.map(part => part.geometry));
  if (!merged) throw new Error("These parts could not be merged.");
  return merged;
}

/** Geometry moved so its pivot is the origin, ready to save as the part's own model. */
export function recentred(part: EditorPart) {
  return part.geometry.clone().translate(-part.pivot[0], -part.pivot[1], -part.pivot[2]);
}

/** Whether making `parent` the parent of `child` would create a cycle. */
export function wouldCycle(parts: EditorPart[], child: string, parent: string | null) {
  const byId = new Map(parts.map(part => [part.id, part]));
  for (let cursor = parent; cursor; cursor = byId.get(cursor)?.parent ?? null) if (cursor === child) return true;
  return false;
}
