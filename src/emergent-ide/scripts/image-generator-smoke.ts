import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {mkdir,mkdtemp,readFile,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {chromium,expect} from "@playwright/test";
import {TextureJobs} from "../src/lib/texture-jobs";
import {readProjectSnapshot} from "../src/lib/project";

async function main(){
 const root=path.resolve("../.."),temporary=await mkdtemp(path.join(os.tmpdir(),"emergent-image-library-"));
 const projectRoot=path.join(temporary,"game"),data=path.join(temporary,"data"),port=process.env.EMERGENT_IMAGE_TEST_PORT??"3001",base=`http://localhost:${port}`;
 const previous={project:process.env.EMERGENT_PROJECT_PATH,data:process.env.EMERGENT_IDE_DATA_DIR,target:process.env.CARGO_TARGET_DIR};
 process.env.EMERGENT_PROJECT_PATH=projectRoot;process.env.EMERGENT_IDE_DATA_DIR=data;process.env.CARGO_TARGET_DIR=path.join(root,"target");
 let server:ReturnType<typeof spawn>|undefined,browser:Awaited<ReturnType<typeof chromium.launch>>|undefined;
 try{
  await mkdir(path.join(projectRoot,"assets/images"),{recursive:true});await mkdir(path.join(projectRoot,"src"));
  await writeFile(path.join(projectRoot,"emergent.toml"),'schema_version=1\nid="image-library-test"\nname="Image Library Test"\nentry_scene="assets/main.scene.toml"\nasset_roots=["assets"]\n');
  await writeFile(path.join(projectRoot,"Cargo.toml"),`[package]\nname="image-library-test"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nemergent={path=${JSON.stringify(root)}}\n`);
  const project=await readProjectSnapshot();let image=Buffer.alloc(0),submissions=0;
  const jobs=new TextureJobs(data,async()=>({submit:async()=>{submissions++;return "mock-image-request";},status:async()=>"completed",result:async()=>[image]}));
  let logs="";
  server=spawn(process.execPath,[path.resolve("node_modules/next/dist/bin/next"),"start","--hostname","::1","--port",port],{env:{...process.env,EMERGENT_NEXT_DIST_DIR:process.env.EMERGENT_NEXT_DIST_DIR??".next-images-build",FAL_KEY:"mock-key-no-network"},stdio:["ignore","pipe","pipe"]});
  server.stdout?.on("data",chunk=>logs+=String(chunk));server.stderr?.on("data",chunk=>logs+=String(chunk));
  await expect.poll(async()=>{if(server?.exitCode!==null)throw new Error(logs);if(!logs.includes("Ready"))return false;try{return (await fetch(`${base}/images`,{signal:AbortSignal.timeout(2000)})).ok;}catch{return false;}},{timeout:30000}).toBe(true);
  browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});
  const page=await browser.newPage({viewport:{width:1600,height:1100}}),errors:string[]=[];page.on("pageerror",error=>errors.push(error.message));
  await page.goto(`${base}/images`);
  image=Buffer.from(await page.evaluate(()=>{const canvas=document.createElement("canvas");canvas.width=64;canvas.height=64;const context=canvas.getContext("2d")!;context.fillStyle="#739984";context.fillRect(0,0,64,64);context.fillStyle="#d8cea1";context.beginPath();context.arc(45,17,9,0,Math.PI*2);context.fill();return canvas.toDataURL().split(",")[1];}),"base64");
  await writeFile(path.join(projectRoot,"assets/images/Existing forest.png"),image);await page.reload();
  const library=page.getByRole("region",{name:"Project image library"}),tile=library.getByRole("button",{name:/^Existing forest Image/});
  await expect(tile).toBeVisible();await tile.click();
  const modal=page.getByRole("dialog",{name:"Existing forest",exact:true});await expect(modal).toBeVisible();
  await expect.poll(()=>modal.locator("img").evaluate(element=>(element as HTMLImageElement).naturalWidth)).toBe(64);
  await page.keyboard.press("Escape");await expect(modal).not.toBeVisible();await expect(tile).toBeFocused();
  let failResponse=true,jobId="";const attempts:{id:string;input:unknown}[]=[];
  await page.route("**/api/texture-jobs",async route=>{
   if(route.request().method()!=="POST"){await route.continue();return;}
   const body=route.request().postDataJSON();attempts.push({id:body.id,input:body.input});jobId=body.id;
   const queued=await jobs.create(project,body.id,body.input);
   if(failResponse){failResponse=false;await route.abort("failed");return;}
   await jobs.refresh(project,body.id);await route.fulfill({status:201,json:queued});
  });
  await page.getByLabel("Image name",{exact:true}).fill("Moonlit forest");await page.getByLabel("Image prompt",{exact:true}).fill("A moonlit forest with a winding river");
  await page.getByRole("button",{name:"Generate image",exact:true}).click();await expect(page.locator(".image-feedback").getByRole("alert")).toBeVisible();
  await page.getByRole("button",{name:"Generate image",exact:true}).click();
  const candidate=page.getByRole("button",{name:"Preview generated Moonlit forest",exact:true});await expect(candidate).toBeVisible({timeout:120000});
  assert.equal(attempts.length,2);assert.deepEqual(attempts[0],attempts[1]);assert.equal(submissions,1);
  assert.equal((await jobs.get(project,jobId)).submittedPrompt,"A moonlit forest with a winding river");
  await expect(library.getByRole("button",{name:/^Moonlit forest Image/})).toHaveCount(0);
  await candidate.click();await expect(page.getByRole("dialog",{name:"Moonlit forest",exact:true})).toBeVisible();await page.getByRole("button",{name:"Close image preview"}).click();
  await candidate.dragTo(library.locator(".image-drop-hint"));
  await expect(library.getByRole("button",{name:/^Moonlit forest Image/})).toBeVisible({timeout:120000});
  await expect(page.getByRole("button",{name:"In library ✓",exact:true})).toBeDisabled();
  const saved=await jobs.get(project,jobId),result=saved.outputs[0];assert.ok(result.savedPath);
  assert.deepEqual(await readFile(path.join(projectRoot,result.savedPath)),image);
  const retry=await page.request.post(`${base}/api/texture-jobs/${jobId}`,{data:{projectId:project.id,action:"save",resultId:result.id,name:"Duplicate name"}});
  assert.equal(retry.status(),200);assert.equal((await retry.json()).outputs[0].savedPath,result.savedPath);
  const listing=await page.request.get(`${base}/api/textures?projectId=${project.id}`);assert.equal((await listing.json()).length,2);
  assert.equal((await page.request.get(`${base}/api/textures?projectId=other-project`)).status(),400);
  assert.equal((await page.request.post(`${base}/api/textures?projectId=${project.id}`,{headers:{Origin:"https://other.example"},data:"invalid"})).status(),400);
  const invalid=await page.request.post(`${base}/api/textures?projectId=${project.id}&name=bad.png`,{data:Buffer.from("invalid pixels")});assert.equal(invalid.status(),400);
  await page.getByLabel("Import library image").setInputFiles({name:"Imported moon.png",mimeType:"image/png",buffer:image});
  await expect(library.getByRole("button",{name:/^Imported moon Image/})).toBeVisible({timeout:120000});
  await page.reload();await expect(library.locator(".image-library-tile")).toHaveCount(3);await expect(candidate).toBeVisible();
  const shot=process.env.EMERGENT_IMAGE_SCREENSHOT??path.join(os.tmpdir(),"emergent-image-generator.png");await page.screenshot({path:shot,fullPage:true});
  await page.setViewportSize({width:390,height:844});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  await library.getByRole("button",{name:/^Imported moon Image/}).click();await expect(page.getByRole("dialog",{name:"Imported moon",exact:true})).toBeVisible();await page.keyboard.press("Escape");
  assert.deepEqual(errors,[]);console.log(`Image Generator smoke passed: existing assets, modal focus/Escape, plain prompt, lost-response idempotency, preview, drag/drop save, duplicate save, import, access checks, reload, and mobile layout. Mocked fal; real native validation and project writes. Screenshot: ${shot}`);
 }finally{
  await browser?.close();
  if(server&&server.exitCode===null){const exited=new Promise<void>(resolve=>server!.once("exit",()=>resolve()));server.kill("SIGTERM");await exited;}
  for(const [key,value] of [["EMERGENT_PROJECT_PATH",previous.project],["EMERGENT_IDE_DATA_DIR",previous.data],["CARGO_TARGET_DIR",previous.target]]){if(value===undefined)delete process.env[key!];else process.env[key!]=value;}
  await rm(temporary,{recursive:true,force:true});
 }
}
main().catch(error=>{console.error(error);process.exitCode=1;});
