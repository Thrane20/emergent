import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {readFile,writeFile,unlink} from "node:fs/promises";
import path from "node:path";
import {chromium,expect,request} from "@playwright/test";
import {parse} from "smol-toml";
async function main(){
 const base=process.env.EMERGENT_IDE_BASE_URL||"http://localhost:3001",root=path.resolve(process.env.EMERGENT_PROJECT_PATH||"../../projects/first_project"),projectId="emergent.first-project";
 const scene=`assets/scenes/shadow-smoke-${randomUUID().slice(0,8)}.scene.toml`;
 const casterAsset="assets/entities/shadow-cube.entity.toml",receiverAsset="assets/entities/shadow-floor.entity.toml";
 const originals=await Promise.all([casterAsset,receiverAsset].map(file=>readFile(path.join(root,file),"utf8")));
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL||"chrome",headless:true});const api=await request.newContext({baseURL:base,timeout:120000});let session:string|null=null;
 try{
  const sourceScene=process.env.EMERGENT_SHADOW_SCENE||"assets/scenes/shadows.scene.toml";
  await writeFile(path.join(root,scene),(await readFile(path.join(root,sourceScene),"utf8")).replace(/^id\s*=.*$/m,'id = "shadow-smoke"'),{flag:"wx"});
  const page=await browser.newPage({viewport:{width:1600,height:1100}});const errors:string[]=[];page.on("pageerror",e=>errors.push(e.message));
  await page.goto(base);await page.getByLabel("Scene viewer",{exact:true}).selectOption(scene);
  const canvas=page.getByLabel("Live Rust inspection viewport",{exact:true}),inspector=page.locator(".material-inspector");
  const pixels=()=>canvas.evaluate(element=>(element as HTMLCanvasElement).toDataURL());
  await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(3);session=await canvas.getAttribute("data-session");
  async function select(id:string){await page.getByRole("region",{name:"Scene instances"}).getByRole("button",{name:`Select instance ${id}`,exact:true}).click();await inspector.scrollIntoViewIfNeeded();await expect(inspector.getByLabel("Object material",{exact:true})).toBeEnabled();const frame=Number(await canvas.getAttribute("data-frame"));await expect.poll(async()=>Number(await canvas.getAttribute("data-frame"))).toBeGreaterThan(frame+2);}
  async function edit(id:string,patch:Record<string,boolean>){
   const response=await api.get(`/api/scenes/placement?projectId=${projectId}&path=${encodeURIComponent(scene)}`),placement=await response.json();assert.equal(response.status(),200);
   const instance=placement.instances.find((item:{id:string})=>item.id===id);
   const saved=await api.put("/api/scenes/placement",{data:{projectId,path:scene,id,revision:placement.revision,action:"material",material:{...instance.material,...patch}}});assert.equal(saved.status(),200,await saved.text());
   await api.delete("/api/inspection",{data:{projectId,id:session}});
   await page.goto(`${base}/?scene=${encodeURIComponent(scene)}&instance=${id}`);await expect.poll(async()=>canvas.getAttribute("data-session"),{timeout:120000}).not.toBe(session);session=await canvas.getAttribute("data-session");
   await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(3);assert.equal(await canvas.getAttribute("data-tick"),"0");
  }
  await select("caster");
  const castBefore=await pixels();await edit("caster",{cast_shadows:false});assert.notEqual(await pixels(),castBefore,"cast flag must change the native scene");
  await edit("caster",{cast_shadows:true});
  await select("receiver");
  const receiveBefore=await pixels();await edit("receiver",{receive_shadows:false});assert.notEqual(await pixels(),receiveBefore,"receive flag must change the native scene");
  const entries=parse(await readFile(path.join(root,scene),"utf8")).entities as Record<string,unknown>[];
  assert.equal((entries[0].material as Record<string,unknown>).cast_shadows,true);assert.equal((entries[1].material as Record<string,unknown>).receive_shadows,false);
  assert.deepEqual(await Promise.all([casterAsset,receiverAsset].map(file=>readFile(path.join(root,file),"utf8"))),originals);assert.deepEqual(errors,[]);
  await page.screenshot({path:"/tmp/emergent-object-shadows.png"});console.log("Material shadow settings save per-instance, change native shadow output and keep inspection paused.");
 }finally{if(session)await api.delete("/api/inspection",{data:{projectId,id:session}}).catch(()=>{});await api.dispose();await browser.close();await unlink(path.join(root,scene));}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
