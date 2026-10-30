import assert from "node:assert/strict";
import test from "node:test";
import { buildProjectPrompt, chatStatementSubject } from "../src/lib/ai-chat";
import type { ProjectFile, ProjectSnapshot } from "../src/lib/project";
const file=(path:string,kind:ProjectFile["kind"],content:string):ProjectFile=>({path,name:path.split("/").at(-1)!,kind,content,bytes:content.length,modifiedAt:"",authored:true,generated:false});
const project:ProjectSnapshot={id:"game",name:"Test game",root:"/unused",entryScene:"assets/main.scene.toml",assetRoots:["assets"],outputs:[],connections:[],entities:[{id:"cube-a",kind:"entity",sourcePath:"assets/main.scene.toml",definition:"scene-entity"},{id:"cube-b",kind:"entity",sourcePath:"assets/main.scene.toml",definition:"scene-entity"}],files:[file("emergent.toml","manifest",'id="game"'),file("assets/main.scene.toml","scene",'schema_version=1\nid="main"\n[[entities]]\nid="cube-a"\nasset="assets/cube.entity.toml"\n[[entities]]\nid="cube-b"\nasset="assets/cube.entity.toml"'),file("assets/cube.entity.toml","entity",'id="cube-asset"\nname="Cube"\n[geometry]\nprimitive="cube"')]};

test("game, scene and object chat scopes carry their actual context",()=>{
 const game=buildProjectPrompt(project,"project:game","hello",[]);
 assert.match(game,/SCOPE\nproject:game/);
 assert.equal(chatStatementSubject(project,"project:game"),null);
 const scene=buildProjectPrompt(project,"file:assets/main.scene.toml","hello",[]);
 assert.match(scene,/Selected file assets\/main.scene.toml/);
 assert.equal(chatStatementSubject(project,"file:assets/main.scene.toml"),"file:assets/main.scene.toml");
 for(const id of ["cube-a","cube-b"]){
  const scope=`entity:assets/main.scene.toml#${id}`;
  const object=buildProjectPrompt(project,scope,"hello",[]);
  assert.ok(object.includes(`Selected entity: ${id}`));
  assert.match(object,/Referenced object asset assets\/cube.entity.toml/);
  assert.match(object,/primitive="cube"/);
  assert.equal(chatStatementSubject(project,scope),"file:assets/cube.entity.toml");
 }
 assert.throws(()=>buildProjectPrompt(project,"entity:assets/main.scene.toml#missing","hello",[]),/no longer present/);
 assert.throws(()=>buildProjectPrompt(project,"project:other","hello",[]),/not part/);
 const missing={...project,files:project.files.filter(file=>file.kind!=="entity")};
 assert.throws(()=>buildProjectPrompt(missing,"entity:assets/main.scene.toml#cube-a","hello",[]),/asset.*no longer present/);
});
