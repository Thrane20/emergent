import * as THREE from "three";
import type { EditorPart } from "./model-part-geometry";

/** Draft cutter in model space; its axes stay fixed when the camera orbits. */
export type CutBox = {
  origin: THREE.Vector3Tuple;
  rotation: THREE.QuaternionTuple;
  min: THREE.Vector3Tuple;
  max: THREE.Vector3Tuple;
};
const MAX_TRIANGLES = 500_000;

export function cutBoxFrame(box: CutBox) {
  if (![...box.origin, ...box.rotation, ...box.min, ...box.max].every(Number.isFinite)
    || box.min.some((value, axis) => value >= box.max[axis])
    || Math.abs(new THREE.Quaternion(...box.rotation).lengthSq() - 1) > 1e-5) {
    throw new Error("The cut box needs finite coordinates, a valid rotation and positive dimensions.");
  }
  return new THREE.Matrix4().compose(new THREE.Vector3(...box.origin), new THREE.Quaternion(...box.rotation), new THREE.Vector3(1, 1, 1));
}

export function cutBoxCorners(box: CutBox) {
  const frame = cutBoxFrame(box);
  return Array.from({ length: 8 }, (_, corner) => new THREE.Vector3(...box.min.map((value, axis) => corner & (1 << axis) ? box.max[axis] : value) as THREE.Vector3Tuple).applyMatrix4(frame));
}

/** Keep the opposite corner fixed, and prevent a handle crossing it or collapsing the box. */
export function resizeBoxCorner(box: CutBox, corner: number, world: THREE.Vector3): CutBox {
  if (!Number.isInteger(corner) || corner < 0 || corner > 7 || !world.toArray().every(Number.isFinite)) throw new Error("Choose a valid box corner and finite position.");
  const local = world.clone().applyMatrix4(cutBoxFrame(box).invert());
  const min = [...box.min] as THREE.Vector3Tuple, max = [...box.max] as THREE.Vector3Tuple;
  const minimum = Math.max(...max.map((value, axis) => value - min[axis])) * 1e-4;
  for (let axis = 0; axis < 3; axis++) {
    if (corner & (1 << axis)) max[axis] = Math.max(min[axis] + minimum, local.getComponent(axis));
    else min[axis] = Math.min(max[axis] - minimum, local.getComponent(axis));
  }
  return { ...box, min, max };
}

/** Rectangle endpoints lie in the view plane through origin; initial depth equals width. */
export function drawnCutBox(origin: THREE.Vector3, rotation: THREE.Quaternion, start: THREE.Vector3, end: THREE.Vector3): CutBox | null {
  const inverse = new THREE.Matrix4().compose(origin, rotation, new THREE.Vector3(1, 1, 1)).invert();
  const a = start.clone().applyMatrix4(inverse), b = end.clone().applyMatrix4(inverse);
  const width = Math.abs(a.x - b.x), height = Math.abs(a.y - b.y);
  if (width < 1e-8 || height < 1e-8) return null;
  return { origin: origin.toArray(), rotation: rotation.toArray(), min: [Math.min(a.x, b.x), Math.min(a.y, b.y), -width / 2], max: [Math.max(a.x, b.x), Math.max(a.y, b.y), width / 2] };
}

type Vertex = { position: THREE.Vector3; local: THREE.Vector3; normal: THREE.Vector3; uv: THREE.Vector2 };

function split(polygon: Vertex[], axis: number, bound: number, sign: number): [Vertex[], Vertex[]] {
  const inside: Vertex[] = [], outside: Vertex[] = [];
  for (let i = 0; i < polygon.length; i++) {
    const a = polygon[i], b = polygon[(i + 1) % polygon.length];
    const da = sign * (a.local.getComponent(axis) - bound), db = sign * (b.local.getComponent(axis) - bound);
    (da >= 0 ? inside : outside).push(a);
    if ((da >= 0) !== (db >= 0)) {
      const t = da / (da - db), edge: Vertex = {
        position: a.position.clone().lerp(b.position, t), local: a.local.clone().lerp(b.local, t),
        normal: a.normal.clone().lerp(b.normal, t), uv: a.uv.clone().lerp(b.uv, t),
      };
      inside.push(edge); outside.push(edge);
    }
  }
  return [inside, outside];
}

function hasArea(a: Vertex, b: Vertex, c: Vertex) {
  const ab = b.position.clone().sub(a.position), ac = c.position.clone().sub(a.position);
  const scale = Math.max(ab.lengthSq(), ac.lengthSq());
  return ab.cross(ac).lengthSq() > scale * scale * 1e-20;
}

/** Surface subtraction only: preserve winding/UVs/normals and leave the opening uncapped. */
export function cutBoxGeometry(geometry: THREE.BufferGeometry, box: CutBox): THREE.BufferGeometry | null {
  const inverse = cutBoxFrame(box).invert(), index = geometry.index;
  const position = geometry.getAttribute("position"), normal = geometry.getAttribute("normal"), uv = geometry.getAttribute("uv");
  if (!index || index.count % 3 || index.count / 3 > MAX_TRIANGLES || !position || !normal || !uv
    || position.itemSize !== 3 || normal.itemSize !== 3 || uv.itemSize !== 2
    || position.count > 1_000_000 || normal.count !== position.count || uv.count !== position.count) throw new Error("Box cutting needs indexed positions, normals and UVs, with at most 500,000 triangles per part.");
  for (const attribute of [position, normal, uv]) for (let i = 0; i < attribute.count; i++) for (let axis = 0; axis < attribute.itemSize; axis++) {
    if (!Number.isFinite(attribute.getComponent(i, axis))) throw new Error("This part contains non-finite vertex data.");
  }
  for (let i = 0; i < index.count; i++) if (!Number.isInteger(index.getX(i)) || index.getX(i) < 0 || index.getX(i) >= position.count) throw new Error("This part contains an invalid vertex index.");
  geometry.computeBoundingBox();
  if (!geometry.boundingBox!.clone().applyMatrix4(inverse).intersectsBox(new THREE.Box3(new THREE.Vector3(...box.min), new THREE.Vector3(...box.max)))) return null;
  const positions: number[] = [], normals: number[] = [], uvs: number[] = [], indices: number[] = [];
  const originals = new Map<Vertex, number>();
  let changed = false;
  function emit(polygon: Vertex[]) {
    for (let i = 1; i + 1 < polygon.length; i++) {
      if (!hasArea(polygon[0], polygon[i], polygon[i + 1])) continue;
      if (indices.length / 3 >= MAX_TRIANGLES) throw new Error("This cut exceeds 500,000 triangles in a part. Use a smaller cut or split the model first.");
      for (const vertex of [polygon[0], polygon[i], polygon[i + 1]]) {
        let mapped = originals.get(vertex);
        if (mapped === undefined) {
          if (positions.length / 3 >= 1_000_000) throw new Error("This cut exceeds one million vertices in a part. Split the model first.");
          mapped = positions.length / 3; originals.set(vertex, mapped);
          positions.push(...vertex.position.toArray()); normals.push(...vertex.normal.clone().normalize().toArray()); uvs.push(...vertex.uv.toArray());
        }
        indices.push(mapped);
      }
    }
  }
  const vertices = new Map<number, Vertex>();
  function vertex(i: number) {
    if (!Number.isInteger(i) || i < 0 || i >= position.count) throw new Error("This part contains an invalid vertex index.");
    let value = vertices.get(i);
    if (!value) {
      const point = new THREE.Vector3().fromBufferAttribute(position, i);
      value = { position: point, local: point.clone().applyMatrix4(inverse), normal: new THREE.Vector3().fromBufferAttribute(normal, i), uv: new THREE.Vector2(uv.getX(i), uv.getY(i)) };
      vertices.set(i, value);
    }
    return value;
  }
  for (let triangle = 0; triangle < index.count; triangle += 3) {
    const original = [vertex(index.getX(triangle)), vertex(index.getX(triangle + 1)), vertex(index.getX(triangle + 2))];
    let inside = original;
    const outside: Vertex[][] = [];
    for (let axis = 0; axis < 3 && inside.length; axis++) for (const sign of [1, -1]) {
      const [next, discarded] = split(inside, axis, sign === 1 ? box.min[axis] : box.max[axis], sign);
      outside.push(discarded); inside = next;
    }
    if (inside.length >= 3 && inside.slice(1, -1).some((v, i) => hasArea(inside[0], v, inside[i + 2]))) {
      changed = true; outside.forEach(emit);
    } else emit(original);
  }
  if (!changed) return null;
  const result = new THREE.BufferGeometry();
  result.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  result.setAttribute("normal", new THREE.Float32BufferAttribute(normals, 3));
  result.setAttribute("uv", new THREE.Float32BufferAttribute(uvs, 2));
  result.setIndex(indices);
  return result;
}

/** Prepare the whole edit before publishing; surviving part identities/pivots stay unchanged. */
export function cutPartsWithBox(parts: EditorPart[], box: CutBox): EditorPart[] | null {
  const created: THREE.BufferGeometry[] = [];
  try {
    let changed = false;
    const next = parts.flatMap(part => {
      const geometry = cutBoxGeometry(part.geometry, box);
      if (!geometry) return [part];
      created.push(geometry); changed = true;
      return geometry.index!.count ? [{ ...part, geometry }] : [];
    });
    if (!changed) return null;
    if (!next.length) throw new Error("This box removes the entire model. Reduce the box to keep at least one part.");
    created.filter(geometry => !geometry.index!.count).forEach(geometry => geometry.dispose());
    const kept = new Set(next.map(part => part.id)), old = new Map(parts.map(part => [part.id, part]));
    return next.map(part => {
      let parent = part.parent;
      while (parent && !kept.has(parent)) parent = old.get(parent)?.parent ?? null;
      return parent === part.parent ? part : { ...part, parent };
    });
  } catch (error) {
    created.forEach(geometry => geometry.dispose());
    throw error;
  }
}
