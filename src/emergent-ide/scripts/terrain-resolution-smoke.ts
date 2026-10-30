import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {mkdir,mkdtemp,readFile,readdir,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {DEFAULT_TERRAIN} from "../src/lib/terrain";
import {parse,stringify} from "smol-toml";
import {chromium,expect} from "@playwright/test";
async function main(){
 const engine=path.resolve("../.."),temporary=await mkdtemp(path.join(os.tmpdir(),"terrain-library-smoke-")),root=path.join(temporary,"project"),base="http://localhost:3001",scenePath="assets/main.scene.toml",otherScene="assets/other.scene.toml";
 let server:ReturnType<typeof spawn>|undefined,browser:Awaited<ReturnType<typeof chromium.launch>>|undefined,logs="";
 try{
  await mkdir(path.join(root,"assets"),{recursive:true});await mkdir(path.join(root,"src/generated"),{recursive:true});await writeFile(path.join(root,"src/main.rs"),"fn main() {}\n");
  await writeFile(path.join(root,"emergent.toml"),'schema_version=1\nid="terrain-library-smoke"\nname="Terrain Library Smoke"\nentry_scene="assets/main.scene.toml"\nasset_roots=["assets"]\ngenerated_code="src/generated"\n[[outputs]]\nid="host"\nos="macos"\narchitecture="aarch64"\nrust_target="aarch64-apple-darwin"\nprofile="development"\n');
  await writeFile(path.join(root,"Cargo.toml"),`[package]\nname="terrain-library-smoke"\nversion="0.1.0"\nedition="2021"\n[workspace]\n[dependencies]\nemergent={path=${JSON.stringify(engine)},features=["physics"]}\n`);
  const scene='# keep scene\nschema_version=1\nid="main"\nname="Main"\nentities=[]\n',other='schema_version=1\nid="other"\nname="Other"\nentities=[]\n';await writeFile(path.join(root,scenePath),scene);await writeFile(path.join(root,otherScene),other);
  await writeFile(path.join(root,"assets/grass.material.toml"),stringify({schema_version:1,id:"grass",name:"Grass",material:{albedo:[.1,.6,.2]}}));
  await writeFile(path.join(root,"assets/legacy.entity.toml"),stringify({schema_version:1,id:"legacy",name:"Legacy terrain",kind:"terrain",terrain:Object.fromEntries(Object.entries({...DEFAULT_TERRAIN,resolution:32}).filter(([key])=>!key.startsWith("sun")))}));
  server=spawn(process.execPath,[path.resolve("node_modules/next/dist/bin/next"),"start","--hostname","::1","--port","3001"],{env:{...process.env,EMERGENT_PROJECT_PATH:root,EMERGENT_IDE_DATA_DIR:path.join(temporary,"data"),EMERGENT_NEXT_DIST_DIR:".next-terrain-resolution-build",FAL_KEY:"",CARGO_TARGET_DIR:path.join(engine,"target")},stdio:["ignore","pipe","pipe"]});server.stdout?.on("data",c=>logs+=c);server.stderr?.on("data",c=>logs+=c);
  await expect.poll(async()=>{if(server?.exitCode!==null)throw new Error(logs);try{return (await fetch("http://[::1]:3001/terrain",{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:60000}).toBe(true);
  browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});const page=await browser.newPage({viewport:{width:1500,height:1000}}),errors:string[]=[];page.on("pageerror",e=>errors.push(e.message));await page.goto(`${base}/terrain`);
  const cdp=await page.context().newCDPSession(page);await cdp.send("Network.enable",{maxTotalBufferSize:256*1024*1024,maxResourceBufferSize:128*1024*1024});
  const library=page.getByRole("region",{name:"Terrain library",exact:true});
  for(const resolution of [512,1024]){
   await page.getByLabel("Terrain resolution",{exact:true}).selectOption(String(resolution));
   await page.getByLabel("Terrain name",{exact:true}).fill(`High resolution ${resolution}`);
   await expect(page.getByRole("button",{name:"Compare algorithms",exact:true})).toBeDisabled();
   const generation=page.waitForResponse(response=>response.url().endsWith("/api/terrain")&&response.request().method()==="POST",{timeout:120000});
   await page.getByRole("button",{name:"Generate terrain",exact:true}).click();const result=await generation;assert.equal(result.status(),200,result.ok()?"":await result.text());
   await expect(page.getByLabel("Paint terrain heightmap",{exact:true})).toHaveAttribute("width",String(resolution),{timeout:120000});
   await expect.poll(()=>page.locator(".terrain-viewer model-viewer").evaluate(el=>(el as HTMLElement&{loaded:boolean}).loaded),{timeout:120000}).toBe(true);

  }
  const heightmap=page.getByLabel("Paint terrain heightmap",{exact:true});await heightmap.focus();await heightmap.press("Space");
  await page.getByRole("button",{name:"Enable material painting",exact:true}).click();
  const saved=page.waitForResponse(response=>response.url().endsWith("/api/terrain/library")&&response.request().method()==="POST",{timeout:240000});
  await library.getByRole("button",{name:"Save current terrain",exact:true}).click();const response=await saved;assert.equal(response.status(),201);await expect(library).toContainText("saved to the terrain library",{timeout:240000});
  const assetPath="assets/terrains/"+(await readdir(path.join(root,"assets/terrains"))).find(file=>file.endsWith(".entity.toml"))!;
  const asset=parse(await readFile(path.join(root,assetPath),"utf8")),heights=await readFile(path.join(root,String(asset.terrain_heights)),"utf8"),paint=await readFile(path.join(root,String(asset.terrain_paint)),"utf8");
  assert.ok(heights.length>2*1024*1024);assert.ok(paint.length>2*1024*1024);assert.equal((parse(heights).values as unknown[]).length,1024**2);assert.equal((parse(paint).weights as unknown[]).length,1024**2*4);
  await page.reload();await library.getByRole("button",{name:"Load terrain High resolution 1024",exact:true}).click();await expect(heightmap).toBeVisible({timeout:120000});
  await expect.poll(()=>page.locator(".terrain-viewer model-viewer").evaluate(el=>(el as HTMLElement&{loaded:boolean}).loaded),{timeout:120000}).toBe(true);
  await library.getByRole("button",{name:"Add terrain High resolution 1024 to scene",exact:true}).click();await expect(library).toContainText("added to Main",{timeout:240000});
  await page.screenshot({path:"/tmp/terrain-resolution-1024.png"});assert.deepEqual(errors,[]);
  console.log(`PASS 512/1024 Rust generation and browser rendering; 1024 sculpt, painted save, native validation, reload and scene placement. Height sidecar ${heights.length} bytes, paint ${paint.length} bytes.`);
 }catch(error){await browser?.contexts()[0]?.pages()[0]?.screenshot({path:"/tmp/terrain-resolution-failure.png"}).catch(()=>{});console.error(logs);throw error;}finally{await browser?.close();if(server){server.kill("SIGTERM");await new Promise<void>(resolve=>{if(server!.exitCode!==null)resolve();else server!.once("exit",()=>resolve());});}await rm(temporary,{recursive:true,force:true});}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
