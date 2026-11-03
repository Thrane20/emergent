import assert from "node:assert/strict";
import test from "node:test";
import {parse} from "smol-toml";
import {hexToLinear,linearToHex,materialSettings} from "../src/lib/material-settings";
import {replaceInstanceMaterial} from "../src/lib/scene-placement";

test("material settings validate channels and convert picker sRGB to linear values",()=>{
  assert.equal(materialSettings(undefined).shading,"flat");
  assert.equal(linearToHex(hexToLinear("#8040ff")),"#8040ff");
  assert.ok(Math.abs(hexToLinear("#808080")[0]-0.21586)<0.0001);
  for(const input of [{albedo:[1,0]},{specular:[-1,0,0]},{shininess:0},{emission_intensity:Infinity},{bump_strength:3},{shading:"pbr"},{unknown:true},{bump_map:{asset:"bad.png"}},{albedo_map:{asset:"assets/a.noise.toml",strength:-1}}])assert.throws(()=>materialSettings(input));
});
test("material edits preserve physics, transforms, other instances and reset to asset defaults",()=>{
  const source='# keep\nschema_version=1\nid="main"\n[[entities]]\nid="a"\nasset="assets/a.entity.toml"\n[entities.physics]\nbody="static"\n[entities.transform]\ntranslation=[0,1,0]\n[[entities]]\nid="b"\nasset="assets/a.entity.toml"\n';
  const original=parse(source),material=materialSettings({shading:"smooth",albedo:[1,0,0],bump_map:{asset:"assets/grain.noise.toml"}});
  const once=replaceInstanceMaterial(source,"a",material);
  const twice=replaceInstanceMaterial(once,"a",{...material,emission:[0,1,0]});
  const items=parse(twice).entities as Record<string,unknown>[];
  assert.deepEqual(items[0].physics,(original.entities as Record<string,unknown>[])[0].physics);
  assert.deepEqual(items[0].transform,(original.entities as Record<string,unknown>[])[0].transform);
  assert.deepEqual(items[1],(original.entities as unknown[])[1]);
  assert.deepEqual(parse(replaceInstanceMaterial(twice,"a",null)),original);
  assert.ok(once.startsWith('# keep'));
});

test("referenced noise changes invalidate the material scene revision",async t=>{
  const {mkdtemp,mkdir,writeFile,rm}=await import("node:fs/promises");const os=await import("node:os");const path=await import("node:path");
  const {scenePlacement}=await import("../src/lib/scene-placement");
  const root=await mkdtemp(path.join(os.tmpdir(),"emergent-material-"));t.after(()=>rm(root,{recursive:true,force:true}));
  await mkdir(path.join(root,"assets"));
  await writeFile(path.join(root,"assets/main.scene.toml"),'schema_version=1\nid="main"\n[[entities]]\nid="cube"\nasset="assets/cube.entity.toml"\n');
  await writeFile(path.join(root,"assets/cube.entity.toml"),'schema_version=1\nid="cube"\nkind="primitive"\n[geometry]\ncolor=[1,1,1,1]\n[material.albedo_map]\nasset="assets/grain.noise.toml"\n');
  await writeFile(path.join(root,"assets/grain.noise.toml"),'[noise]\nseed=1\n');
  const project={root,id:"test",name:"Test",entryScene:"assets/main.scene.toml",assetRoots:["assets"],files:[],entities:[],connections:[],outputs:[]};
  const before=await scenePlacement(project,"assets/main.scene.toml");
  assert.equal(before.instances[0].renderable,true);
  assert.equal(before.instances[0].material?.albedo_map?.asset,"assets/grain.noise.toml");
  await writeFile(path.join(root,"assets/grain.noise.toml"),'[noise]\nseed=2\n');
  assert.notEqual((await scenePlacement(project,"assets/main.scene.toml")).revision,before.revision);
});


test("object shadow flags default on, validate booleans and persist independently",()=>{
  const defaults=materialSettings(undefined);assert.equal(defaults.cast_shadows,true);assert.equal(defaults.receive_shadows,true);
  for(const invalid of [{cast_shadows:1},{receive_shadows:"false"}])assert.throws(()=>materialSettings(invalid),/booleans/);
  const source='schema_version=1\nid="shadow"\n[[entities]]\nid="cube"\nasset="assets/cube.entity.toml"\n';
  for(const cast of [false,true])for(const receive of [false,true]){
    const next=replaceInstanceMaterial(source,"cube",{cast_shadows:cast,receive_shadows:receive});
    const material=((parse(next).entities as Record<string,unknown>[])[0].material as Record<string,unknown>);
    assert.equal(material.cast_shadows,cast);assert.equal(material.receive_shadows,receive);
  }
});

test("material map resolution accepts bounded quality choices",()=>{
  for(const size of [32,64,128])assert.equal(materialSettings({noise_resolution:size}).noise_resolution,size);
  assert.throws(()=>materialSettings({noise_resolution:256}),/resolution/);
});

test("diffuse images and named layers validate and survive repeated TOML edits",()=>{
 const material=materialSettings({texture_tiling:[2,3],diffuse_map:{asset:"assets/grass.PNG"},terrain_layers:[{id:"rock",name:"Rock",diffuse_map:{asset:"assets/rock.jpg",scale:0.5}},{id:"snow",name:"Snow"}],active_layer:"rock"});
 assert.equal(material.diffuse_map?.scale,1);assert.deepEqual(material.texture_tiling,[2,3]);
 for(const invalid of [{texture_tiling:[0,1]},{texture_tiling:[1]},{texture_tiling:[1,Infinity]},{diffuse_map:{asset:"bad.svg"}},{diffuse_map:{asset:"ok.png",scale:0}},{terrain_layers:[{id:"a",name:"A"},{id:"a",name:"B"}]},{terrain_layers:[{id:"a",name:""}]},{active_layer:"missing"},{terrain_layers:Array.from({length:9},(_,id)=>({id:String(id),name:"Layer"}))}])assert.throws(()=>materialSettings(invalid));
 const source='# keep\nschema_version=1\nid="main"\n[[entities]]\nid="terrain"\nasset="assets/terrain.entity.toml"\n[entities.physics]\nbody="static"\n[[entities]]\nid="other"\nasset="assets/cube.entity.toml"\n';
 const first=replaceInstanceMaterial(source,"terrain",material),second=replaceInstanceMaterial(first,"terrain",{...material,active_layer:"snow",terrain_layers:material.terrain_layers.map(layer=>({...layer,name:layer.name+" edited"}))});
 const entries=parse(second).entities as Record<string,unknown>[];assert.equal((entries[0].material as Record<string,unknown>).active_layer,"snow");assert.equal(((entries[0].material as Record<string,unknown>).terrain_layers as unknown[]).length,2);assert.deepEqual(entries[0].physics,(parse(source).entities as Record<string,unknown>[])[0].physics);assert.deepEqual(entries[1],(parse(source).entities as Record<string,unknown>[])[1]);assert.deepEqual(parse(replaceInstanceMaterial(second,"terrain",null)),parse(source));
});

test("referenced image bytes invalidate a material scene revision",async t=>{
 const {mkdtemp,mkdir,writeFile,rm}=await import("node:fs/promises"),os=await import("node:os"),path=await import("node:path"),{scenePlacement}=await import("../src/lib/scene-placement");
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-image-revision-"));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(path.join(root,"assets"));
 await writeFile(path.join(root,"assets/main.scene.toml"),'schema_version=1\nid="main"\n[[entities]]\nid="cube"\nasset="assets/cube.entity.toml"\n');
 await writeFile(path.join(root,"assets/cube.entity.toml"),'schema_version=1\nid="cube"\nkind="primitive"\n[geometry]\ncolor=[1,1,1,1]\n[material.diffuse_map]\nasset="assets/color.png"\nscale=1\n');
 const project={root,id:"test",name:"Test",entryScene:"assets/main.scene.toml",assetRoots:["assets"],files:[],entities:[],connections:[],outputs:[]};
 // This checks revision hashing only; full image decoding is validated natively.
 await writeFile(path.join(root,"assets/color.png"),Buffer.from([1,2,3,4]));const before=await scenePlacement(project,"assets/main.scene.toml");
 await writeFile(path.join(root,"assets/color.png"),Buffer.from([4,3,2,1]));assert.notEqual((await scenePlacement(project,"assets/main.scene.toml")).revision,before.revision);
});

test("library references persist without inline overrides and reject ambiguous settings",()=>{
 const source='schema_version=1\nid="main"\n[[entities]]\nid="a"\nasset="assets/a.entity.toml"\n';
 const reference={asset:"assets/materials/stone.material.toml"};
 assert.deepEqual(materialSettings(reference),reference);
 assert.deepEqual(JSON.parse(JSON.stringify((parse(replaceInstanceMaterial(source,"a",reference)).entities as Record<string,unknown>[])[0].material)),reference);
 assert.throws(()=>materialSettings({...reference,albedo:[1,0,0]}));assert.throws(()=>materialSettings({asset:"bad.png"}));
});

test("material library reads expose plain nested objects for server-rendered generators",async t=>{
 const {mkdtemp,mkdir,writeFile,rm}=await import("node:fs/promises"),os=await import("node:os"),path=await import("node:path"),{readMaterial}=await import("../src/lib/material-assets");
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-material-library-"));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(path.join(root,"assets"));
 const relative="assets/stone.material.toml";await writeFile(path.join(root,relative),'schema_version=1\nid="stone"\nname="Stone"\n[material]\nterrain_layers=[{id="old",name="Old",diffuse_map={asset="assets/old.png",scale=1}}]\n[material.diffuse_map]\nasset="assets/stone.png"\nscale=2\n');
 const project={root,id:"test",name:"Test",entryScene:"assets/main.scene.toml",assetRoots:["assets"],files:[],entities:[],connections:[],outputs:[]},loaded=await readMaterial(project,relative);
 assert.equal(Object.getPrototypeOf(loaded.material),Object.prototype);assert.equal(Object.getPrototypeOf(loaded.material.diffuse_map),Object.prototype);assert.equal(Object.getPrototypeOf(loaded.material.terrain_layers[0]),Object.prototype);assert.equal(Object.getPrototypeOf(loaded.material.terrain_layers[0].diffuse_map),Object.prototype);
 await writeFile(path.join(root,relative),'schema_version=1\nid="stone"\nname="Stone"\n');await assert.rejects(readMaterial(project,relative),/material.*table/);
 await writeFile(path.join(root,relative),'schema_version=1\nid="stone"\nname="Stone"\n[material]\nasset="assets/stone.material.toml"\n');await assert.rejects(readMaterial(project,relative),/cannot reference/);
});
