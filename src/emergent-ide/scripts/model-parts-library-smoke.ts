import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import path from "node:path";
import {chromium,expect} from "@playwright/test";
import {parse} from "smol-toml";
import {parsePrefab} from "../src/lib/prefabs";
import {smokeProjectRoot} from "./project-fixture";

async function main(){
 const root=smokeProjectRoot(),folder="assets/models/box-cut-qa",descriptor=`${folder}/object.asset.json`,prefabPath=`${folder}/parts/model.prefab.toml`,base=process.env.EMERGENT_NAV_TEST_URL??"http://localhost:3001";
 assert.ok(path.basename(root).startsWith("emergent-box-cut-qa-"),"Use a fresh project from model-box-cut-smoke.ts --prepare");
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1600,height:1100}}),errors:string[]=[];page.on("pageerror",e=>errors.push(e.message));
  await page.route("**/api/fal-balance*",route=>route.fulfill({json:{state:"no-key",balance:0,currency:"USD",message:"No key",checkedAt:null}}));
  await page.goto(`${base}/models`);const save=page.getByRole("button",{name:"Save To Object Library",exact:true});await expect(save).toBeEnabled({timeout:20000});
  await page.getByRole("group",{name:"Cut axis",exact:true}).getByRole("button",{name:"X",exact:true}).click();await page.getByRole("button",{name:"Cut part",exact:true}).click();
  async function saveLibrary(){const pending=page.waitForResponse(response=>response.url().endsWith("/api/models/parts")&&response.request().method()==="POST",{timeout:180000});await save.click();const result=await pending;assert.equal(result.status(),201,await result.text());await expect(page.getByText("Saved 2 parts to Object Library.",{exact:true})).toBeVisible();}
  await expect(page.locator(".model-parts-list li")).toHaveCount(2);await saveLibrary();
  const first=parsePrefab(await readFile(path.join(root,prefabPath),"utf8"));assert.equal(first.entities.length,3);assert.equal(new Set(first.entities.map(x=>x.id)).size,3);
  await page.locator('.model-parts-list input:not([type="checkbox"])').first().fill("Library body");await saveLibrary();
  const second=parsePrefab(await readFile(path.join(root,prefabPath),"utf8"));assert.equal(second.id,first.id,"resaving preserves prefab identity");
  await page.getByRole("link",{name:"Open Object Library",exact:false}).click();await expect(page).toHaveURL(/\/object-builder$/);
  const tile=page.getByRole("list",{name:"Saved objects",exact:true}).getByRole("button",{name:"Open Box Cut QA",exact:true});await expect(tile).toHaveCount(1);await page.screenshot({path:"/tmp/emergent-model-parts-library.png",fullPage:true});
  await tile.click();await expect(page).toHaveURL(/\/models\?model=/);await expect(save).toBeEnabled();await expect(page.locator('.model-parts-list input:not([type="checkbox"])').first()).toHaveValue("Library body");
  const partsDoc=parse(await readFile(path.join(root,folder,"parts/parts.toml"),"utf8")) as unknown as {parts:{id:string;name:string;asset:string;parent?:string;pivot:number[]}[]};
  const kept=partsDoc.parts[1],bytes=await readFile(path.join(root,kept.asset.replace(/\.entity\.toml$/,".glb"))),projectId=String(parse(await readFile(path.join(root,"emergent.toml"),"utf8")).id);
  async function persist(id=kept.id){return page.request.post(`${base}/api/models/parts`,{multipart:{projectId,descriptor,parts:JSON.stringify([{id,name:kept.name,parent:null,pivot:kept.pivot}]),[`part:${id}`]:{name:`${id}.glb`,mimeType:"model/gltf-binary",buffer:bytes}},timeout:180000});}
  const trimmed=await persist();assert.equal(trimmed.status(),201,await trimmed.text());const third=parsePrefab(await readFile(path.join(root,prefabPath),"utf8"));assert.equal(third.id,first.id);assert.equal(third.entities.length,2,"owned manifest references do not block removal");
  await page.goto(`${base}/object-builder`);await expect(tile).toHaveCount(1);await tile.click({button:"right"});const dialog=page.getByRole("dialog",{name:"Add to scene",exact:true});await expect(dialog).toBeVisible();await page.getByLabel("Object destination scene",{exact:true}).selectOption("assets/scenes/main.scene.toml");
  const before=parse(await readFile(path.join(root,"assets/scenes/main.scene.toml"),"utf8")) as {entities?:unknown[]};await dialog.getByRole("button",{name:"Add to scene",exact:true}).click();await expect(page.getByText("Saved object added to scene.",{exact:true})).toBeVisible({timeout:180000});
  const after=parse(await readFile(path.join(root,"assets/scenes/main.scene.toml"),"utf8")) as unknown as {entities:{id:string;asset:string;parent?:string}[]};assert.equal(after.entities.length,(before.entities?.length??0)+2);const added=after.entities.slice(-2);assert.ok(added.some(x=>x.asset===kept.asset));assert.equal(added[1].parent,added[0].id);
  const published=await readFile(path.join(root,prefabPath),"utf8");
  const rejected=await persist("33333333-3333-4333-8333-333333333333");assert.equal(rejected.status(),409,await rejected.text());assert.match((await rejected.json()).error,/from scenes before deleting/);assert.equal(await readFile(path.join(root,prefabPath),"utf8"),published);
  assert.equal(parsePrefab(await readFile(path.join(root,prefabPath),"utf8")).entities.length,2);assert.deepEqual(errors,[]);
  console.log("PASS Save To Object Library, hierarchy prefab, stable repeat save, one library tile, correct editor reopening, internal-reference-safe removal, full scene placement and external-reference protection. No paid calls.");
 }finally{await browser.close();}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
