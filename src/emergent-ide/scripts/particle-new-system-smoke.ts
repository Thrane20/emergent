import assert from "node:assert/strict";
import {chromium,expect} from "@playwright/test";
import type {ParticleDefinition} from "../src/lib/particles";

async function main(){
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1600,height:1100}}),errors:string[]=[];
  page.on("pageerror",error=>errors.push(error.message));
  const png=Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aBmcAAAAASUVORK5CYII=","base64");
  await page.route("**/api/particles/thumbnail?*",route=>route.fulfill({contentType:"image/png",body:png}));
  let failSave=true,saves=0,loads=0,saved:{assetPath:string;effectPath:string;definition:ParticleDefinition;revision:string}|undefined;
  await page.route(/\/api\/particles(?:\?.*)?$/,async route=>{
   if(route.request().method()==="POST"){
    saves++;const body=route.request().postDataJSON();assert.equal(body.assetPath,undefined,"New drafts must not overwrite the previously saved effect");
    if(failSave){await route.fulfill({status:409,json:{error:"Fixture save conflict"}});return;}
    saved={assetPath:"assets/particles/new-system-smoke.entity.toml",effectPath:"assets/particles/new-system-smoke.particle.toml",definition:body.definition,revision:"fixture"};
    await route.fulfill({status:201,json:saved});
   }else if(saved&&new URL(route.request().url()).searchParams.get("asset")===saved.assetPath){loads++;await route.fulfill({json:saved});}
   else await route.continue();
  });
  await page.goto(`${process.env.EMERGENT_PARTICLE_TEST_URL??"http://localhost:3002"}/particles`);
  const create=page.getByRole("button",{name:"New blank particle system",exact:true}),name=page.getByLabel("Effect name",{exact:true});
  await expect(create).toBeVisible();assert.equal(await create.evaluate(el=>el.closest("article")===el.closest('[role="list"]')?.firstElementChild),true);
  assert.equal(await create.evaluate(el=>getComputedStyle(el.closest("article")!).position),"sticky");
  await create.click();await expect(name).toHaveValue("Untitled particle system");
  await expect(page.getByLabel("Particles per second",{exact:true})).toHaveValue("0");
  await expect(page.getByRole("dialog",{name:"Save current particle system?"})).not.toBeVisible();
  await name.fill("Unsaved QA draft");await create.click();
  const dialog=page.getByRole("dialog",{name:"Save current particle system?",exact:true});
  await expect(dialog).toBeVisible();await dialog.getByRole("button",{name:"Keep editing",exact:true}).click();await expect(name).toHaveValue("Unsaved QA draft");assert.equal(saves,0);
  await create.click();await page.keyboard.press("Escape");await expect(dialog).not.toBeVisible();await expect(name).toHaveValue("Unsaved QA draft");
  await create.click();await dialog.getByRole("button",{name:"Save & create",exact:true}).click();
  await expect(dialog.getByRole("alert")).toHaveText("Fixture save conflict");await expect(name).toHaveValue("Unsaved QA draft");await expect(dialog).toBeVisible();
  failSave=false;await dialog.getByRole("button",{name:"Save & create",exact:true}).click();
  await expect(dialog).not.toBeVisible();await expect(name).toHaveValue("Untitled particle system");
  assert.equal(saved?.definition.name,"Unsaved QA draft");assert.equal(saves,2);
  await expect(page.getByRole("button",{name:"Edit Unsaved QA draft",exact:true})).toBeVisible();
  await name.fill("");await create.click();await expect(dialog.getByRole("button",{name:"Save & create",exact:true})).toBeDisabled();
  await dialog.getByRole("button",{name:"Discard & create",exact:true}).click();await expect(name).toHaveValue("Untitled particle system");assert.equal(saves,2);
  await name.fill("Keep this until confirmed");await page.getByRole("button",{name:"Edit Unsaved QA draft",exact:true}).click();
  const discard=page.getByRole("dialog",{name:"Discard unsaved changes?",exact:true});await expect(discard).toBeVisible();assert.equal(loads,0);
  await discard.getByRole("button",{name:"Discard & load",exact:true}).click();await expect(name).toHaveValue("Unsaved QA draft");assert.equal(loads,1);
  await create.click();await expect(name).toHaveValue("Untitled particle system");await expect(dialog).not.toBeVisible();
  await page.getByRole("list",{name:"Saved particle effects"}).evaluate(el=>el.scrollLeft=el.scrollWidth);await expect(create).toBeInViewport();
  assert.deepEqual(errors,[]);console.log("Particle new-system smoke passed: pinned tile, blank recipe, clean creation, cancel/Escape, failed-save preservation, save-and-create, invalid-save guard, discard, and existing load confirmation. Saves mocked; no project files changed.");
 }finally{await browser.close();}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
