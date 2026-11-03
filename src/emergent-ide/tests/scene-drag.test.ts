import test from "node:test";
import assert from "node:assert/strict";
import { prefabDrop, sceneInstanceDrop } from "../src/lib/scene-drag";
import type { ScenePlacement } from "../src/lib/scene-placement";

const placement: ScenePlacement = { path: "assets/main.scene.toml", revision: "current", space: "local", instances: [{ id: "body", asset: "assets/body.entity.toml", parent: null, defaults: { translation: [0,0,0], rotation_degrees: [0,0,0], scale: [1,1,1] }, transform: { translation: [0,0,0], rotation_degrees: [0,0,0], scale: [1,1,1] }, overridden: false }] };
const data = { projectId: "game", scene: placement.path, revision: placement.revision, id: "body", asset: "assets/drone.prefab.toml" };

test("scene and prefab drag payloads carry project, scene and revision boundaries", () => {
  assert.equal(sceneInstanceDrop(JSON.stringify(data), "game", placement), "body");
  assert.equal(prefabDrop(JSON.stringify(data), "game", placement), data.asset);
  for (const patch of [{ projectId: "other" }, { scene: "other.scene.toml" }, { revision: "stale" }]) {
    assert.throws(() => sceneInstanceDrop(JSON.stringify({ ...data, ...patch }), "game", placement), /changed/);
    assert.throws(() => prefabDrop(JSON.stringify({ ...data, ...patch }), "game", placement), /changed/);
  }
  assert.throws(() => sceneInstanceDrop(JSON.stringify({ ...data, id: "deleted" }), "game", placement), /no longer exists/);
  assert.throws(() => prefabDrop(JSON.stringify({ ...data, asset: "assets/body.entity.toml" }), "game", placement), /Drag a prefab/);
  assert.throws(() => sceneInstanceDrop("{}", "game", placement), /changed/);
  assert.throws(() => prefabDrop("null", "game", placement), /changed/);
});
