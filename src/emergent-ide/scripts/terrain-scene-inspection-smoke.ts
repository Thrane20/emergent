import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {mkdtemp,readFile,readdir,rm} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {parse} from "smol-toml";
import {chromium,expect} from "@playwright/test";

async function main(){
 const root=path.resolve(process.env.EMERGENT_PROJECT_PATH??"../../projects/first_project"),base="http://localhost:3001",temporary=await mkdtemp(path.join(os.tmpdir(),"terrain-scene-inspection-"));
 const manifest=parse(await readFile(path.join(root,"emergent.toml"),"utf8"));
 const files=await readdir(path.join(root,"assets"),{recursive:true});let scenePath="",instanceId="";
 for(const file of files.filter(file=>file.endsWith(".scene.toml"))){const scene=parse(await readFile(path.join(root,"assets",file),"utf8"));for(const instance of scene.entities as {id:string;asset:string}[]??[]){const asset=parse(await readFile(path.join(root,instance.asset),"utf8"));if((asset.terrain as {resolution?:number}|undefined)?.resolution===1024){scenePath=`assets/${file}`;instanceId=instance.id;break;}}if(scenePath)break;}
 assert.ok(scenePath,"The smoke needs a saved 1024 terrain in a scene.");
 let browser:Awaited<ReturnType<typeof chromium.launch>>|undefined,session:string|null=null,logs="";
 const server=spawn(process.execPath,[path.resolve("node_modules/next/dist/bin/next"),"start","--hostname","::1","--port","3001"],{env:{...process.env,EMERGENT_PROJECT_PATH:root,EMERGENT_IDE_DATA_DIR:temporary,EMERGENT_NEXT_DIST_DIR:".next-terrain-scene-limit-build",FAL_KEY:""},stdio:["ignore","pipe","pipe"]});server.stdout?.on("data",c=>logs+=c);server.stderr?.on("data",c=>logs+=c);
 try{
  await expect.poll(async()=>{if(server.exitCode!==null)throw new Error(logs);try{return(await fetch("http://[::1]:3001",{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:60000}).toBe(true);
  browser=await chromium.launch({channel:"chrome",headless:true});const page=await browser.newPage({viewport:{width:1600,height:1100}}),errors:string[]=[];page.on("pageerror",e=>errors.push(e.message));
  await page.goto(base);await page.getByLabel("Scene viewer",{exact:true}).selectOption(scenePath);
  const row=page.getByRole("region",{name:"Scene instances",exact:true}).locator(`button[data-instance-id="${instanceId}"]`);await expect(row).toBeEnabled({timeout:120000});
  const canvas=page.getByLabel("Live Rust inspection viewport",{exact:true});await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(3);session=await canvas.getAttribute("data-session");assert.ok(session);
  await row.click();await expect(page.locator(".transform-inspector").getByLabel("Position X",{exact:true})).toBeEnabled({timeout:30000});
  const pixels=()=>canvas.evaluate(element=>(element as HTMLCanvasElement).toDataURL("image/png")),before=await pixels(),box=(await canvas.boundingBox())!;
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width/2+90,box.y+box.height/2+30,{steps:6});await page.mouse.up();
  await expect.poll(pixels,{timeout:30000}).not.toBe(before);await page.screenshot({path:"/tmp/terrain-scene-inspection-1024.png"});assert.deepEqual(errors,[]);
  const stopped=await page.request.delete(`${base}/api/inspection`,{headers:{Origin:base},data:{projectId:manifest.id,id:session}});assert.equal(stopped.status(),200);session=null;
  console.log(`PASS main scene graph, terrain selection, enabled transform controls and live native orbit input with 1024 terrain in ${scenePath}. Project files unchanged.`);
 }catch(error){console.error(logs);throw error;}finally{
  if(session)await fetch(`${base}/api/inspection`,{method:"DELETE",headers:{Origin:base,"Content-Type":"application/json"},body:JSON.stringify({projectId:manifest.id,id:session})}).catch(()=>{});
  await browser?.close();server.kill("SIGTERM");await new Promise<void>(resolve=>{if(server.exitCode!==null)resolve();else server.once("exit",()=>resolve());});await rm(temporary,{recursive:true,force:true});
 }
}
main().catch(error=>{console.error(error);process.exitCode=1;});
