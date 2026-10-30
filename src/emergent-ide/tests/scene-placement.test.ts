import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, readFile, rm, symlink, truncate, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse } from "smol-toml";
import { replaceInstanceParent, replaceInstanceTransform, replaceSceneSubtree, saveScenePlacement, scenePlacement } from "../src/lib/scene-placement";
import { sceneHierarchy, validParents } from "../src/lib/scene-hierarchy";
import type { ProjectSnapshot } from "../src/lib/project";

const source = '# keep scene comment\nschema_version=1\nid="main"\n[[entities]]\nid="first"\nasset="assets/cube.entity.toml"\n# keep instance comment\n[[entities]]\nid="second"\nasset="assets/cube.entity.toml"\nparent="first"\n';
const values = {translation:[4,5,6],rotation_degrees:[0,45,90],scale:[2,2,2]};

test("derived scene tree and reparenting preserve local values and reject cycles",()=>{
  const nodes=[{id:"a",parent:null},{id:"b",parent:"a"},{id:"c",parent:null}];
  assert.deepEqual(sceneHierarchy(nodes).map(row=>[row.instance.id,row.depth]),[["a",0],["b",1],["c",0]]);
  assert.deepEqual(validParents(nodes,"a").map(node=>node.id),["c"]);
  assert.throws(()=>sceneHierarchy([{id:"a",parent:"b"},{id:"b",parent:"a"}]),/cycle/);
  assert.throws(()=>sceneHierarchy([{id:"a",parent:"missing"}]),/missing parent/);
  assert.throws(()=>replaceInstanceParent(source,"first","second"),/cycle/);
  assert.throws(()=>replaceInstanceParent(source,"second","second"),/cycle/);
  const unparented=replaceInstanceParent(source,"second",null);
  assert.equal((parse(unparented).entities as {parent?:string}[])[1].parent,undefined);
  const edited=replaceInstanceTransform(unparented,"second",values);
  const reparented=replaceInstanceParent(edited,"second","first");
  assert.deepEqual((parse(reparented).entities as {transform:typeof values}[])[1].transform.translation,values.translation);
  assert.ok(reparented.startsWith("# keep scene comment"));
});

test("transform edits target one instance, preserve unrelated text and reset to inheritance", () => {
  const edited = replaceInstanceTransform(source,"first",values);
  assert.ok(edited.startsWith("# keep scene comment"));
  assert.ok(edited.includes("# keep instance comment"));
  const instances = parse(edited).entities as {id:string;transform?:typeof values;parent?:string}[];
  assert.deepEqual(instances[0].transform?.translation,values.translation);
  assert.equal(instances[1].transform,undefined);
  assert.equal(instances[1].parent,"first");
  const changed = replaceInstanceTransform(edited.replace("scale = [2,2,2]","scale = [2,2,2] # keep scale comment"),"first",{...values,scale:[3,4,5]});
  const reset = replaceInstanceTransform(changed,"first",null);
  assert.equal((parse(reset).entities as {transform?:unknown}[])[0].transform,undefined);
  assert.ok(reset.includes("# keep instance comment"));
  assert.ok(reset.includes("# keep scale comment"));
  assert.throws(() => replaceInstanceTransform(source,"absent",values),/no longer exists/);
  assert.throws(() => replaceInstanceTransform('entities=[{id="first",asset="a"}]',"first",values),/tables/);
  assert.throws(() => replaceInstanceTransform(source,"first",{...values,scale:[0,1,1]}),/non-zero/);
});

test("subtree deletion removes descendants and clears a deleted active camera",()=>{
  const original='# keep scene comment\nschema_version=1\nid="main"\nactive_camera="child"\n[[entities]]\nid="root"\nasset="assets/root.entity.toml"\n# root note\n[[entities]]\nid="child"\nasset="assets/child.entity.toml"\nparent="root"\n[[entities]]\nid="grandchild"\nasset="assets/child.entity.toml"\nparent="child"\n[[entities]]\nid="sibling"\nasset="assets/child.entity.toml"\n';
  const deleted=replaceSceneSubtree(original,"child");
  const document=parse(deleted) as {active_camera?:string;entities:{id:string;parent?:string}[]};
  assert.deepEqual(document.entities.map(entity=>entity.id),["root","sibling"]);
  assert.equal(document.active_camera,undefined);
  assert.ok(deleted.startsWith("# keep scene comment"));
  assert.ok(deleted.includes("# root note"));
  assert.throws(()=>replaceSceneSubtree(original,"missing"),/no longer exists/);
  assert.throws(()=>replaceSceneSubtree('entities=[{id="root",asset="a"}]',"root"),/tables/);
});

test("inspector resolves per-vector defaults and rejects stale, invalid and escaping edits before saving", async t => {
  const root = await mkdtemp(path.join(os.tmpdir(),"emergent-placement-"));
  t.after(() => rm(root,{recursive:true,force:true}));
  await mkdir(path.join(root,"assets"));
  await writeFile(path.join(root,"assets/main.scene.toml"),source);
  const asset = 'schema_version=1\nid="cube"\nkind="primitive"\n[transform]\ntranslation=[2.0,0.0,0.0]\nscale=[3.0,3.0,3.0]\n';
  await writeFile(path.join(root,"assets/cube.entity.toml"),asset);
  const project: ProjectSnapshot = {root,id:"test",name:"Test",assetRoots:["assets"],entryScene:"assets/main.scene.toml",files:[],entities:[],connections:[],outputs:[]};
  const scenePath = "assets/main.scene.toml";
  const first = await scenePlacement(project,scenePath);
  assert.equal(first.instances.length,2);
  assert.deepEqual(first.instances[0].transform.translation,[2,0,0]);
  assert.deepEqual(first.instances[0].transform.scale,[3,3,3]);
  await assert.rejects(saveScenePlacement(project,scenePath,"first",first.revision,{...values,translation:[NaN,0,0]}),/finite/);
  await assert.rejects(saveScenePlacement(project,scenePath,"first",first.revision,{translation:[1,2,3]}),/must supply/);
  await writeFile(path.join(root,scenePath),source+'\n[entities.transform]\ntranslation=[8,0,0]\n');
  const next = await scenePlacement(project,scenePath);
  assert.deepEqual(next.instances[1].transform.translation,[8,0,0]);
  assert.deepEqual(next.instances[1].transform.scale,[3,3,3]);
  assert.notEqual(first.revision,next.revision);
  await assert.rejects(saveScenePlacement(project,scenePath,"first",first.revision,values),/changed externally/);
  await writeFile(path.join(root,"assets/cube.entity.toml"),asset.replace("2.0","7.0"));
  assert.notEqual((await scenePlacement(project,scenePath)).revision,next.revision);
  await assert.rejects(scenePlacement(project,"../secret.scene.toml"),/asset root/);
  await assert.rejects(scenePlacement(project,"assets/../secret.scene.toml"),/asset root/);
  await writeFile(path.join(root,"secret.scene.toml"),"unrelated");
  await symlink(path.dirname(root),path.join(root,"assets/link.scene.toml"));
  await assert.rejects(scenePlacement(project,"assets/link.scene.toml"),/outside/);
  assert.equal(await readFile(path.join(root,"secret.scene.toml"),"utf8"),"unrelated");
});

test("local name edits preserve IDs, comments, parent links and all other instance settings",async()=>{
 const {replaceInstanceName}=await import("../src/lib/scene-placement");
 const original=source.replace('id="first"','id="first"\nname="Old" # retain name note')+'\n[entities.transform]\ntranslation=[1,2,3]\n';
 const updated=replaceInstanceName(original,"first"," Local cube ");
 assert.ok(updated.includes("# keep scene comment"));assert.ok(updated.includes("# retain name note"));
 const before=parse(original),after=parse(updated);
 const entries=after.entities as {id:string;name?:string;parent?:string}[];
 assert.equal(entries[0].name,"Local cube");assert.equal(entries[0].id,"first");assert.equal(entries[1].parent,"first");
 delete entries[0].name;delete (before.entities as {name?:string}[])[0].name;assert.deepEqual(after,before);
 const duplicate=replaceInstanceName(updated,"second","Local cube");assert.equal((parse(duplicate).entities as {name:string}[])[1].name,"Local cube");
 for(const name of ["", " ", "x".repeat(65),"bad\nname",12])assert.throws(()=>replaceInstanceName(source,"first",name),/printable/);
 assert.throws(()=>replaceInstanceName(source,"missing","Name"),/no longer exists/);
});

test("scene snapshots accept large terrain sidecars, track edits and retain bounded asset reads",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-large-terrain-placement-"));t.after(()=>rm(root,{recursive:true,force:true}));
 await mkdir(path.join(root,"assets"));
 await writeFile(path.join(root,"assets/main.scene.toml"),'schema_version=1\nid="main"\n[[entities]]\nid="ground"\nasset="assets/ground.entity.toml"\n');
 // Exercise snapshot loading independently of the game's native transform-report command.
 await writeFile(path.join(root,"assets/ground.entity.toml"),'schema_version=1\nid="ground"\nterrain_heights="assets/ground.heights.toml"\nterrain_paint="assets/ground.paint.toml"\n');
 const heightPath=path.join(root,"assets/ground.heights.toml"),paintPath=path.join(root,"assets/ground.paint.toml");
 await writeFile(heightPath,'schema_version=1\nvalues=['+'0,'.repeat(1024**2)+']\n');
 await writeFile(paintPath,'schema_version=1\nresolution=1024\nweights=['+'255,0,0,0,'.repeat(1024**2)+']\n');
 const project:ProjectSnapshot={root,id:"test",name:"Test",assetRoots:["assets"],entryScene:"assets/main.scene.toml",files:[],entities:[],connections:[],outputs:[]};
 const first=await scenePlacement(project,project.entryScene);assert.equal(first.instances[0].id,"ground");
 await writeFile(heightPath,(await readFile(heightPath,"utf8"))+'# height edit\n');assert.notEqual((await scenePlacement(project,project.entryScene)).revision,first.revision);
 const second=await scenePlacement(project,project.entryScene);await writeFile(paintPath,(await readFile(paintPath,"utf8"))+'# paint edit\n');assert.notEqual((await scenePlacement(project,project.entryScene)).revision,second.revision);
 const {TERRAIN_DATA_MAX_BYTES}=await import("../src/lib/terrain");
 const original=await readFile(paintPath);await truncate(paintPath,TERRAIN_DATA_MAX_BYTES+1);await assert.rejects(scenePlacement(project,project.entryScene),/size limit/);await writeFile(paintPath,original);
 await truncate(heightPath,TERRAIN_DATA_MAX_BYTES+1);await assert.rejects(scenePlacement(project,project.entryScene),/size limit/);
 await writeFile(heightPath,"values=[]\n");await writeFile(path.join(root,"assets/ground.entity.toml"),'#'+'x'.repeat(256*1024));await assert.rejects(scenePlacement(project,project.entryScene),/size limit/);
});
