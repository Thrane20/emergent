import assert from "node:assert/strict";
import {chromium,expect} from "@playwright/test";
import {PARTICLE_EMITTER_PRESETS} from "../src/lib/particle-emitter-presets";
import type {ParticleDefinition} from "../src/lib/particles";
async function main(){
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1600,height:1000}}),errors:string[]=[];let saved:ParticleDefinition|undefined;
  page.on("pageerror",e=>errors.push(e.message));page.on("console",m=>{if(m.type()==="error")errors.push(m.text());});
  const png=Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aBmcAAAAASUVORK5CYII=","base64");
  await page.route("**/api/texture-jobs?*",r=>r.fulfill({json:[]}));await page.route("**/api/particle-media?*",r=>r.fulfill({json:[]}));await page.route("**/api/particles/thumbnail?*",r=>r.fulfill({body:png,contentType:"image/png"}));
  await page.route(/\/api\/particles$/,async r=>{saved=r.request().postDataJSON().definition;await r.fulfill({status:201,json:{assetPath:"assets/particles/preset-qa.entity.toml",effectPath:"assets/particles/preset-qa.particle.toml",definition:saved,revision:"qa"}});});
  await page.goto("http://localhost:3001/particles");const emitters=page.getByRole("group",{name:"Effect emitters",exact:true});await expect(emitters.getByRole("button")).toHaveCount(3);await emitters.getByRole("button").nth(1).click();
  await page.getByRole("button",{name:"Choose particle sprite",exact:true}).click();const sprites=page.getByRole("dialog",{name:"Choose particle sprite",exact:true});assert.ok(await sprites.locator(".particle-swatch-grid button").count()>1);await sprites.locator(".particle-swatch-grid button").nth(1).click();await page.getByLabel("Atlas columns",{exact:true}).fill("2");await page.getByLabel("Animation frames",{exact:true}).fill("2");
  await page.getByRole("button",{name:"Save effect",exact:true}).click();await expect.poll(()=>saved?.emitters[1].columns).toBe(2);const baseline=structuredClone(saved!);
  const picker=page.getByRole("button",{name:"Emitter library · 20 types",exact:true}),dialog=page.getByRole("dialog",{name:"Choose emitter type",exact:true});
  await picker.click();await expect(dialog.locator(".particle-emitter-preset-grid button")).toHaveCount(20);await page.keyboard.press("Escape");await expect(dialog).not.toBeVisible();await expect(page.locator(".particle-controls .kind-badge")).toHaveText("SAVED / PRESET");
  for(const preset of PARTICLE_EMITTER_PRESETS){
   await picker.click();await dialog.getByRole("button",{name:`Apply ${preset.name} emitter`,exact:true}).click();await expect(dialog).not.toBeVisible();await expect(page.getByLabel("Emitter name",{exact:true})).toHaveValue(preset.name);await expect(page.locator(".particle-controls .kind-badge")).toHaveText("UNSAVED");await expect(emitters.getByRole("button")).toHaveCount(3);await expect(page.getByLabel("Atlas columns",{exact:true})).toHaveValue("1");
   await page.getByRole("button",{name:"Update effect",exact:true}).click();await expect.poll(()=>saved?.emitters[1].name).toBe(preset.name);assert.equal(saved!.name,baseline.name);assert.equal(saved!.id,baseline.id);assert.deepEqual(saved!.emitters[0],baseline.emitters[0]);assert.deepEqual(saved!.emitters[2],baseline.emitters[2]);assert.equal(saved!.emitters[1].texture,undefined);
  }
  await page.setViewportSize({width:430,height:900});await picker.click();await expect(dialog).toBeInViewport();await dialog.getByRole("button",{name:"Apply Falling leaves emitter",exact:true}).click();await expect(page.getByLabel("Emitter name",{exact:true})).toHaveValue("Falling leaves");assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));assert.deepEqual(errors,[]);
  console.log("PASS all 20 emitter types apply/save to the selected layer only; effect identity and sibling settings stay intact, sprite/atlas reset, dirty state, Escape and mobile selection work. No project writes or paid calls.");
 }finally{await browser.close();}
}
main().catch(e=>{console.error(e);process.exitCode=1;});
