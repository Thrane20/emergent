import assert from "node:assert/strict";
import test from "node:test";
import type { ProjectFile, ProjectSnapshot } from "../src/lib/project";
import { findEntries, projectIndex, resolveRef } from "../src/lib/project-index";

let generation = 0;
function snapshot(files: Record<string, string>): ProjectSnapshot {
  generation++; // Distinct cache keys per fixture.
  const list: ProjectFile[] = Object.entries(files).map(([path, content]) => ({ path, name: path.split("/").at(-1)!, content: /\.(toml|json)$/.test(path) ? content : null, kind: "file", bytes: content.length + generation, modifiedAt: "", authored: true, generated: false }));
  return { name: "Fixture", id: `fixture-${generation}`, entryScene: "assets/scenes/main.scene.toml", root: "/nonexistent", assetRoots: ["assets"], files: list, connections: [], entities: [], outputs: [] };
}

const cube = 'schema_version = 1\nid = "cube"\nname = "Cube"\nkind = "primitive"\n[transform]\ntranslation = [0, 1, 0]\n';
const lamp = 'schema_version = 1\nid = "lamp"\nname = "Lamp"\nkind = "light"\n';

test("literal-quoted template IDs are indexed without missing-id repairs", async () => {
  const index = await projectIndex(snapshot({
    "assets/entities/cube.entity.toml": "schema_version=1\nid='cube'\nname='Cube'\nkind='primitive'\n",
    "assets/scenes/main.scene.toml": "schema_version=1\nid='main'\n[[entities]]\nid='one'\nasset='assets/entities/cube.entity.toml'\n",
  }));
  assert.equal(index.assets[0].ref, "asset:cube");
  assert.equal(index.scenes[0].ref, "scene:main");
  assert.deepEqual(index.diagnostics, []);
});
const scene = `schema_version = 1
id = "main"
name = "Main"
[[entities]]
id = "table"
asset = "assets/entities/cube.entity.toml"
name = "Table"
[entities.transform]
translation = [2, 0, 3]
[[entities]]
id = "cube-a"
asset = "assets/entities/cube.entity.toml"
parent = "table"
[[entities]]
id = "cube-b"
asset = "assets/entities/cube.entity.toml"
[[entities]]
id = "lamp"
asset = "assets/entities/lamp.entity.toml"
parent = "cube-a"
`;

test("index assigns durable refs, resolves names like the IDE and builds hierarchy paths", async () => {
  const index = await projectIndex(snapshot({ "assets/entities/cube.entity.toml": cube, "assets/entities/lamp.entity.toml": lamp, "assets/scenes/main.scene.toml": scene }));
  assert.deepEqual(index.assets.map(asset => asset.ref).sort(), ["asset:cube", "asset:lamp"]);
  assert.equal(index.entryScene, "scene:main");
  const lampInstance = resolveRef(index, "instance:main/lamp");
  assert.equal(lampInstance.type, "instance");
  if (lampInstance.type !== "instance") return;
  assert.equal(lampInstance.hierarchy, "Table/Cube/Lamp");
  assert.equal(lampInstance.parent, "instance:main/cube-a");
  assert.equal(lampInstance.asset, "asset:lamp");
  const table = resolveRef(index, "instance:main/table");
  assert.deepEqual(table.type === "instance" && table.translation, [2, 0, 3]);
  const inherited = resolveRef(index, "instance:main/cube-b");
  assert.deepEqual(inherited.type === "instance" && inherited.translation, [0, 1, 0]);
  assert.deepEqual(index.diagnostics, []);
});

test("duplicate names stay distinct candidates; exact names outrank substrings", async () => {
  const index = await projectIndex(snapshot({ "assets/entities/cube.entity.toml": cube, "assets/entities/lamp.entity.toml": lamp, "assets/scenes/main.scene.toml": scene }));
  const cubes = findEntries(index, "cube", { type: "instance" });
  assert.deepEqual(cubes.slice(0, 2).map(result => result.entry.ref).sort(), ["instance:main/cube-a", "instance:main/cube-b"]);
  assert.equal(cubes[0].score, cubes[1].score);
  assert.equal(findEntries(index, "lam")[0].entry.name, "Lamp");
  assert.equal(findEntries(index, "light", { type: "asset" })[0].entry.ref, "asset:lamp");
  assert.deepEqual(findEntries(index, "  "), []);
  assert.ok(findEntries(index, "cube", { limit: 100 }).length <= 20);
});

test("UI assets are discoverable by durable ID and diagnose missing/duplicate identity", async () => {
  const ui = 'schema_version=1\nid="options"\nname="Options"\nkind="ui"\ncanvas=[640,480]\n';
  const index = await projectIndex(snapshot({ "assets/ui/options.ui.toml": ui }));
  assert.equal(findEntries(index, "Options", { type: "asset" })[0].entry.ref, "asset:options");
  assert.equal(resolveRef(index, "asset:options").type, "asset");
  assert.equal(index.assets[0].kind, "ui");
  const broken = await projectIndex(snapshot({ "assets/ui/options.ui.toml": ui, "assets/ui/copy.ui.toml": ui, "assets/ui/missing.ui.toml": 'schema_version=1\nname="Missing"\n' }));
  assert.throws(() => resolveRef(broken, "asset:options"), /ambiguous/);
  assert.ok(broken.diagnostics.some(item => item.code === "missing-id"));
});

test("stale, malformed and duplicated refs fail instead of resolving to another object", async () => {
  const index = await projectIndex(snapshot({
    "assets/entities/cube.entity.toml": cube,
    "assets/entities/copy.entity.toml": cube.replace('name = "Cube"', 'name = "Copy"'),
    "assets/entities/lamp.entity.toml": lamp,
    "assets/scenes/main.scene.toml": scene,
  }));
  assert.throws(() => resolveRef(index, "asset:cube"), /ambiguous/);
  assert.ok(index.diagnostics.some(item => item.code === "duplicate-id" && item.paths.length === 2));
  assert.ok(!findEntries(index, "copy").some(result => result.entry.ref === "asset:cube"));
  assert.throws(() => resolveRef(index, "instance:main/deleted"), /does not exist/);
  assert.throws(() => resolveRef(index, "assets/entities/lamp.entity.toml"), /not a ref/);
  assert.throws(() => resolveRef(index, "instance:main/../lamp"), /not a ref/);
});

test("descriptor bundles own their media; unowned media and id-less definitions are reported", async () => {
  const index = await projectIndex(snapshot({
    "assets/models/arm/object.asset.json": JSON.stringify({ schema_version: 1, kind: "generated-model", id: "8c37", name: "robot arm" }),
    "assets/models/arm/model.glb": "glb",
    "assets/textures/loose/atlas.png": "png",
    "assets/entities/nameless.entity.toml": 'schema_version = 1\nname = "No id"\n',
    "assets/scenes/broken.scene.toml": 'schema_version = 1\nid = "broken"\n[[entities]]\nasset = "x"\n',
  }));
  const arm = resolveRef(index, "asset:8c37");
  assert.deepEqual(arm.type === "asset" && arm.files, ["assets/models/arm/object.asset.json", "assets/models/arm/model.glb"]);
  assert.equal(findEntries(index, "robot arm")[0].entry.ref, "asset:8c37");
  const codes = index.diagnostics.map(item => `${item.code} ${item.paths[0]}`).sort();
  assert.deepEqual(codes, ["invalid-scene assets/scenes/broken.scene.toml", "missing-id assets/entities/nameless.entity.toml", "missing-id assets/textures/loose/atlas.png"]);
});
