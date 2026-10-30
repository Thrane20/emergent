import assert from "node:assert/strict";
import test from "node:test";
import {mkdir,mkdtemp,readFile,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {parse,stringify} from "smol-toml";
import {prepareTerrainSave,prepareTerrainUpdate,readTerrainEdit,terrainSceneRevision} from "../src/lib/terrain-assets";
import {applyChanges} from "../src/lib/statement-files";
import {readProjectSnapshot} from "../src/lib/project";
import {DEFAULT_TERRAIN} from "../src/lib/terrain";
import {physicsSettings,terrainSceneFrame,type SceneInstance} from "../src/lib/scene-placement";

test("terrain saves a recipe and unique scene instance together, preserving comments and rejecting stale saves",async(t)=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-terrain-save-")),previous=process.env.EMERGENT_IDE_DATA_DIR,previousRoot=process.env.EMERGENT_PROJECT_PATH;process.env.EMERGENT_PROJECT_PATH=root;process.env.EMERGENT_IDE_DATA_DIR=path.join(root,"data");
 t.after(async()=>{if(previous===undefined)delete process.env.EMERGENT_IDE_DATA_DIR;else process.env.EMERGENT_IDE_DATA_DIR=previous;if(previousRoot===undefined)delete process.env.EMERGENT_PROJECT_PATH;else process.env.EMERGENT_PROJECT_PATH=previousRoot;await rm(root,{recursive:true,force:true});});
 await mkdir(path.join(root,"assets"));await mkdir(path.join(root,"src"));
 const scenePath="assets/target.scene.toml",before='# preserve comment\nschema_version=1\nid="target"\nentities=[]\n';
 await writeFile(path.join(root,"emergent.toml"),'id="test"\nname="Test"\nasset_roots=["assets"]\nentry_scene="assets/other.scene.toml"');await writeFile(path.join(root,scenePath),before);
 const project=await readProjectSnapshot();
 const input={scenePath,sceneRevision:terrainSceneRevision(before),name:"Mountain",settings:{...DEFAULT_TERRAIN,preset:"mountainous"},collider:true};
 const first=await prepareTerrainSave(project,input),second=await prepareTerrainSave(project,input);
 assert.notEqual(first.result.id,second.result.id);assert.notEqual(first.result.path,second.result.path);
 const asset=parse(first.changes[0].content);assert.equal(asset.kind,"terrain");assert.equal((asset.material as Record<string,unknown>).shading,"smooth");assert.equal((asset.terrain as Record<string,unknown>).seed,42);assert.equal((asset.terrain as Record<string,unknown>).sunStrength,undefined);
 assert.deepEqual(JSON.parse(JSON.stringify(asset.physics)),{enabled:true,body:"static",friction:0.5,restitution:0,collider:{shape:"terrain"}});
 const heights=Array.from({length:128*128},(_,i)=>Math.fround(i%31/31*18));
 const sculpted=await prepareTerrainSave(project,{...input,heights});assert.equal(sculpted.changes.length,3);
 const heightAsset=parse(sculpted.changes[0].content),sculptAsset=parse(sculpted.changes[1].content);
 assert.deepEqual(heightAsset.values,heights);assert.equal(sculptAsset.terrain_heights,sculpted.changes[0].path);
 for(const invalid of [[],new Array(128*128).fill(9999),new Array(128*128).fill(null)])await assert.rejects(prepareTerrainSave(project,{...input,heights:invalid}),/height field/);
 await applyChanges(project,first.changes);assert.ok((await readFile(path.join(root,scenePath),"utf8")).startsWith("# preserve comment"));
 await assert.rejects(prepareTerrainSave(project,input),/Scene changed/);await assert.rejects(applyChanges(project,second.changes),/changed/);
 await assert.rejects(readFile(path.join(root,second.result.path)),/ENOENT/);
 const current={...input,sceneRevision:first.result.sceneRevision};
 for(const invalid of [{scenePath:"../outside.scene.toml"},{name:""},{name:"bad\nname"},{collider:"true"},{settings:{resolution:2048}}])await assert.rejects(prepareTerrainSave(project,{...current,...invalid}));
});

test("terrain colliders are static and terrain framing respects parent scale and world positions",()=>{
 assert.equal(physicsSettings({body:"static",collider:{shape:"terrain"}}).collider.shape,"terrain");
 for(const invalid of [{body:"dynamic",collider:{shape:"terrain"}},{body:"static",collider:{shape:"terrain",half_extents:[1,1,1]}}])assert.throws(()=>physicsSettings(invalid));
 const identity={translation:[0,0,0],rotation_degrees:[0,0,0],scale:[1,1,1]};
 const instances:SceneInstance[]=[{id:"parent",asset:"a",parent:null,defaults:identity,transform:{...identity,scale:[2,1,3]},overridden:true},{id:"terrain",asset:"b",parent:"parent",defaults:identity,transform:identity,overridden:false,terrain:true,terrainExtent:{size:100,height:38}}];
 const frame=terrainSceneFrame(instances,new Map([["terrain",{translation:[5,10,20]}]]))!;assert.deepEqual(frame.target,[5,10,20]);assert.ok(frame.distance>500&&frame.distance<1000);
 assert.equal(terrainSceneFrame([],new Map()),undefined);
});


test("existing terrain loads frozen samples and updates the same asset without changing scene, materials or physics",async(t)=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-terrain-edit-")),previous=process.env.EMERGENT_PROJECT_PATH,previousData=process.env.EMERGENT_IDE_DATA_DIR;
 process.env.EMERGENT_PROJECT_PATH=root;process.env.EMERGENT_IDE_DATA_DIR=path.join(root,"data");
 t.after(async()=>{if(previous===undefined)delete process.env.EMERGENT_PROJECT_PATH;else process.env.EMERGENT_PROJECT_PATH=previous;if(previousData===undefined)delete process.env.EMERGENT_IDE_DATA_DIR;else process.env.EMERGENT_IDE_DATA_DIR=previousData;await rm(root,{recursive:true,force:true});});
 await mkdir(path.join(root,"assets"));await mkdir(path.join(root,"src"));
 const scenePath="assets/main.scene.toml",assetPath="assets/ground.entity.toml",heightPath="assets/ground.heights.toml",settings={...DEFAULT_TERRAIN,resolution:32};
 const scene='# keep scene comments\nschema_version=1\nid="scene"\n[[entities]]\nid="ground"\nasset="assets/ground.entity.toml"\nname="Painted instance"\n[entities.transform]\ntranslation=[2.0,3.0,4.0]\n';
 const recipe=Object.fromEntries(Object.entries(settings).filter(([key])=>!key.startsWith("sun")));
 const asset='# keep asset comments\n'+stringify({schema_version:1,id:"stable-terrain-id",name:"Shared ground",kind:"terrain",terrain_heights:heightPath,terrain:recipe,material:{shading:"flat",albedo:[0.1,0.2,0.3]},physics:{enabled:false,body:"static",friction:0.7,restitution:0.2,collider:{shape:"terrain"}},transform:{scale:[1,2,1]}});
 await writeFile(path.join(root,"emergent.toml"),'id="edit-test"\nname="Edit"\nasset_roots=["assets"]\nentry_scene="assets/main.scene.toml"');
 await writeFile(path.join(root,scenePath),scene);await writeFile(path.join(root,assetPath),asset);await writeFile(path.join(root,heightPath),stringify({schema_version:1,values:new Array(1024).fill(3.25)}));
 const project=await readProjectSnapshot(),loaded=await readTerrainEdit(project,scenePath,"ground");
 assert.equal(loaded.name,"Painted instance");assert.ok(loaded.heights?.every(h=>h===3.25));assert.equal(loaded.assetPath,assetPath);
 const input={scenePath,instanceId:"ground",editRevision:loaded.revision,settings:{...settings,seed:123},heights:new Array(1024).fill(4.5)};
 const update=await prepareTerrainUpdate(project,input),updatedAsset=parse(update.changes.find(change=>change.path===assetPath)!.content),oldAsset=parse(asset);
 assert.equal(update.result.id,"ground");assert.equal(update.result.path,assetPath);assert.equal(update.changes.find(change=>change.path===scenePath)!.content,scene);
 for(const key of ["id","name","physics","material","transform"])assert.deepEqual(updatedAsset[key],oldAsset[key]);
 assert.ok(update.changes.find(change=>change.path===assetPath)!.content.includes("# keep asset comments"));
 await applyChanges(project,update.changes);const reopened=await readTerrainEdit(project,scenePath,"ground");
 assert.equal(reopened.revision,update.result.editRevision);assert.ok(reopened.heights?.every(h=>h===4.5));assert.equal(reopened.settings.seed,123);assert.equal(await readFile(path.join(root,scenePath),"utf8"),scene);
 await assert.rejects(prepareTerrainUpdate(project,input),/changed externally/);
 await assert.rejects(readTerrainEdit(project,"../outside.scene.toml","ground"));await assert.rejects(readTerrainEdit(project,scenePath,"missing"));
 const fresh={...input,editRevision:reopened.revision};const pending=await prepareTerrainUpdate(project,fresh);
 const assetBefore=await readFile(path.join(root,assetPath),"utf8");await writeFile(path.join(root,assetPath),assetBefore+"\n# external edit\n");
 await assert.rejects(prepareTerrainUpdate(project,fresh),/changed externally/);await assert.rejects(applyChanges(project,pending.changes),/changed/);assert.deepEqual(parse(await readFile(path.join(root,heightPath),"utf8")).values,new Array(1024).fill(4.5));
 await writeFile(path.join(root,assetPath),assetBefore);await writeFile(path.join(root,scenePath),scene+"\n# external scene edit\n");await assert.rejects(prepareTerrainUpdate(project,fresh),/changed externally/);await writeFile(path.join(root,scenePath),scene);
 await writeFile(path.join(root,heightPath),stringify({schema_version:1,values:new Array(1024).fill(5)}));await assert.rejects(prepareTerrainUpdate(project,fresh),/changed externally/);
 await writeFile(path.join(root,heightPath),stringify({schema_version:1,values:[1]}));await assert.rejects(readTerrainEdit(project,scenePath,"ground"),/height field/);
 // Recipe-only terrains retain their recipe until the first edited height snapshot is saved.
 const noHeights={...oldAsset};delete noHeights.terrain_heights;await writeFile(path.join(root,assetPath),stringify(noHeights));
 const procedural=await readTerrainEdit(project,scenePath,"ground");assert.equal(procedural.heights,undefined);
 const frozen=await prepareTerrainUpdate(project,{...input,editRevision:procedural.revision});assert.equal(frozen.changes[0].before,null);assert.ok(parse(frozen.changes[1].content).terrain_heights);
 await writeFile(path.join(root,assetPath),asset.replace('kind = "terrain"','kind = "primitive"'));await assert.rejects(readTerrainEdit(project,scenePath,"ground"),/terrain asset/);
});

test("terrain paint saves and reopens exact weights, preserves placement and rejects changed shared materials",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-paint-save-")),oldRoot=process.env.EMERGENT_PROJECT_PATH,oldData=process.env.EMERGENT_IDE_DATA_DIR;
 process.env.EMERGENT_PROJECT_PATH=root;process.env.EMERGENT_IDE_DATA_DIR=path.join(root,"data");
 t.after(async()=>{if(oldRoot===undefined)delete process.env.EMERGENT_PROJECT_PATH;else process.env.EMERGENT_PROJECT_PATH=oldRoot;if(oldData===undefined)delete process.env.EMERGENT_IDE_DATA_DIR;else process.env.EMERGENT_IDE_DATA_DIR=oldData;await rm(root,{recursive:true,force:true});});
 await mkdir(path.join(root,"assets"));await mkdir(path.join(root,"src"));
 const scenePath="assets/main.scene.toml",scene='# keep\nschema_version=1\nid="main"\nentities=[]\n';
 await writeFile(path.join(root,"emergent.toml"),'id="paint-test"\nname="Paint"\nasset_roots=["assets"]\nentry_scene="assets/main.scene.toml"');await writeFile(path.join(root,scenePath),scene);
 const materialPath="assets/ground.material.toml",material='schema_version=1\nid="ground"\nname="Ground"\n[material]\nalbedo=[0.1,0.2,0.3]\n';await writeFile(path.join(root,materialPath),material);
 const project=await readProjectSnapshot(),settings={...DEFAULT_TERRAIN,resolution:32},paint={schema_version:1,resolution:32,materials:[materialPath,materialPath],transitions:[2,5,9],blend:2,weights:Array.from({length:4096},(_,i)=>i%4===0?128:i%4===1?127:0)};
 const input={scenePath,sceneRevision:terrainSceneRevision(scene),name:"Painted ground",settings,collider:true,heights:new Array(1024).fill(4.25),paint};
 const saved=await prepareTerrainSave(project,input);await applyChanges(project,saved.changes);
 const refreshed=await readProjectSnapshot(),loaded=await readTerrainEdit(refreshed,scenePath,saved.result.id);
 assert.deepEqual(loaded.paint,paint);assert.equal(loaded.revision,saved.result.editRevision);assert.ok(loaded.heights?.every(value=>value===4.25));
 const beforeScene=await readFile(path.join(root,scenePath),"utf8"),beforeAsset=parse(await readFile(path.join(root,saved.result.path),"utf8"));
 const updateInput={...input,instanceId:saved.result.id,editRevision:loaded.revision,paint:{...paint,blend:3,weights:paint.weights.map((value,i)=>i%4===0?255:0)}};
 const updated=await prepareTerrainSave(refreshed,updateInput);await applyChanges(refreshed,updated.changes);
 const reopened=await readTerrainEdit(refreshed,scenePath,saved.result.id);assert.deepEqual(reopened.paint,updateInput.paint);assert.equal(reopened.revision,updated.result.editRevision);assert.equal(await readFile(path.join(root,scenePath),"utf8"),beforeScene);
 const afterAsset=parse(await readFile(path.join(root,saved.result.path),"utf8"));for(const key of ["id","physics","material"])assert.deepEqual(afterAsset[key],beforeAsset[key]);
 const stale={...updateInput,editRevision:reopened.revision};await writeFile(path.join(root,materialPath),material+"\n# external edit\n");await assert.rejects(prepareTerrainSave(refreshed,stale),/changed externally/);
 const current=await readTerrainEdit(refreshed,scenePath,saved.result.id);const cleared=await prepareTerrainSave(refreshed,{...stale,editRevision:current.revision,paint:null});await applyChanges(refreshed,cleared.changes);assert.equal((await readTerrainEdit(refreshed,scenePath,saved.result.id)).paint,undefined);
 await assert.rejects(prepareTerrainSave(refreshed,{...input,sceneRevision:terrainSceneRevision(beforeScene),paint:{...paint,weights:[255,0,0,0]}}),/paint|resolution/i);
});
