import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {chromium,expect} from "@playwright/test";
import type {TextureJob} from "../src/lib/texture-job-types";
async function main(){
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1600,height:1000}}),jobs=new Map<string,TextureJob>(),errors:string[]=[],deleted:string[]=[];let generated=0,fail=true;
  page.on("pageerror",e=>errors.push(e.message));const png=Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aBmcAAAAASUVORK5CYII=","base64");
  await page.route("**/api/particle-media?*",r=>r.fulfill({json:[]}));await page.route("**/api/texture-jobs/*/artifact?*",r=>r.fulfill({body:png,contentType:"image/png"}));
  await page.route(/\/api\/texture-jobs$/,r=>{generated++;const body=r.request().postDataJSON(),count=generated===1?1:2,date=`2100-01-0${generated}T00:00:00Z`,job:TextureJob={schema_version:1,id:body.id,projectId:body.projectId,input:{...body.input,count},submittedPrompt:body.input.prompt,endpoint:"mock",requestId:"mock",state:"ready",createdAt:date,updatedAt:date,error:null,outputs:Array.from({length:count},(_,index)=>({id:randomUUID(),index,file:`result-${index}.png`,saveName:null,savedPath:count===2&&index===0?"assets/saved-copy.png":null}))};jobs.set(job.id,job);return r.fulfill({json:job});});
  await page.route(/\/api\/texture-jobs\/[^/?]+$/,async r=>{const id=new URL(r.request().url()).pathname.split("/").pop()!,body=r.request().postDataJSON();assert.equal(body.action,"delete");if(fail){fail=false;await r.fulfill({status:500,json:{error:"Fixture deletion failed"}});return;}const job=jobs.get(id)!;deleted.push(body.resultId);job.outputs=job.outputs.filter(o=>o.id!==body.resultId);job.updatedAt="2100-02-01T00:00:00Z";await r.fulfill({json:job});});
  await page.goto("http://localhost:3001/images");const libraryCount=await page.locator(".image-library-tile").count();
  for(const name of ["Older fixture","Delete this batch"]){await page.getByLabel("Image name",{exact:true}).fill(name);await page.getByLabel("Image prompt",{exact:true}).fill("Soft smoke sprite on transparent background");await page.getByRole("button",{name:"Generate image",exact:true}).click();await expect(page.locator(".image-candidate b").first()).toHaveText(name);}
  const buttons=page.getByRole("button",{name:"Delete generated image",exact:true});await expect(buttons).toHaveCount(2);await expect(page.getByRole("button",{name:"In library ✓",exact:true})).toBeDisabled();
  await buttons.first().click();await expect(page.locator(".image-feedback").getByRole("alert")).toHaveText("Fixture deletion failed");await expect(buttons).toHaveCount(2);
  await buttons.first().click();await expect(buttons).toHaveCount(1);assert.equal(await page.locator(".image-library-tile").count(),libraryCount);await expect(page.locator(".image-feedback")).toContainText("saved library copy was kept");
  await page.setViewportSize({width:430,height:900});await page.getByRole("button",{name:"Preview generated Delete this batch",exact:true}).click({button:"right"});const dialog=page.getByRole("dialog",{name:"Sprite options",exact:true});await dialog.getByRole("button",{name:"Delete result",exact:true}).click();await expect(dialog).not.toBeVisible();await expect(page.locator(".image-candidate b").first()).toHaveText("Older fixture");await expect(page.locator(".image-history .object-history-item").filter({hasText:"Delete this batch"})).toHaveCount(0);assert.equal(deleted.length,2);assert.deepEqual(errors,[]);
  console.log("PASS Image Builder delete buttons/context action, failure keeps preview, sibling preservation, saved library tiles remain, empty history removal, next-job selection and mobile. Mocked generations only.");
 }finally{await browser.close();}
}
main().catch(e=>{console.error(e);process.exitCode=1;});
