import assert from "node:assert/strict";
import {chromium,expect} from "@playwright/test";
import type {ParticleDefinition} from "../src/lib/particles";

async function main(){
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1600,height:1000}}),errors:string[]=[];let saved:ParticleDefinition|undefined;
  page.on("pageerror",e=>errors.push(e.message));page.on("console",m=>{if(m.type()==="error")errors.push(m.text());});
  const png=Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aBmcAAAAASUVORK5CYII=","base64");
  await page.route("**/api/particles/thumbnail?*",r=>r.fulfill({contentType:"image/png",body:png}));
  await page.route(/\/api\/particles$/,async r=>{saved=r.request().postDataJSON().definition;await r.fulfill({status:201,json:{assetPath:"assets/particles/qa.entity.toml",effectPath:"assets/particles/qa.particle.toml",definition:saved,revision:"qa"}});});
  await page.goto("http://localhost:3001/particles");
  await expect(page.getByRole("region",{name:"Particle sprite library",exact:true})).toBeVisible();
  const birth="Birth color / opacity";
  await page.getByLabel(`${birth} colour`,{exact:true}).fill("#123456");
  await expect(page.getByLabel(`${birth} Red`,{exact:true})).toHaveValue("18");await expect(page.getByLabel(`${birth} Green`,{exact:true})).toHaveValue("52");await expect(page.getByLabel(`${birth} Blue`,{exact:true})).toHaveValue("86");
  await page.getByLabel(`${birth} Opacity`,{exact:true}).fill("0.01");await expect(page.locator(".particle-color input[type=range]")).toHaveCount(0);
  await page.getByRole("button",{name:"Choose particle sprite",exact:true}).click();const dialog=page.getByRole("dialog",{name:"Choose particle sprite",exact:true});await expect(dialog).toBeVisible();await page.keyboard.press("Escape");await expect(dialog).not.toBeVisible();
  await page.getByRole("button",{name:"Choose particle sprite",exact:true}).click();
  const choices=dialog.locator(".particle-swatch-grid button");assert.ok(await choices.count()>1,"Use a project containing library images for this smoke");const selectedName=await choices.nth(1).innerText();await choices.nth(1).click();await expect(dialog).not.toBeVisible();await expect(page.getByRole("button",{name:"Choose particle sprite",exact:true})).toContainText(selectedName);
  await page.getByLabel("Atlas columns",{exact:true}).fill("2");await page.getByLabel("Animation frames",{exact:true}).fill("2");
  await page.getByRole("button",{name:"Choose particle sprite",exact:true}).click();await dialog.locator('button[aria-pressed="true"]').click();await expect(page.getByLabel("Atlas columns",{exact:true})).toHaveValue("2");
  for(const blend of ["alpha","additive","screen","multiply"]){await page.getByLabel("Particle blending",{exact:true}).selectOption(blend);await page.getByRole("button",{name:saved?"Update effect":"Save effect",exact:true}).click();await expect.poll(()=>saved?.emitters[0].blend).toBe(blend);await page.waitForTimeout(150);}
  const emitter=saved!.emitters[0];assert.deepEqual(emitter.color_start,[18/255,52/255,86/255,.01]);assert.ok(emitter.texture);assert.equal(emitter.columns,2);
  await page.getByLabel(`${birth} colour`,{exact:true}).scrollIntoViewIfNeeded();await page.screenshot({path:"/tmp/particle-appearance-controls.png"});
  await page.setViewportSize({width:430,height:900});await page.getByRole("button",{name:"Choose particle sprite",exact:true}).click();await expect(dialog).toBeInViewport();assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));await page.keyboard.press("Escape");assert.deepEqual(errors,[]);
  console.log("PASS RGB picker/compact RGBA values, four blend modes, sprite dialog selection/Escape/mobile, retained flipbook layout, save payload; zero paid submissions.");
 }finally{await browser.close();}
}
main().catch(e=>{console.error(e);process.exitCode=1;});
