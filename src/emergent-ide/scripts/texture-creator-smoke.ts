import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {randomUUID} from "node:crypto";
import {cp,mkdir,mkdtemp,readFile,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {parse,stringify} from "smol-toml";
import {chromium,expect} from "@playwright/test";
import {TextureJobs} from "../src/lib/texture-jobs";
import {readProjectSnapshot} from "../src/lib/project";
import {DEFAULT_TERRAIN} from "../src/lib/terrain";
async function main(){
 const root=path.resolve("../.."),temporary=await mkdtemp(path.join(os.tmpdir(),"emergent-texture-creator-ui-")),projectRoot=path.join(temporary,"game"),data=path.join(temporary,"data"),port=process.env.EMERGENT_TEXTURE_CREATOR_TEST_PORT??"3004",base=`http://localhost:${port}`;
 const previousRoot=process.env.EMERGENT_PROJECT_PATH,previousData=process.env.EMERGENT_IDE_DATA_DIR;process.env.EMERGENT_PROJECT_PATH=projectRoot;process.env.EMERGENT_IDE_DATA_DIR=data;
 await cp(path.join(root,"projects/first_project"),projectRoot,{recursive:true,filter:source=>!["target","dist",".git","node_modules"].includes(path.basename(source))});
 const manifest=parse(await readFile(path.join(projectRoot,"Cargo.toml"),"utf8"));(manifest.package as Record<string,unknown>).name=`emergent-texture-creator-${randomUUID().slice(0,8)}`;(manifest.dependencies as Record<string,unknown>).emergent={path:root,features:["graphics","physics"]};manifest.workspace={};await writeFile(path.join(projectRoot,"Cargo.toml"),stringify(manifest));await cp(path.join(root,"Cargo.lock"),path.join(projectRoot,"Cargo.lock"));
 const scene="assets/scenes/texture-creator-smoke.scene.toml";await mkdir(path.join(projectRoot,"assets/scenes"),{recursive:true});
 await writeFile(path.join(projectRoot,scene),'schema_version=1\nid="texture-creator"\n[[entities]]\nid="cube"\nasset="assets/scenes/creator-cube.entity.toml"\n[[entities]]\nid="terrain"\nasset="assets/scenes/creator-terrain.entity.toml"\n');
 await writeFile(path.join(projectRoot,"assets/scenes/creator-cube.entity.toml"),'schema_version=1\nid="cube"\nkind="primitive"\n[geometry]\nprimitive="cube"\nhalf_extent=1\ncolor=[1,1,1,1]\n');
 const recipe=Object.fromEntries(Object.entries({...DEFAULT_TERRAIN,resolution:32,size:6,height:0.5}).filter(([key])=>!key.startsWith("sun")));
 await writeFile(path.join(projectRoot,"assets/scenes/creator-terrain.entity.toml"),stringify({schema_version:1,id:"terrain",kind:"terrain",terrain:recipe,transform:{translation:[0,-1,-4]},material:{shading:"smooth"}}));
 const project=await readProjectSnapshot();let requests=0;let images:Buffer[]=[];
 const jobs=new TextureJobs(data,async()=>({submit:async()=>{requests++;return "fixture-request";},status:async()=>"completed",result:async()=>images}));
 let logs="";const server=spawn(process.execPath,[path.resolve("node_modules/next/dist/bin/next"),"start","--port",port],{env:{...process.env,FAL_KEY:"fixture-key-no-network",CARGO_TARGET_DIR:path.join(root,"target")},stdio:["ignore","pipe","pipe"]});server.stdout.on("data",chunk=>logs+=String(chunk));server.stderr.on("data",chunk=>logs+=String(chunk));
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true}),page=await browser.newPage({viewport:{width:1600,height:1100}}),errors:string[]=[];let jobId="",session:string|null=null;page.on("pageerror",error=>errors.push(error.message));
 try{
  await expect.poll(async()=>{if(server.exitCode!==null)throw new Error(logs);try{return (await fetch(`${base}/textures`,{signal:AbortSignal.timeout(2000)})).ok;}catch{return false;}},{timeout:30000}).toBe(true);
  await page.goto(`${base}/textures`);await expect(page.getByRole("heading",{name:"Create textures",exact:true})).toBeVisible();
  const encoded=await page.evaluate(()=>["#547b25","#c1a479"].map(colour=>{const c=document.createElement("canvas");c.width=32;c.height=32;const context=c.getContext("2d")!;context.fillStyle=colour;context.fillRect(0,0,32,32);context.fillStyle="#29311f";for(let i=0;i<8;i++)context.fillRect(i*4,i*3,2,2);return c.toDataURL().split(",")[1];}));images=encoded.map(value=>Buffer.from(value,"base64"));
  await page.route("**/api/texture-jobs",async route=>{if(route.request().method()!=="POST"){await route.continue();return;}const body=route.request().postDataJSON();jobId=body.id;const queued=await jobs.create(project,jobId,body.input);await jobs.refresh(project,jobId);await route.fulfill({status:201,json:queued});});
  await page.getByLabel("Texture name",{exact:true}).fill("Pebble ground");await page.getByLabel("Texture prompt",{exact:true}).fill("Small rounded pebbles mixed with moss");await page.getByLabel("Texture candidate count",{exact:true}).selectOption("2");await page.getByRole("button",{name:"Generate textures",exact:false}).click();
  await expect(page.getByRole("checkbox",{name:"Select texture 1",exact:true})).toBeVisible({timeout:120000});await expect(page.locator(".texture-result-image img")).toHaveCount(2);await expect.poll(()=>page.locator(".texture-result-image img").evaluateAll(elements=>elements.every(element=>(element as HTMLImageElement).naturalWidth===32))).toBe(true);
  await page.getByRole("checkbox",{name:"Select texture 1",exact:true}).check();await page.getByRole("checkbox",{name:"Select texture 2",exact:true}).check();await page.getByRole("button",{name:"Add selected to project",exact:true}).click();
  await expect.poll(async()=>(await jobs.get(project,jobId)).outputs.filter(result=>result.savedPath).length,{timeout:120000}).toBe(2);await expect(page.getByText("Available in object and terrain material pickers.",{exact:true})).toHaveCount(2);
  const saved=await jobs.get(project,jobId);assert.equal(requests,1);assert.ok(saved.outputs.every(result=>result.savedPath));
  const retry=await page.request.post(`${base}/api/texture-jobs/${jobId}`,{data:{projectId:project.id,action:"save",resultId:saved.outputs[0].id,name:"Another name"}});assert.equal(retry.status(),200);assert.equal((await retry.json()).outputs[0].savedPath,saved.outputs[0].savedPath);
  await page.screenshot({path:path.join(os.tmpdir(),"emergent-texture-creator.png")});await page.reload();await expect(page.getByText("Available in object and terrain material pickers.",{exact:true})).toHaveCount(2);
  await page.setViewportSize({width:600,height:800});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await page.setViewportSize({width:1600,height:1100});
  await page.goto(`${base}/?scene=${encodeURIComponent(scene)}`);const canvas=page.getByLabel("Live Rust inspection viewport",{exact:true});await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(2);session=await canvas.getAttribute("data-session");
  await page.getByRole("button",{name:"Select instance cube",exact:true}).click();const material=page.locator(".material-inspector");await material.getByRole("button",{name:"Diffuse image",exact:true}).click();const picker=material.getByRole("dialog",{name:"Choose Diffuse image",exact:true});await expect(picker.getByRole("button",{name:"Pebble ground 1",exact:true})).toBeVisible();await expect(picker.getByRole("button",{name:"Pebble ground 2",exact:true})).toBeVisible();
  const assigned=page.waitForResponse(response=>response.url().endsWith("/api/scenes/placement")&&response.request().method()==="PUT",{timeout:120000});await picker.getByRole("button",{name:"Pebble ground 1",exact:true}).click();const response=await assigned;assert.equal(response.status(),200,await response.text());
  await expect(material.getByRole("status")).toHaveText("Saved automatically",{timeout:120000});await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(2);session=await canvas.getAttribute("data-session");
  await page.getByRole("button",{name:"Select instance terrain",exact:true}).click();const added=page.waitForResponse(response=>response.url().endsWith("/api/scenes/placement")&&response.request().method()==="PUT",{timeout:120000});await material.getByRole("button",{name:"Add material layer",exact:true}).click();assert.equal((await added).status(),200);await expect(material.getByRole("status")).toHaveText("Saved automatically",{timeout:120000});
  await material.getByRole("button",{name:"Layer 1 image",exact:true}).click();await expect(material.getByRole("dialog",{name:"Choose Layer 1 image",exact:true}).getByRole("button",{name:"Pebble ground 2",exact:true})).toBeVisible();
  assert.deepEqual(errors,[]);console.log("Texture Creator smoke passed: prompt/batch entry, candidates, multi-save/retry, restart history, responsive layout, and generated images in object/terrain material pickers. Mocked fal; real native validation and project saves.");
 }finally{if(session)await page.request.delete(`${base}/api/inspection`,{data:{projectId:project.id,id:session}}).catch(()=>{});await browser.close();server.kill("SIGTERM");await new Promise<void>(resolve=>{if(server.exitCode!==null)resolve();else server.once("exit",()=>resolve());});if(previousRoot===undefined)delete process.env.EMERGENT_PROJECT_PATH;else process.env.EMERGENT_PROJECT_PATH=previousRoot;if(previousData===undefined)delete process.env.EMERGENT_IDE_DATA_DIR;else process.env.EMERGENT_IDE_DATA_DIR=previousData;await rm(temporary,{recursive:true,force:true});}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
