import assert from "node:assert/strict";
import test from "node:test";
import {mkdtemp,mkdir,readFile,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {readTomlDocument,saveTomlDocument} from "../src/lib/project-toml";
import type {ProjectSnapshot} from "../src/lib/project";
import { SCENE_MAX_BYTES } from "../src/lib/scene-limits";

test("TOML editor scopes documents and preserves source on syntax, bounds and external-reference conflicts",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-toml-test-"));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(path.join(root,"assets"));
 const target="assets/main.scene.toml",asset="assets/cube.entity.toml",scene='schema_version=1\nid="main"\nentities=[]\n';
 await writeFile(path.join(root,target),scene);await writeFile(path.join(root,asset),'schema_version=1\nid="cube"');
 const files=[target,asset].map(relative=>({path:relative,name:relative.split("/").at(-1)!,kind:"file" as const,content:"",bytes:0,modifiedAt:"",authored:true,generated:false}));
 const project:ProjectSnapshot={root,id:"test",name:"Test",entryScene:target,assetRoots:["assets"],files,entities:[],outputs:[],connections:[]};
 const initial=await readTomlDocument(project,target);assert.equal(initial.content,scene);
 await assert.rejects(saveTomlDocument(project,target,"invalid = [",initial.revision));assert.equal(await readFile(path.join(root,target),"utf8"),scene);
 // Scenes may be up to 2 MiB (4,096 instances); other TOML stays at 256 KiB.
 await assert.rejects(saveTomlDocument(project,target," ".repeat(SCENE_MAX_BYTES+1),initial.revision),/2 MiB/);
 await writeFile(path.join(root,asset),'schema_version=1\nid="changed"');
 await assert.rejects(saveTomlDocument(project,target,scene+"# edit",initial.revision),/changed externally/);
 assert.equal(await readFile(path.join(root,target),"utf8"),scene);
 await assert.rejects(readTomlDocument(project,"emergent.toml"),/authored scene, entity, noise or tween/);
 await assert.rejects(readTomlDocument(project,"assets/../secret.entity.toml"),/authored scene, entity, noise or tween/);
});


test("small TOML documents open with large terrain/code dependencies and detect same-size external edits",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-toml-large-"));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(path.join(root,"assets"));await mkdir(path.join(root,"src"));
 const target="assets/ground.entity.toml",heights="assets/terrain.heights.toml",code="src/generated.rs",content='schema_version=1\nid="ground"\nkind="primitive"\n[geometry]\nprimitive="plane"\n';
 await writeFile(path.join(root,target),content);const large='#'+"x".repeat(320*1024)+"\n";await writeFile(path.join(root,heights),large);await writeFile(path.join(root,code),"//"+large);
 const files=[target,heights,code].map(relative=>({path:relative,name:relative.split("/").at(-1)!,kind:"file" as const,content:null,bytes:0,modifiedAt:"",authored:true,generated:false}));
 const project:ProjectSnapshot={root,id:"test",name:"Test",entryScene:"assets/main.scene.toml",assetRoots:["assets"],files,entities:[],outputs:[],connections:[]};
 const initial=await readTomlDocument(project,target);assert.equal(initial.content,content);
 await writeFile(path.join(root,heights),large.replace("x","y"));const after=await readTomlDocument(project,target);assert.notEqual(after.revision,initial.revision);
 await assert.rejects(saveTomlDocument(project,target,content+"# edit",initial.revision),/changed externally/);assert.equal(await readFile(path.join(root,target),"utf8"),content);
 await writeFile(path.join(root,code),"//"+large.replace("x","z"));assert.notEqual((await readTomlDocument(project,target)).revision,after.revision);
 await writeFile(path.join(root,target),large);await assert.rejects(readTomlDocument(project,target),/preview size limit/);
});
