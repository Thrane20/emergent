import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {spawn} from "node:child_process";
import {cp,mkdir,mkdtemp,readFile,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {parse,stringify} from "smol-toml";
import {terrainGlbHeights} from "../src/lib/terrain-brush";
import {DEFAULT_TERRAIN} from "../src/lib/terrain";
import {chromium,expect} from "@playwright/test";

// Own temporary project/server; this smoke never writes the user's linked assets.
async function main(){
 const root=path.resolve("../.."),temporary=await mkdtemp(path.join(os.tmpdir(),"emergent-terrain-save-ui-")),project=path.join(temporary,"project");
 const port=process.env.EMERGENT_TERRAIN_TEST_PORT??"3001",base=`http://localhost:${port}`;
 await cp(path.join(root,"templates/standard"),project,{recursive:true,filter:source=>!["target",".git","node_modules","dist"].includes(path.basename(source))});
 const cargo=parse(await readFile(path.join(project,"Cargo.toml"),"utf8"));(cargo.package as Record<string,unknown>).name=`emergent-terrain-save-smoke-${randomUUID().slice(0,8)}`;(cargo.dependencies as Record<string,unknown>).emergent={path:root,features:["graphics","physics"]};cargo.workspace={};await writeFile(path.join(project,"Cargo.toml"),stringify(cargo));await cp(path.join(root,"Cargo.lock"),path.join(project,"Cargo.lock"));
 const scene="assets/scenes/terrain-save.scene.toml",projectId=parse(await readFile(path.join(project,"emergent.toml"),"utf8")).id;
 await mkdir(path.join(project,"assets/scenes"),{recursive:true});await writeFile(path.join(project,scene),'schema_version=1\nid="terrain-save"\nname="Terrain destination"\n[[entities]]\nid="fixture-sun"\nasset="assets/scenes/terrain-save-sun.entity.toml"\n');
 await writeFile(path.join(project,"assets/scenes/terrain-save-sun.entity.toml"),'schema_version=1\nid="fixture-sun"\nkind="directional-light"\n[transform]\nrotation_degrees=[-45,45,0]\n[light]\ntype="directional"\ncolor=[1,1,1]\nintensity=1.2\n');
 const original=await readFile(path.join(project,"assets/scenes/main.scene.toml"),"utf8");
 let logs="";const server=spawn(process.execPath,[path.resolve("node_modules/next/dist/bin/next"),"start","--port",port],{env:{...process.env,EMERGENT_PROJECT_PATH:project,EMERGENT_IDE_DATA_DIR:path.join(temporary,"data"),CARGO_TARGET_DIR:path.join(root,"target")},stdio:["ignore","pipe","pipe"]});server.stdout.on("data",chunk=>logs+=String(chunk));server.stderr.on("data",chunk=>logs+=String(chunk));
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});let session:string|null=null;
 const page=await browser.newPage({viewport:{width:1600,height:1100}});const errors:string[]=[];page.on("pageerror",error=>errors.push(error.message));
 try{
  await expect.poll(async()=>{if(server.exitCode!==null)throw new Error(logs);try{return (await fetch(`${base}/terrain`,{signal:AbortSignal.timeout(2000)})).ok;}catch{return false;}},{timeout:30000}).toBe(true);
  await page.goto(`${base}/?scene=${encodeURIComponent(scene)}`);await expect(page.getByLabel("Scene viewer",{exact:true})).toHaveValue(scene);
  await expect.poll(async()=>Number(await page.getByLabel("Live Rust inspection viewport",{exact:true}).getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(1);
  await page.getByRole("link",{name:"Terrain Builder",exact:true}).click();await expect(page.getByLabel("Destination scene",{exact:true})).toHaveValue(scene);
  await page.getByRole("button",{name:"Mountainous",exact:true}).click();await page.getByLabel("Terrain resolution",{exact:true}).selectOption("32");
  await page.getByLabel("Width (m)",{exact:true}).fill("100");
  const generation=page.waitForResponse(r=>r.url().endsWith("/api/terrain")&&r.request().method()==="POST",{timeout:120000});await page.getByRole("button",{name:"Generate terrain",exact:true}).click();assert.equal((await generation).status(),200);
  const viewer=page.locator("model-viewer"),heightmap=page.getByLabel("Paint terrain heightmap",{exact:true});
  async function field(){const bytes=Uint8Array.from(await viewer.evaluate(async element=>Array.from(new Uint8Array(await (await fetch((element as HTMLElement&{src:string}).src)).arrayBuffer()))));return terrainGlbHeights(bytes.buffer,{...DEFAULT_TERRAIN,resolution:32,height:38});}
  await heightmap.click();await expect(heightmap).toBeFocused();
  const beforeSpace=await field();await page.keyboard.press("Space");assert.notDeepEqual(await field(),beforeSpace,"click must focus the canvas so Space paints");
  await page.getByRole("button",{name:"Undo stroke",exact:true}).click();
  const originalField=await field(),box=(await heightmap.boundingBox())!;
  await page.mouse.move(box.x+box.width*.2,box.y+box.height*.3);await page.mouse.down();await page.mouse.move(box.x+box.width*.8,box.y+box.height*.3,{steps:12});await page.mouse.up();
  const quick=await field();assert.ok(Math.max(...quick.map((h,i)=>h-originalField[i]))>38*8/255,"a quick drag at default strength must visibly change heights");
  await page.getByRole("button",{name:"Undo stroke",exact:true}).click();assert.deepEqual(await field(),originalField);
  await page.getByLabel("Brush strength",{exact:true}).press("End");
  async function stroke(mode:string){
   await page.getByLabel("Brush mode",{exact:true}).selectOption(mode);const box=(await heightmap.boundingBox())!;
   await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
   await expect(page.getByRole("button",{name:"Save terrain to scene",exact:true})).toBeDisabled();
   await page.waitForTimeout(250);await page.mouse.move(box.x+box.width*.55,box.y+box.height*.5,{steps:4});await page.mouse.up();
  }
  await stroke("raise");const raised=await field();assert.ok(raised.some((h,i)=>h>originalField[i]));
  await page.getByRole("button",{name:"Undo stroke",exact:true}).click();assert.deepEqual(await field(),originalField);
  await stroke("lower");const lowered=await field();assert.ok(lowered.some((h,i)=>h<originalField[i]));
  await stroke("smooth");assert.notDeepEqual(await field(),lowered);
  await page.getByLabel("Brush mode",{exact:true}).selectOption("flatten");await page.getByLabel("Flatten height",{exact:true}).fill("3");
  await stroke("flatten");const sculptedField=await field();assert.notDeepEqual(sculptedField,originalField);
  await expect.poll(async()=>await viewer.evaluate(element=>(element as HTMLElement&{loaded:boolean}).loaded)).toBe(true);
  await page.screenshot({path:path.join(os.tmpdir(),"emergent-terrain-brush.png")});
  await page.setViewportSize({width:600,height:800});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await page.setViewportSize({width:1600,height:1100});
  await page.getByLabel("Seed",{exact:true}).fill("99");await page.getByLabel("Terrain name",{exact:true}).fill("Saved mountain");
  const saved=page.waitForResponse(r=>r.url().endsWith("/api/terrain/save"),{timeout:120000});await page.getByRole("button",{name:"Save terrain to scene",exact:true}).click();const response=await saved,result=await response.json();assert.equal(response.status(),201,JSON.stringify(result));
  const asset=parse(await readFile(path.join(project,result.path),"utf8"));assert.equal((asset.terrain as Record<string,unknown>).seed,42,"save uses the generated recipe, not edited inputs");assert.equal((asset.terrain as Record<string,unknown>).algorithmVersion,2);assert.ok(typeof asset.terrain_heights==="string");const savedHeights=parse(await readFile(path.join(project,asset.terrain_heights as string),"utf8"));assert.deepEqual(savedHeights.values,Array.from(sculptedField));assert.equal(result.scenePath,scene);assert.equal(await readFile(path.join(project,"assets/scenes/main.scene.toml"),"utf8"),original);
  await page.getByRole("link",{name:"View in scene",exact:false}).click();await expect(page.getByLabel("Scene viewer",{exact:true})).toHaveValue(scene);
  const canvas=page.getByLabel("Live Rust inspection viewport",{exact:true});await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(3);session=await canvas.getAttribute("data-session");
  await expect(page.locator(".physics-inspector").getByLabel("Physics enabled",{exact:true})).toBeChecked({timeout:120000});await expect(page.getByLabel("Collider type",{exact:true})).toHaveValue("terrain");await expect(page.getByLabel("Physics body",{exact:true})).toHaveValue("static");
  const colors=await canvas.evaluate(element=>{const c=element as HTMLCanvasElement;return new Set(c.getContext("2d")!.getImageData(0,0,c.width,c.height).data.filter((_,i)=>i%4===0)).size;});assert.ok(colors>10,"saved terrain must render through native inspection");
  assert.equal(await canvas.getAttribute("data-tick"),"0");
  const disabled=page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement")&&r.request().method()==="PUT",{timeout:120000});await page.locator(".physics-inspector").getByLabel("Physics enabled",{exact:true}).uncheck();const disabledResponse=await disabled;assert.equal(disabledResponse.status(),200,await disabledResponse.text());
  const entities=parse(await readFile(path.join(project,scene),"utf8")).entities as Record<string,unknown>[];assert.equal((entities.find(entity=>entity.id===result.id)!.physics as Record<string,unknown>).enabled,false);
  await expect.poll(async()=>await canvas.getAttribute("data-session"),{timeout:120000}).not.toBe(session);session=await canvas.getAttribute("data-session");
  await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(3);
  await page.screenshot({path:path.join(os.tmpdir(),"emergent-saved-terrain-scene.png")});assert.deepEqual(errors,[]);
  console.log("Terrain brush/save smoke passed: all four brush modes, undo, brush-inclusive snapshot, narrow layout, active destination, displayed recipe, native candidate validation, authored scene/mesh, terrain collider inspector and live disable. Temporary project only.");
 }finally{
  if(session)await page.request.delete(`${base}/api/inspection`,{data:{projectId,id:session}}).catch(()=>{});
  await browser.close();server.kill("SIGTERM");await new Promise<void>(resolve=>{if(server.exitCode!==null)resolve();else server.once("exit",()=>resolve());});await rm(temporary,{recursive:true,force:true});
 }
}
main().catch(error=>{console.error(error);process.exitCode=1;});
