import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {readFile,writeFile,unlink} from "node:fs/promises";
import path from "node:path";
import {chromium,expect,request} from "@playwright/test";
import {parse} from "smol-toml";

async function main(){
 const base=process.env.EMERGENT_IDE_BASE_URL||"http://localhost:3001",root=path.resolve(process.env.EMERGENT_PROJECT_PATH||"../../projects/first_project");
 const scene=`assets/scenes/material-smoke-${randomUUID().slice(0,8)}.scene.toml`,asset="assets/entities/material-sphere.entity.toml",noise="assets/filters/material-grain.noise.toml",projectId="emergent.first-project";
 const original=await readFile(path.join(root,asset),"utf8");
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL||"chrome",headless:true});
 const api=await request.newContext({baseURL:base,timeout:120000});let session:string|null=null,libraryPath:string|null=null;
 try {
  await writeFile(path.join(root,scene),`schema_version=1\nid="material-smoke"\n[[entities]]\nid="surface"\nasset="${asset}"\n[[entities]]\nid="other"\nasset="${asset}"\n[entities.transform]\ntranslation=[3,0,0]\n[[entities]]\nid="light"\nasset="assets/entities/key-light.entity.toml"\n`,{flag:"wx"});
  const page=await browser.newPage({viewport:{width:1600,height:1100}});const errors:string[]=[];page.on("pageerror",e=>errors.push(e.message));
  await page.goto(base);await page.getByLabel("Scene viewer",{exact:true}).selectOption(scene);
  const canvas=page.getByLabel("Live Rust inspection viewport",{exact:true});
  await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(3);
  session=await canvas.getAttribute("data-session");
  const before=await canvas.evaluate(element=>(element as HTMLCanvasElement).toDataURL());
  await page.getByRole("region",{name:"Scene instances"}).getByRole("button",{name:"Select instance surface",exact:true}).click();
  const inspector=page.locator(".material-inspector");await inspector.scrollIntoViewIfNeeded();
  const created=await api.post("/api/materials",{data:{projectId,name:"Material smoke",material:{shading:"smooth",albedo:[0.8,0.1,0.05],specular:[1,1,1],emission:[0.05,0.1,0.8],emission_intensity:1,bump_map:{asset:noise,strength:1}}}});
  assert.equal(created.status(),201,await created.text());libraryPath=(await created.json()).path;
  await api.delete("/api/inspection",{data:{projectId,id:session}});
  await page.goto(`${base}/materials?asset=${encodeURIComponent(libraryPath!)}`);
  await expect(page.getByLabel("Material shading",{exact:true})).toHaveValue("smooth");
  await page.getByLabel("Material emission intensity",{exact:true}).fill("2");
  await page.getByLabel("Material bump strength",{exact:true}).fill("0.25");
  const sharedSave=page.waitForResponse(r=>r.url().endsWith("/api/materials")&&r.request().method()==="POST",{timeout:120000});
  await page.getByRole("button",{name:"Save shared material",exact:true}).click();const shared=await sharedSave;assert.equal(shared.status(),201,await shared.text());
  await page.goto(`${base}/?scene=${encodeURIComponent(scene)}&instance=surface`);
  await expect(inspector.getByLabel("Object material",{exact:true})).toBeEnabled({timeout:120000});
  session=await canvas.getAttribute("data-session");
  const saved=page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement")&&r.request().method()==="PUT",{timeout:120000});
  await inspector.getByLabel("Object material",{exact:true}).selectOption(libraryPath!);const result=await saved;assert.equal(result.status(),200,await result.text());
  await expect.poll(async()=>canvas.getAttribute("data-session"),{timeout:120000}).not.toBe(session);session=await canvas.getAttribute("data-session");
  await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(3);
  assert.equal(await canvas.getAttribute("data-tick"),"0");
  assert.notEqual(await canvas.evaluate(element=>(element as HTMLCanvasElement).toDataURL()),before);
  const entries=parse(await readFile(path.join(root,scene),"utf8")).entities as Record<string,unknown>[];
  assert.equal((entries[0].material as {asset:string}).asset,libraryPath);
  const material=parse(await readFile(path.join(root,libraryPath!),"utf8")).material as Record<string,unknown>;
  assert.equal(material.emission_intensity,2);assert.equal(material.bump_strength,0.25);assert.equal((material.bump_map as {asset:string}).asset,noise);assert.equal(entries[1].material,undefined);
  assert.equal(await readFile(path.join(root,asset),"utf8"),original);assert.deepEqual(errors,[]);
  await page.screenshot({path:"/tmp/emergent-material-editor.png"});
  console.log("Material Generator channels, shared save, library application, native render change and tick-zero restart verified.");
 }finally{if(session)await api.delete("/api/inspection",{data:{projectId,id:session}}).catch(()=>{});await api.dispose();await browser.close();await unlink(path.join(root,scene));if(libraryPath)await unlink(path.join(root,libraryPath));}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
