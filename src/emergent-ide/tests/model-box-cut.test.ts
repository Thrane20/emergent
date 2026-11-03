import assert from "node:assert/strict";
import test from "node:test";
import * as THREE from "three";
import { normalized, type EditorPart } from "../src/lib/model-part-geometry";
import { cutBoxCorners, cutBoxFrame, cutBoxGeometry, cutPartsWithBox, drawnCutBox, resizeBoxCorner, type CutBox } from "../src/lib/model-box-cut";

const box = (): CutBox => ({ origin: [0, 0, 0], rotation: [0, 0, 0, 1], min: [-0.5, -0.5, -1], max: [0.5, 0.5, 1] });
const close = (value: number, expected: number) => assert.ok(Math.abs(value - expected) < 1e-5, `${value} != ${expected}`);
function area(geometry: THREE.BufferGeometry) {
  const position = geometry.getAttribute("position"), index = geometry.index!;
  let result = 0;
  for (let i = 0; i < index.count; i += 3) {
    const a = new THREE.Vector3().fromBufferAttribute(position, index.getX(i)), b = new THREE.Vector3().fromBufferAttribute(position, index.getX(i + 1)), c = new THREE.Vector3().fromBufferAttribute(position, index.getX(i + 2));
    result += b.sub(a).cross(c.sub(a)).length() / 2;
  }
  return result;
}
const surface = () => normalized(new THREE.PlaneGeometry(4, 4));

test("box clips through large triangles, removing the enclosed area while preserving UVs, normals and winding", () => {
  const source = surface(), original = source.getAttribute("position").array.slice();
  const result = cutBoxGeometry(source, box())!;
  close(area(source), 16); close(area(result), 15);
  assert.deepEqual(source.getAttribute("position").array, original);
  const position = result.getAttribute("position"), uv = result.getAttribute("uv"), normal = result.getAttribute("normal"), index = result.index!;
  assert.equal(new Set(index.array).size, position.count, "no unreferenced vertices survive");
  assert.equal(uv.count, position.count); assert.equal(normal.count, position.count);
  for (let i = 0; i < position.count; i++) {
    close(uv.getX(i), position.getX(i) / 4 + 0.5); close(uv.getY(i), position.getY(i) / 4 + 0.5); close(normal.getZ(i), 1);
    assert.ok(!(Math.abs(position.getX(i)) < 0.49999 && Math.abs(position.getY(i)) < 0.49999), "interior points are removed");
  }
  for (let i = 0; i < index.count; i += 3) {
    const a = new THREE.Vector3().fromBufferAttribute(position, index.getX(i)), b = new THREE.Vector3().fromBufferAttribute(position, index.getX(i + 1)), c = new THREE.Vector3().fromBufferAttribute(position, index.getX(i + 2));
    assert.ok(b.sub(a).cross(c.sub(a)).z > 0, "triangle winding preserved; no degenerate faces");
  }
});

test("rotated/translated cutter uses its 3D frame, and missing boxes preserve original geometry", () => {
  const cutter = box(); cutter.origin = [5, -3, 8]; cutter.rotation = new THREE.Quaternion().setFromEuler(new THREE.Euler(0.4, 0.7, -0.2)).toArray();
  const source = surface().applyMatrix4(cutBoxFrame(cutter));
  close(area(cutBoxGeometry(source, cutter)!), 15);
  const miss = box(); miss.origin = [100, 0, 0]; assert.equal(cutBoxGeometry(source, miss), null);
  const away = box(); away.min[2] = 2; away.max[2] = 3; assert.equal(cutBoxGeometry(surface(), away), null);
});

test("full containment produces empty compact geometry, but deleting the entire model is refused atomically", () => {
  const source = normalized(new THREE.BoxGeometry(0.5, 0.5, 0.5));
  const result = cutBoxGeometry(source, box())!;
  assert.equal(result.index!.count, 0); assert.equal(result.getAttribute("position").count, 0);
  const part: EditorPart = { id: "body", name: "Body", parent: null, pivot: [0, 0, 0], geometry: source, material: new THREE.MeshStandardMaterial() };
  assert.throws(() => cutPartsWithBox([part], box()), /entire model/);
  assert.equal(source.index!.count, 36); assert.equal(part.geometry, source);
});

test("cuts cross parts, remove fully covered parts and reattach children while preserving identity/pivot/material", () => {
  const make = (id: string, x: number, parent: string | null): EditorPart => ({ id, name: id, parent, pivot: [x, 0, 0], geometry: normalized(new THREE.BoxGeometry(0.5, 0.5, 0.5).translate(x, 0, 0)), material: new THREE.MeshStandardMaterial() });
  const root = make("root", 5, null), removed = make("removed", 0, root.id), child = make("child", 2, removed.id);
  const next = cutPartsWithBox([root, removed, child], box())!;
  assert.deepEqual(next.map(part => [part.id, part.parent]), [["root", null], ["child", "root"]]);
  assert.equal(next[1].geometry, child.geometry); assert.equal(next[1].material, child.material); assert.equal(next[1].pivot, child.pivot);
  assert.equal(cutPartsWithBox([root, child], box()), null);
  const a = { ...root, geometry: surface() }, b = { ...child, geometry: surface().translate(0, 0, 0.25) };
  const cut = cutPartsWithBox([a, b], box())!;
  close(area(cut[0].geometry), 15); close(area(cut[1].geometry), 15);
});

test("invalid box, vertex data and over-budget geometry are rejected before editing any part", () => {
  assert.throws(() => cutBoxGeometry(surface(), { ...box(), max: [-1, 0, 0] }), /positive dimensions/);
  assert.throws(() => cutBoxFrame({ ...box(), rotation: [0, 0, 0, 0] }), /valid rotation/);
  assert.throws(() => cutBoxFrame({ ...box(), origin: [NaN, 0, 0] }), /finite/);
  const bad = surface(); bad.getAttribute("position").setX(0, NaN);
  assert.throws(() => cutBoxGeometry(bad, box()), /non-finite/);
  const tooLarge = surface(); tooLarge.setIndex(new THREE.BufferAttribute(new Uint32Array(500_001 * 3), 1));
  assert.throws(() => cutBoxGeometry(tooLarge, box()), /500,000/);
  const badIndex = surface(); badIndex.index!.setX(0, 9999);
  assert.throws(() => cutBoxGeometry(badIndex, box()), /invalid vertex index/);
});

test("drawing in either direction starts with width-depth equality; corner dragging fixes the opposite corner", () => {
  const origin = new THREE.Vector3(3, 4, 5), rotation = new THREE.Quaternion().setFromEuler(new THREE.Euler(0.4, 0.5, 0.1));
  const transform = new THREE.Matrix4().compose(origin, rotation, new THREE.Vector3(1, 1, 1));
  const a = new THREE.Vector3(-1, -2, 0).applyMatrix4(transform), b = new THREE.Vector3(2, 2, 0).applyMatrix4(transform);
  const drawn = drawnCutBox(origin, rotation, b, a)!;
  close(drawn.max[0] - drawn.min[0], 3); close(drawn.max[1] - drawn.min[1], 4); close(drawn.max[2] - drawn.min[2], 3);
  assert.equal(drawnCutBox(origin, rotation, a, a), null);
  const opposite = cutBoxCorners(drawn)[0];
  const resized = resizeBoxCorner(drawn, 7, new THREE.Vector3(1, 1, 0.25).applyMatrix4(transform));
  close(cutBoxCorners(resized)[0].distanceTo(opposite), 0);
  close(resized.max[2], 0.25);
  const clamped = resizeBoxCorner(drawn, 7, new THREE.Vector3(-50, -50, -50).applyMatrix4(transform));
  assert.ok(clamped.max.every((value, axis) => value > clamped.min[axis]));
});
