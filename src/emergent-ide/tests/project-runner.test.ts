import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { setTimeout } from "node:timers/promises";
import { projectRunManager } from "../src/lib/project-runner";
import type { ProjectSnapshot, ProjectFile } from "../src/lib/project";
import { projectTargetDir } from "../src/lib/workspace";

test("Run passes the selected scene to Cargo and rejects invalid selections before spawning",async t=>{
  const root=await mkdtemp(path.join(os.tmpdir(),"emergent-run-"));
  const previous=process.env.PATH;
  t.after(async()=>{if(previous===undefined)delete process.env.PATH;else process.env.PATH=previous;await rm(root,{recursive:true,force:true});});
  await mkdir(path.join(root,"assets"));await mkdir(path.join(root,"bin"));
  await writeFile(path.join(root,"Cargo.toml"),'[package]\nname="fixture"\n');
  for(const name of ["main","physics"]){await writeFile(path.join(root,`assets/${name}.scene.toml`),`id="${name}"\n`);}
  const file=(name:string):ProjectFile=>({path:`assets/${name}.scene.toml`,name,kind:"scene",bytes:0,modifiedAt:"",content:`id="${name}"\n`,authored:true,generated:false});
  const project:ProjectSnapshot={root,id:"fixture",name:"Fixture",entryScene:"assets/main.scene.toml",assetRoots:["assets"],files:[file("main"),file("physics"),file("escape")],entities:[],connections:[],outputs:[]};
  // Exercise the real spawn arguments without opening a window or building a fixture.
  await writeFile(path.join(root,"bin/cargo"),`#!${process.execPath}\nconsole.log('ARGS='+JSON.stringify(process.argv.slice(2)));\n`,{mode:0o700});
  process.env.PATH=`${path.join(root,"bin")}${path.delimiter}${previous??""}`;
  for(const value of [undefined,null,"", "../outside.scene.toml","assets/missing.scene.toml","Cargo.toml"])await assert.rejects(projectRunManager.start(project,value),/Choose a scene/);
  await symlink(path.resolve("tests/project-runner.test.ts"),path.join(root,"assets/escape.scene.toml"));
  await assert.rejects(projectRunManager.start(project,"assets/escape.scene.toml"),/missing or outside/);
  // A listed but missing scene is rejected as well.
  project.files.push(file("deleted"));
  await assert.rejects(projectRunManager.start(project,"assets/deleted.scene.toml"),/missing or outside/);
  for(const selected of ["assets/physics.scene.toml","assets/main.scene.toml"]){
    const started=await projectRunManager.start(project,selected);
    assert.equal(started.scenePath,selected);
    await assert.rejects(projectRunManager.start(project,selected),/already running/);
    for(let attempt=0;attempt<200&&projectRunManager.status().state!=="succeeded";attempt++)await setTimeout(10);
    const result=projectRunManager.status();assert.equal(result.state,"succeeded",result.output);
    const line=result.output.split("\n").find(line=>line.startsWith("ARGS="));assert.ok(line);
    assert.deepEqual(JSON.parse(line.slice(5)),["run","--manifest-path",path.join(root,"Cargo.toml"),"--target-dir",projectTargetDir(root),"--","--run-scene",selected]);
  }
});
