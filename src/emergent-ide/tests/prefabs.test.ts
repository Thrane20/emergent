import test from "node:test";
import assert from "node:assert/strict";
import { parse, stringify } from "smol-toml";
import { duplicateSceneObject, capturePrefab, parsePrefab, placePrefab } from "../src/lib/prefabs";
import type { ComponentType } from "../src/lib/components";
import { SCENE_MAX_INSTANCES } from "../src/lib/scene-limits";

const types: ComponentType[] = [
  { id: "emergent.follow", label: "Follow", description: "", applies_to: "node", fields: [{ name: "target", label: "Target", kind: "entity", help: "" }] },
  { id: "emergent.brain", label: "Brain", description: "", applies_to: "node", fields: [] },
  { id: "project.links", label: "Links", description: "", applies_to: "node", fields: [{ name: "targets", label: "Targets", kind: "entities", help: "" }] },
];
const source = stringify({ schema_version: 1, id: "scene", entities: [
  { id: "outside", asset: "assets/body.entity.toml" },
  { id: "body", parent: "outside", asset: "assets/body.entity.toml", transform: { translation: [10, 2, 0], scale: [2, 2, 2] }, components: [{ type: "emergent.brain", brain: "assets/drone.brain.toml" }] },
  { id: "rotor", parent: "body", asset: "assets/rotor.entity.toml", transform: { translation: [1, 0, 0] }, components: [{ type: "emergent.follow", target: "body" }, { type: "project.links", targets: ["body", "rotor"] }] },
] });

test("capture a subtree and place independent, named copies with remapped links", () => {
  const prefab = capturePrefab(source, "body", " Drone ", "drone", types);
  assert.equal(prefab.name, "Drone");
  assert.equal(prefab.entities.length, 2);
  assert.equal(prefab.entities[0].parent, undefined);
  assert.deepEqual(prefab.entities[0].transform, { translation: [0, 0, 0], scale: [2, 2, 2], rotation_degrees: [0, 0, 0] });
  assert.deepEqual(prefab.entities[0].brain_bindings, { body: "body", rotor: "rotor" });
  let id = 0;
  const first = placePrefab(source, prefab, types, () => `new${++id}`);
  const second = placePrefab(first.content, prefab, types, () => `new${++id}`);
  const entities = parse(second.content).entities as typeof prefab.entities;
  assert.equal(entities.find(item=>item.id===first.root)!.prefab_label,"Drone");
  assert.equal(first.root, "new1"); assert.equal(second.root, "new3");
  assert.equal(entities[4].parent, "new1"); assert.equal(entities[6].parent, "new3");
  assert.equal(entities[4].components?.[0].target, "new1");
  assert.deepEqual(entities[6].components?.[1].targets, ["new3", "new4"]);
  assert.deepEqual({ ...entities[5].brain_bindings }, { body: "new3", rotor: "new4" });
  const recaptured = capturePrefab(second.content, "new3", "Second drone", "drone2", types);
  assert.deepEqual(recaptured.entities[0].brain_bindings, { body: "new3", rotor: "new4" });
  const third = placePrefab('schema_version=1\nid="empty"\nentities=[]\n', recaptured, types, () => `new${++id}`);
  assert.deepEqual({ ...(parse(third.content).entities as typeof prefab.entities)[0].brain_bindings }, { body: "new5", rotor: "new6" });
  assert.equal(parse(source).entities instanceof Array, true);
});

test("capture refuses external entity links, including declared project fields and brain bindings", () => {
  assert.throws(() => capturePrefab(source.replace('target = "body"', 'target = "outside"'), "body", "Drone", "drone", types), /external object/);
  const prefab = capturePrefab(source, "body", "Drone", "drone", types);
  prefab.entities[0].brain_bindings = { enemy: "outside" };
  let fresh = 0;
  assert.throws(() => placePrefab(source, prefab, types, () => `fresh${++fresh}`), /external object/);
  assert.throws(() => capturePrefab(source, "missing", "Drone", "d", types), /Select/);
});

test("reject invalid prefab roots, recursion, names, identifiers and oversized scenes", () => {
  const prefab = capturePrefab(source, "body", "Drone", "drone", types);
  assert.throws(() => parsePrefab(stringify({ ...prefab, root: "absent" })), /root/);
  assert.throws(() => parsePrefab(stringify({ ...prefab, name: "" })), /Name/);
  assert.throws(() => capturePrefab(source, "body", "Drone", "bad/id", types), /schema/);
  assert.throws(() => parsePrefab(stringify({ ...prefab, entities: [{ id: "a", parent: "a", asset: "assets/a.entity.toml" }] })), /cycle/);
  assert.throws(() => parsePrefab(stringify({ ...prefab, entities: [{ id: "a", asset: "assets/nested.prefab.toml" }] })), /entity assets/);
  assert.throws(() => placePrefab(stringify({ entities: Array(SCENE_MAX_INSTANCES - 1).fill({ id: "a", asset: "assets/a.entity.toml" }) }), prefab, types, () => "id"), /4096/);
  assert.throws(() => placePrefab(source, prefab, types, () => "body"), /fresh and unique/);
});

test("placement preserves scene comments and trailing environment tables", () => {
  const prefab = capturePrefab(source, "body", "Drone", "drone", types);
  let id = 0;
  const scene = '# scene comment\nschema_version=1\nid="main"\nentities=[]\n[environment]\nambient_intensity=0.5\n';
  const placed = placePrefab(scene, prefab, types, () => `p${++id}`);
  assert.ok(placed.content.startsWith("# scene comment"));
  assert.deepEqual(parse(placed.content).environment, parse(scene).environment);
  assert.equal((parse(placed.content).entities as unknown[]).length, 2);
});


test("scene duplication keeps transforms, parent, external links and remaps copied descendants", () => {
  const input=parse(source);
  (input.entities as {name?:string}[])[1].name="Drone";
  const original="# scene comment\n"+stringify(input)+"\n[environment]\nexposure=1.0\n";
  const copy=duplicateSceneObject(original,"body",types,id=>`copy-${id}`);
  const document=parse(copy.content),all=document.entities as Record<string,unknown>[];
  assert.equal(copy.id,"copy-body");
  assert.ok(copy.content.startsWith(original.trimEnd()));
  assert.deepEqual(all.slice(0,3),parse(original).entities);
  assert.equal(document.environment && (document.environment as {exposure:number}).exposure,1);
  assert.equal(all[3].name,"Drone (1)");
  assert.equal(all[3].parent,"outside");
  assert.deepEqual(all[3].transform,all[1].transform);
  assert.equal(all[4].parent,"copy-body");
  assert.deepEqual(JSON.parse(JSON.stringify(all[4].components)),[{type:"emergent.follow",target:"copy-body"},{type:"project.links",targets:["copy-body","copy-rotor"]}]);
  assert.deepEqual(JSON.parse(JSON.stringify(all[3].brain_bindings)),{outside:"outside",body:"copy-body",rotor:"copy-rotor"});
  const again=duplicateSceneObject(copy.content,copy.id,types,id=>`again-${id}`);
  assert.equal((parse(again.content).entities as {name?:string}[]).at(-2)?.name,"Drone (2)");
});

test("copy numbering skips existing suffixes and collisions; invalid copies are rejected", () => {
  const input=parse(source),all=input.entities as Record<string,unknown>[];
  all[1].name="Drone (2)";all[0].name="Drone (7)";
  const result=duplicateSceneObject(stringify(input),"body",types,id=>`new-${id}`);
  assert.equal((parse(result.content).entities as {name?:string}[]).at(-2)?.name,"Drone (8)");
  const external=duplicateSceneObject(source,"rotor",types,id=>`new-${id}`);
  assert.deepEqual(JSON.parse(JSON.stringify((parse(external.content).entities as {components:unknown}[]).at(-1)?.components)),[{type:"emergent.follow",target:"body"},{type:"project.links",targets:["body","new-rotor"]}]);
  assert.throws(()=>duplicateSceneObject(source,"missing",types,id=>id),/no longer exists/);
  assert.throws(()=>duplicateSceneObject(source,"body",types,id=>id),/fresh and unique/);
  assert.throws(()=>duplicateSceneObject(source,"body",types,()=>"same"),/fresh and unique/);
});

test("capture materializes optional empty wheel defaults without changing their membership",()=>{
 const wheelType:ComponentType={id:"emergent.locomotion",label:"Locomotion",description:"",applies_to:"any",fields:[{name:"steering_wheels",label:"Steering wheels",help:"",kind:"entities",default:[]},{name:"rolling_wheels",label:"Rolling wheels",help:"",kind:"entities",default:[]}]};
 const source=stringify({schema_version:1,id:"scene",entities:[{id:"car",asset:"assets/car.entity.toml",components:[{type:"emergent.locomotion",profile:"assets/car.locomotion.toml"}]}]});
 const captured=capturePrefab(source,"car","Car","car-prefab",[wheelType]);
 assert.deepEqual(captured.entities[0].components?.[0].rolling_wheels,[]);
 assert.deepEqual(captured.entities[0].components?.[0].steering_wheels,[]);
 assert.equal(captured.entities[0].components?.[0].profile,"assets/car.locomotion.toml");
});
