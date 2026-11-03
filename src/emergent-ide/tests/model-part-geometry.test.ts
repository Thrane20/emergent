import assert from "node:assert/strict";
import test from "node:test";
import * as THREE from "three";
import { center, cut, merge, normalized, partsFromScene, recentred, wouldCycle, type EditorPart } from "../src/lib/model-part-geometry";

let next = 0;
const id = () => `00000000-0000-4000-8000-${String(++next).padStart(12, "0")}`;
const box = (x: number, y: number, z: number) => normalized(new THREE.BoxGeometry(1, 1, 1).translate(x, y, z));

test("meshes become parts in model space with their own pivots", () => {
  const scene = new THREE.Group(), body = new THREE.Mesh(new THREE.BoxGeometry(2, 1, 4), new THREE.MeshStandardMaterial()), rotor = new THREE.Mesh(new THREE.BoxGeometry(6, 0.1, 0.3), body.material);
  body.name = "Body"; rotor.name = "Main rotor!"; rotor.position.set(0, 1.5, 0.5);
  scene.add(body, rotor);
  const parts = partsFromScene(scene, id);
  assert.deepEqual(parts.map(part => part.name), ["Body", "Main rotor"]);
  assert.deepEqual(parts[1].pivot.map(n => Math.round(n * 100) / 100), [0, 1.5, 0.5], "pivot at the rotor's centre, in model space");
  for (const part of parts) assert.deepEqual(Object.keys(part.geometry.attributes).sort(), ["normal", "position", "uv"]);
});

test("a plane cut splits triangles by side and keeps texture coordinates", () => {
  const shared = new THREE.MeshStandardMaterial();
  const two = [box(-2, 0, 0), box(2, 0, 0)].map((geometry): EditorPart => ({ id: id(), name: "x", parent: null, pivot: center(geometry), geometry, material: shared }));
  const geometry = merge(two);
  const halves = cut(geometry, 0, 0)!;
  assert.ok(halves, "both sides have triangles");
  const [left, right] = halves;
  assert.equal(left.index!.count + right.index!.count, geometry.index!.count, "no triangle lost or duplicated");
  assert.ok(center(left)[0] < -1.9 && center(right)[0] > 1.9, `${center(left)} | ${center(right)}`);
  assert.equal(left.getAttribute("uv").count, left.getAttribute("position").count);
  assert.equal(cut(geometry, 0, 10), null, "a plane missing the part does nothing");
});

test("merging requires a shared texture; recentring moves the pivot to the origin", () => {
  const texture = new THREE.Texture(), shared = new THREE.MeshStandardMaterial({ map: texture });
  const part = (geometry: THREE.BufferGeometry, material: THREE.Material): EditorPart => ({ id: id(), name: "p", parent: null, pivot: center(geometry), geometry, material });
  const merged = merge([part(box(0, 0, 0), shared), part(box(0, 3, 0), new THREE.MeshStandardMaterial({ map: texture }))]);
  assert.equal(merged.index!.count, box(0, 0, 0).index!.count * 2);
  assert.throws(() => merge([part(box(0, 0, 0), shared), part(box(0, 3, 0), new THREE.MeshStandardMaterial({ map: new THREE.Texture() }))]), /different textures/);
  const rotor = part(box(0, 3, 0), shared);
  assert.deepEqual(center(recentred(rotor)).map(n => Math.round(n * 1e6) / 1e6 + 0), [0, 0, 0]);
});

test("reparenting refuses cycles", () => {
  const make = (parent: string | null): EditorPart => ({ id: id(), name: "p", parent, pivot: [0, 0, 0], geometry: box(0, 0, 0), material: new THREE.MeshStandardMaterial() });
  const body = make(null), rotor = make(body.id), blade = make(rotor.id);
  const parts = [body, rotor, blade];
  assert.equal(wouldCycle(parts, body.id, blade.id), true, "a body cannot hang under its own blade");
  assert.equal(wouldCycle(parts, blade.id, body.id), false);
  assert.equal(wouldCycle(parts, rotor.id, null), false);
});
