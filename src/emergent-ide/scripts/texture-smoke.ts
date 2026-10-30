import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {randomUUID} from "node:crypto";
import {cp,mkdir,mkdtemp,readFile,readdir,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {parse,stringify} from "smol-toml";
import {chromium,expect} from "@playwright/test";
import {DEFAULT_TERRAIN} from "../src/lib/terrain";
async function main(){
 const root=path.resolve("../.."),temporary=await mkdtemp(path.join(os.tmpdir(),"emergent-texture-ui-")),project=path.join(temporary,"project"),port=process.env.EMERGENT_TEXTURE_TEST_PORT??"3001",base=`http://localhost:${port}`;
 await cp(path.join(root,"projects/first_project"),project,{recursive:true,filter:source=>!["target",".git","node_modules","dist"].includes(path.basename(source))});
 const cargo=parse(await readFile(path.join(project,"Cargo.toml"),"utf8"));(cargo.package as Record<string,unknown>).name=`emergent-texture-smoke-${randomUUID().slice(0,8)}`;(cargo.dependencies as Record<string,unknown>).emergent={path:root,features:["graphics","physics"]};cargo.workspace={};await writeFile(path.join(project,"Cargo.toml"),stringify(cargo));await cp(path.join(root,"Cargo.lock"),path.join(project,"Cargo.lock"));
 const scene="assets/scenes/texture-smoke.scene.toml",projectId=parse(await readFile(path.join(project,"emergent.toml"),"utf8")).id;
 await mkdir(path.join(project,"assets/scenes"),{recursive:true});
 await writeFile(path.join(project,scene),'schema_version=1\nid="texture-smoke"\n[[entities]]\nid="cube"\nasset="assets/scenes/texture-cube.entity.toml"\n[[entities]]\nid="terrain"\nasset="assets/scenes/texture-terrain.entity.toml"\n');
 await writeFile(path.join(project,"assets/scenes/texture-cube.entity.toml"),'schema_version=1\nid="cube"\nkind="primitive"\n[transform]\ntranslation=[-2,0,0]\n[geometry]\nprimitive="cube"\nhalf_extent=1\ncolor=[1,1,1,1]\n[material]\nshading="unlit"\n');
 const recipe=Object.fromEntries(Object.entries({...DEFAULT_TERRAIN,resolution:32,size:6,height:0.7}).filter(([key])=>!key.startsWith("sun")));
 await writeFile(path.join(project,"assets/scenes/texture-terrain.entity.toml"),stringify({schema_version:1,id:"terrain",kind:"terrain",transform:{translation:[0,-1,-4]},terrain:recipe,material:{shading:"unlit"}}));
 let logs="";const server=spawn(process.execPath,[path.resolve("node_modules/next/dist/bin/next"),"start","--port",port],{env:{...process.env,EMERGENT_PROJECT_PATH:project,EMERGENT_IDE_DATA_DIR:path.join(temporary,"data"),CARGO_TARGET_DIR:path.join(root,"target")},stdio:["ignore","pipe","pipe"]});server.stdout.on("data",chunk=>logs+=String(chunk));server.stderr.on("data",chunk=>logs+=String(chunk));
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});const page=await browser.newPage({viewport:{width:1600,height:1100}}),errors:string[]=[];page.on("pageerror",error=>errors.push(error.message));let session:string|null=null;
 try{
  await expect.poll(async()=>{if(server.exitCode!==null)throw new Error(logs);try{return (await fetch(`${base}/terrain`,{signal:AbortSignal.timeout(2000)})).ok;}catch{return false;}},{timeout:30000}).toBe(true);
  await page.goto(`${base}/?scene=${encodeURIComponent(scene)}`);const canvas=page.getByLabel("Live Rust inspection viewport",{exact:true});
  const ready=async()=>{await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(2);session=await canvas.getAttribute("data-session");};await ready();
  const png=await page.evaluate(()=>{const canvas=document.createElement("canvas");canvas.width=32;canvas.height=32;const ctx=canvas.getContext("2d")!;for(let y=0;y<8;y++)for(let x=0;x<8;x++){ctx.fillStyle=(x+y)%2?"#192cf0":"#f01818";ctx.fillRect(x*4,y*4,4,4);}return canvas.toDataURL("image/png").split(",")[1];});
  const jpg=await page.evaluate(()=>{const canvas=document.createElement("canvas");canvas.width=16;canvas.height=16;const ctx=canvas.getContext("2d")!;ctx.fillStyle="#18e62a";ctx.fillRect(0,0,16,16);return canvas.toDataURL("image/jpeg").split(",")[1];});
  const before=await canvas.screenshot();
  async function upload(name:string,encoded:string){const response=await page.request.post(`${base}/api/textures?projectId=${projectId}&name=${encodeURIComponent(name)}`,{headers:{"Content-Type":"application/octet-stream"},data:Buffer.from(encoded,"base64")});assert.equal(response.status(),201,await response.text());return (await response.json()).path as string;}
  const stone=await upload("stone.png",png),grass=await upload("grass.jpg",jpg);
  async function library(name:string,image:string){const response=await page.request.post(`${base}/api/materials`,{data:{projectId,name,material:{shading:"unlit",albedo:[1,1,1],diffuse_map:{asset:image,scale:0.5}}}});assert.equal(response.status(),201,await response.text());return (await response.json()).path as string;}
  const rockMaterial=await library("Rock",stone),grassMaterial=await library("Grass",grass);
  const placement=await (await page.request.get(`${base}/api/scenes/placement?projectId=${projectId}&path=${encodeURIComponent(scene)}`)).json();
  const applied=await page.request.put(`${base}/api/scenes/placement`,{data:{projectId,path:scene,id:"cube",revision:placement.revision,action:"material",material:{asset:rockMaterial}}});assert.equal(applied.status(),200,await applied.text());
  if(session)await page.request.delete(`${base}/api/inspection`,{data:{projectId,id:session}});
  await page.reload();await ready();assert.notDeepEqual(await canvas.screenshot(),before);
  const opened=await (await page.request.get(`${base}/api/terrain?projectId=${projectId}&scene=${encodeURIComponent(scene)}&instance=terrain`)).json();
  const weights=Array.from({length:32*32*4},(_,i)=>i%4===0?(Math.floor(i/4)%32<16?255:0):i%4===1?(Math.floor(i/4)%32<16?0:255):0);
  const paint={schema_version:1,resolution:32,materials:[grassMaterial,rockMaterial],transitions:[0.1,0.3,0.5],blend:0.1,weights};
  const painted=await page.request.post(`${base}/api/terrain/save`,{data:{projectId,scenePath:scene,instanceId:"terrain",editRevision:opened.editRevision,settings:opened.settings,heights:opened.heights,paint}});assert.equal(painted.status(),201,await painted.text());
  const reopened=await (await page.request.get(`${base}/api/terrain?projectId=${projectId}&scene=${encodeURIComponent(scene)}&instance=terrain`)).json();assert.deepEqual(reopened.paint,paint);
  const entries=parse(await readFile(path.join(project,scene),"utf8")).entities as Record<string,unknown>[];assert.equal((entries.find(entity=>entity.id==="cube")!.material as {asset:string}).asset,rockMaterial);
  const folders=await readdir(path.join(project,"assets/textures"));const invalid=await page.request.post(`${base}/api/textures?projectId=${projectId}&name=bad.png`,{headers:{"Content-Type":"application/octet-stream"},data:Buffer.from([137,80,78,71,13,10,26,10,0])});assert.equal(invalid.status(),400);assert.deepEqual(await readdir(path.join(project,"assets/textures")),folders);
  await page.screenshot({path:path.join(os.tmpdir(),"emergent-texture-layers.png")});assert.deepEqual(errors,[]);console.log("Texture smoke passed: PNG/JPEG import, shared material library, native textured cube, two painted terrain materials, exact reopening and corrupt import rejection. Temporary project only.");
 }finally{if(session)await page.request.delete(`${base}/api/inspection`,{data:{projectId,id:session}}).catch(()=>{});await browser.close();server.kill("SIGTERM");await new Promise<void>(resolve=>{if(server.exitCode!==null)resolve();else server.once("exit",()=>resolve());});await rm(temporary,{recursive:true,force:true});}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
