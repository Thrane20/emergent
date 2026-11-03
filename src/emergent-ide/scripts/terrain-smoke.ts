import { smokeProjectRoot } from "./project-fixture";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { chromium, expect } from "@playwright/test";
import { parse } from "smol-toml";
import { DEFAULT_TERRAIN, TERRAIN_PRESETS } from "../src/lib/terrain";

async function main(){
 const base=process.env.EMERGENT_IDE_BASE_URL||"http://localhost:3001",root=smokeProjectRoot();
 const projectId=parse(await readFile(path.join(root,"emergent.toml"),"utf8")).id;
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL||"chrome",headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors:string[]=[];page.on("pageerror",error=>errors.push(error.message));
  async function api(settings:unknown){return page.request.post(`${base}/api/terrain`,{timeout:120000,data:{projectId,settings}});}
  let first:Buffer|undefined;
  for(const preset of TERRAIN_PRESETS){
   const response=await api({...DEFAULT_TERRAIN,preset:preset.id,resolution:32});assert.equal(response.status(),200,response.ok()?"":await response.text());
   const bytes=await response.body();assert.equal(bytes.readUInt32LE(0),0x46546c67);if(preset.id==="hilly")first=bytes;
  }
  assert.deepEqual(await (await api({...DEFAULT_TERRAIN,resolution:32})).body(),first);
  assert.notDeepEqual(await (await api({...DEFAULT_TERRAIN,resolution:32,seed:43})).body(),first);
  assert.equal((await api({...DEFAULT_TERRAIN,resolution:512})).status(),400);
  await page.goto(`${base}/terrain`);await expect(page.getByRole("heading",{name:"Terrain generation",exact:true})).toBeVisible();
  const viewer=page.locator("model-viewer");
  for(const preset of TERRAIN_PRESETS){
   await page.getByRole("button",{name:preset.name,exact:true}).click();
   const old=await viewer.count()?await viewer.evaluate(element=>(element as HTMLElement&{src:string}).src):null;
   const generated=page.waitForResponse(response=>response.url().endsWith("/api/terrain")&&response.request().method()==="POST",{timeout:120000});
   await page.getByRole("button",{name:"Generate terrain",exact:true}).click();const response=await generated;assert.equal(response.status(),200,response.ok()?"":await response.text());
   if(old)await expect(viewer).not.toHaveJSProperty("src",old);
   await page.waitForFunction(()=>(document.querySelector("model-viewer") as HTMLElement&{loaded?:boolean})?.loaded,{timeout:30000});
   await expect(page.locator(".model-viewer-status")).toHaveCount(0);
   await expect(page.getByRole("heading",{name:`${preset.name} · seed 42`,exact:true})).toBeVisible();
   await page.screenshot({path:path.join(os.tmpdir(),`emergent-terrain-${preset.id}.png`)});
  }
  await page.getByRole("button",{name:"Mountainous",exact:true}).click();
  await expect(page.getByLabel("Mountain count",{exact:true})).toHaveValue("3");
  await expect(page.getByRole("region",{name:"Directional light",exact:true})).toBeVisible();
  await page.getByRole("slider",{name:"Sun strength",exact:true}).press("End");
  await page.getByRole("slider",{name:"Sun direction",exact:true}).press("Home");
  await page.getByRole("slider",{name:"Sun elevation",exact:true}).press("ArrowRight");
  await page.getByLabel("Mountain count",{exact:true}).fill("2");
  const roughness=page.getByRole("slider",{name:"Roughness",exact:true});await roughness.press("Home");await roughness.press("ArrowRight");await expect(roughness.locator("..").locator("output")).toHaveText("0.01");
  const previousTerrain=await viewer.evaluate(element=>(element as HTMLElement&{src:string}).src);
  const generated=page.waitForResponse(response=>response.url().endsWith("/api/terrain")&&response.request().method()==="POST",{timeout:120000});
  await page.getByRole("button",{name:"Generate terrain",exact:true}).click();const mountainResponse=await generated;assert.equal(mountainResponse.status(),200);
  const sent=mountainResponse.request().postDataJSON();assert.equal(sent.settings.mountainCount,2);assert.equal(sent.settings.roughness,0.01);assert.equal(sent.settings.sunStrength,1);assert.equal(sent.settings.sunAzimuth,0);assert.equal(sent.settings.sunElevation,36);
  await expect(viewer).not.toHaveJSProperty("src",previousTerrain);
  await page.waitForFunction(()=>(document.querySelector("model-viewer") as HTMLElement&{loaded?:boolean})?.loaded);
  const lightCheck=await viewer.evaluate(async element=>{
   // Inspect the pinned viewer's scene in this test to verify the GLB light renders.
   const key=Object.getOwnPropertySymbols(element).find(symbol=>symbol.description==="scene");if(!key)throw new Error("Viewer scene not found");
   type Light={isDirectionalLight?:boolean;intensity:number};
   const scene=Reflect.get(element,key) as {traverse:(visit:(node:Light)=>void)=>void;queueRender:()=>void};
   const model=element as HTMLElement&{jumpCameraToGoal:()=>void;toDataURL:()=>string};
   const lights:Light[]=[];scene.traverse(node=>{if(node.isDirectionalLight)lights.push(node);});if(lights.length!==1)throw new Error("Terrain needs one directional sun");
   model.jumpCameraToGoal();scene.queueRender();await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));const lit=model.toDataURL();const intensity=lights[0].intensity;
   lights[0].intensity=0;scene.queueRender();await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));const unlit=model.toDataURL();lights[0].intensity=intensity;scene.queueRender();await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));
   const samples:number[]=[];
   for(const source of [lit,unlit]){
    const image=new Image();image.src=source;await image.decode();const canvas=document.createElement("canvas");canvas.width=image.width;canvas.height=image.height;const context=canvas.getContext("2d")!;context.drawImage(image,0,0);const pixels=context.getImageData(0,0,canvas.width,canvas.height).data;let total=0,count=0;
    for(let i=0;i<pixels.length;i+=4)if(pixels[i+3]>128){total+=(pixels[i]+pixels[i+1]+pixels[i+2])/3;count++;}
    samples.push(total/Math.max(1,count));
   }
   return {intensity,changed:lit!==unlit,litMean:samples[0],unlitMean:samples[1]};
  });
  assert.ok(lightCheck.intensity>0);assert.equal(lightCheck.changed,true,"directional sun must change the actual rendered terrain");
  assert.ok(lightCheck.litMean-lightCheck.unlitMean>20,"sunlight must provide clearly visible relief contrast");assert.ok(lightCheck.unlitMean>5,"ambient fill must keep unlit slopes readable");assert.ok(lightCheck.unlitMean<110,"ambient fill must not wash out the directional light");
  console.log("Terrain light comparison:",JSON.stringify(lightCheck));
  await page.screenshot({path:path.join(os.tmpdir(),"emergent-terrain-two-peaks.png")});
  const orbit=()=>viewer.evaluate(element=>(element as HTMLElement&{getCameraOrbit:()=>{theta:number;phi:number;radius:number}}).getCameraOrbit());
  const before=await orbit(),box=(await viewer.boundingBox())!;
  await page.mouse.move(box.x+box.width*.5,box.y+box.height*.5);await page.mouse.down();await page.mouse.move(box.x+box.width*.65,box.y+box.height*.6,{steps:8});await page.mouse.up();
  await expect.poll(async()=>Math.abs((await orbit()).theta-before.theta)).toBeGreaterThan(0.01);
  const radius=(await orbit()).radius;await page.mouse.wheel(0,-150);await expect.poll(async()=>Math.abs((await orbit()).radius-radius)).toBeGreaterThan(0.01);
  await page.getByRole("button",{name:"Reset view",exact:true}).click();await expect.poll(async()=>Math.abs((await orbit()).theta-Math.PI/6)).toBeLessThan(0.01);
  const compared=page.waitForResponse(response=>response.url().endsWith("/api/terrain")&&response.request().method()==="POST",{timeout:120000});
  await page.getByRole("button",{name:"Compare algorithms",exact:true}).click();const comparedResponse=await compared;assert.equal(comparedResponse.status(),200,comparedResponse.ok()?"":await comparedResponse.text());
  const comparison=await comparedResponse.json();assert.equal(comparison.variants.length,3);assert.deepEqual(comparison.variants.map((item:{settings:{algorithmVersion:number}})=>item.settings.algorithmVersion),[1,1,2]);
  for(const item of comparison.variants){assert.ok(item.metrics.generationMs>0);assert.ok(item.metrics.meshMs>0);assert.equal(item.metrics.fieldBytes,128*128*4);assert.equal(Buffer.from(item.glb,"base64").readUInt32LE(0),0x46546c67);}
  await expect(page.locator(".terrain-comparison-card")).toHaveCount(3);
  for(const label of ["Original noise","Diamond-square","Enhanced pipeline"])await expect(page.getByRole("heading",{name:label,exact:true})).toBeVisible();
  await page.waitForFunction(()=>Array.from(document.querySelectorAll("model-viewer")).length===3&&Array.from(document.querySelectorAll("model-viewer")).every(element=>(element as HTMLElement&{loaded?:boolean}).loaded));
  const cameras=await viewer.evaluateAll(elements=>elements.map(element=>({orbit:(element as HTMLElement&{cameraOrbit:string}).cameraOrbit,target:(element as HTMLElement&{cameraTarget:string}).cameraTarget})));assert.ok(cameras[0].orbit.includes("m"));assert.ok(cameras[0].target.includes("m"));assert.deepEqual(cameras[0],cameras[1]);assert.deepEqual(cameras[1],cameras[2]);
  const actualCameras=await viewer.evaluateAll(elements=>elements.map(element=>{const model=element as HTMLElement&{jumpCameraToGoal:()=>void;getCameraOrbit:()=>{theta:number;phi:number;radius:number}};model.jumpCameraToGoal();return model.getCameraOrbit();}));
  for(const camera of actualCameras)assert.ok(Math.abs(camera.radius-Math.hypot(100,38)*1.8)<0.01,`absolute comparison radius was clamped: ${camera.radius}`);
  await expect(page.locator(".model-viewer-status")).toHaveCount(0);await page.screenshot({path:path.join(os.tmpdir(),"emergent-terrain-comparison.png")});
  await page.setViewportSize({width:600,height:800});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await page.setViewportSize({width:1440,height:1000});
  await page.getByRole("combobox",{name:"Base generator",exact:true}).selectOption("diamond-square");
  const single=page.waitForResponse(response=>response.url().endsWith("/api/terrain")&&response.request().method()==="POST",{timeout:120000});await page.getByRole("button",{name:"Generate terrain",exact:true}).click();assert.equal((await single).status(),200);await expect(viewer).toHaveCount(1);await page.waitForFunction(()=>(document.querySelector("model-viewer") as HTMLElement&{loaded?:boolean})?.loaded);
  const good=await viewer.evaluate(element=>(element as HTMLElement&{src:string}).src);await page.route("**/api/terrain",route=>route.fulfill({status:400,json:{error:"Synthetic generation failure"}}));await page.getByRole("button",{name:"Generate terrain",exact:true}).click();await expect(page.locator(".terrain-error")).toContainText("Synthetic generation failure");await expect(viewer).toHaveJSProperty("src",good);await expect(page.getByRole("button",{name:"Generate terrain",exact:true})).toBeEnabled();
  await page.setViewportSize({width:600,height:800});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);assert.deepEqual(errors,[]);
  console.log("Terrain smoke passed: actual Rust generation for all presets and diamond-square, matched three-way comparisons with metrics, deterministic seeds, bounded inputs, GLB loading, orbit/zoom/reset, retained preview on failure, and narrow layout. No project files or provider calls.");
 }finally{await browser.close();}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
