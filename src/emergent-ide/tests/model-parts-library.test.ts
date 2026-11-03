import assert from "node:assert/strict";
import test from "node:test";
import { stringify } from "smol-toml";
import { modelPartsLibrary } from "../src/lib/model-parts";
import type { ProjectSnapshot } from "../src/lib/project";

const folder = "assets/models/edited";
const id = "11111111-1111-4111-8111-111111111111";
const source = (path: string, content: string) => ({ path, content });
const base = [
  source(`${folder}/object.asset.json`, JSON.stringify({ schema_version: 1, kind: "generated-model", id, name: "Edited model", model: `${folder}/model.glb` })),
  source(`${folder}/parts/parts.toml`, stringify({ schema_version: 1, model: `${folder}/object.asset.json`, root: `${folder}/parts/root.entity.toml`, parts: [{ id, name: "Body", pivot: [0, 0, 0], asset: `${folder}/parts/${id}.entity.toml` }] })),
];
const prefab = stringify({ schema_version: 1, id: "22222222-2222-4222-8222-222222222222", name: "Edited model", root: "model", entities: [{ id: "model", asset: `${folder}/parts/root.entity.toml` }, { id: "body", asset: `${folder}/parts/${id}.entity.toml`, parent: "model" }] });

test("saved multipart prefab is a single library item that reopens its Model Parts descriptor", () => {
  const project = { files: [...base, source(`${folder}/parts/model.prefab.toml`, prefab)] } as ProjectSnapshot;
  assert.deepEqual(modelPartsLibrary(project), [{ path: `${folder}/parts/model.prefab.toml`, name: "Edited model", modelDescriptor: `${folder}/object.asset.json` }]);
});

test("unsaved, legacy without a library prefab, and damaged part sets do not become broken library tiles", () => {
  assert.deepEqual(modelPartsLibrary({ files: base } as ProjectSnapshot), []);
  assert.deepEqual(modelPartsLibrary({ files: [base[0], source(`${folder}/parts/model.prefab.toml`, prefab)] } as ProjectSnapshot), []);
  assert.deepEqual(modelPartsLibrary({ files: [...base, source(`${folder}/parts/model.prefab.toml`, "bad toml [")] } as ProjectSnapshot), []);
});
