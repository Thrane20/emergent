import assert from "node:assert/strict";
import test from "node:test";
import {mkdir,mkdtemp,readFile,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {parse} from "smol-toml";
import {DEFAULT_TWEEN,EASINGS,ease,sampleTween,tweenAsset,tweenSettings} from "../src/lib/tween";
import {prepareTweenSave,runTweenTool,saveTween} from "../src/lib/tween-assets";
import {readProjectSnapshot} from "../src/lib/project";
import {readTomlDocument,saveTomlDocument} from "../src/lib/project-toml";

test("easing endpoints, playback, reverse/constant ranges and invalid settings",()=>{
  for(const {id} of EASINGS){assert.equal(ease(id,0),0);assert.equal(ease(id,1),1);let previous=0;for(let i=0;i<=100;i++){const value=ease(id,i/100);assert.ok(value>=previous&&value<=1);previous=value;}}
  const settings={...DEFAULT_TWEEN,easing:"linear" as const};
  for(const [time,value] of [[-1,-3],[0,-3],[1,0],[2,3],[3,0],[4,-3]])assert.equal(sampleTween(settings,time),value);
  assert.equal(sampleTween({...settings,playback:"once"},20),3);
  assert.equal(sampleTween({...settings,playback:"loop"},2),-3);
  assert.equal(sampleTween({...settings,from:5,to:-5},0),5);
  assert.equal(sampleTween({...settings,from:2,to:2},1),2);
  for(const patch of [{duration_seconds:0},{duration_seconds:NaN},{duration_seconds:3601},{from:Infinity},{to:1000001},{easing:"unknown"},{playback:"unknown"},{extra:true},{duration_seconds:"2"}])assert.throws(()=>tweenSettings({...settings,...patch}));
  assert.throws(()=>tweenSettings(null));
});

test("native sampler matches browser curves; saved variants keep independent identities and validate source edits",{timeout:120000},async(t)=>{
  const root=await mkdtemp(path.join(os.tmpdir(),"emergent-tween-test-")),oldRoot=process.env.EMERGENT_PROJECT_PATH,oldData=process.env.EMERGENT_IDE_DATA_DIR;
  process.env.EMERGENT_PROJECT_PATH=root;process.env.EMERGENT_IDE_DATA_DIR=path.join(root,"private");
  t.after(async()=>{if(oldRoot===undefined)delete process.env.EMERGENT_PROJECT_PATH;else process.env.EMERGENT_PROJECT_PATH=oldRoot;if(oldData===undefined)delete process.env.EMERGENT_IDE_DATA_DIR;else process.env.EMERGENT_IDE_DATA_DIR=oldData;await rm(root,{recursive:true,force:true});});
  await mkdir(path.join(root,"assets"));await mkdir(path.join(root,"src"));
  await writeFile(path.join(root,"emergent.toml"),'id="tween-test"\nname="Tween test"\nasset_roots=["assets"]\nentry_scene="assets/main.scene.toml"\n');
  await writeFile(path.join(root,"Cargo.toml"),`[dependencies]\nemergent={path=${JSON.stringify(path.resolve("../.."))}}\n`);
  const project=await readProjectSnapshot();
  for(const easing of EASINGS){
    const settings={...DEFAULT_TWEEN,easing:easing.id,from:1.25,to:-4.5,duration_seconds:2.5};
    const {change}=prepareTweenSave(project,{name:"Comparison",settings});
    const {samples}=parse(await runTweenTool(project,change.content)) as {samples:number[]};
    assert.equal(samples.length,201);
    for(let i=0;i<samples.length;i++)assert.ok(Math.abs(samples[i]-sampleTween(settings,i/100*settings.duration_seconds))<0.00001,`${easing.id}: sample ${i}`);
  }
  const first=await saveTween(project,{name:"Movement",settings:DEFAULT_TWEEN}),second=await saveTween(project,{name:"Movement",settings:{...DEFAULT_TWEEN,easing:"linear"}});
  assert.notEqual(first.id,second.id);assert.notEqual(first.path,second.path);assert.equal(first.name,second.name);
  const content=await readFile(path.join(root,first.path),"utf8");assert.deepEqual(tweenAsset(first.path,content),first);
  let current=await readProjectSnapshot();assert.equal(current.files.find(file=>file.path===first.path)?.kind,"tween");
  const document=await readTomlDocument(current,first.path);
  await assert.rejects(saveTomlDocument(current,first.path,content.replace("sine-in-out","invalid"),document.revision),/unknown variant/);
  assert.equal(await readFile(path.join(root,first.path),"utf8"),content);
  await saveTomlDocument(current,first.path,content.replace("sine-in-out","sine-out"),document.revision);
  current=await readProjectSnapshot();assert.equal(tweenAsset(first.path,current.files.find(file=>file.path===first.path)!.content!).settings.easing,"sine-out");
  for(const name of ["","Bad\nname","a".repeat(65)])assert.throws(()=>prepareTweenSave(project,{name,settings:DEFAULT_TWEEN}));
  await assert.rejects(runTweenTool(project,content.replace("schema_version = 1","schema_version = 2")),/invalid schema/);
  await assert.rejects(runTweenTool(project," ".repeat(8193)),/8 KiB/);
});
