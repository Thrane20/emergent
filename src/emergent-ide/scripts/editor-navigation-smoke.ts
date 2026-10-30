import assert from "node:assert/strict";
import {chromium,expect} from "@playwright/test";

async function main(){
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1600,height:1100}}),base=process.env.EMERGENT_NAV_TEST_URL??"http://localhost:3002",errors:string[]=[];
  page.on("pageerror",error=>errors.push(error.message));
  let balanceState:"ready"|"no-key"|"unavailable"="ready";
  await page.route("**/api/fal-balance*",route=>route.fulfill({json:{state:balanceState,balance:balanceState==="ready"?8.76:balanceState==="no-key"?0:null,currency:"USD",message:balanceState==="unavailable"?"This fal key needs billing:usage:read permission.":"Available fal credit",checkedAt:null}}));
  const png=Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aBmcAAAAASUVORK5CYII=","base64");
  await page.route("**/api/particles/thumbnail?*",route=>route.fulfill({contentType:"image/png",body:png}));
  const scene="assets/scenes/main.scene.toml";
  const routes=[['/images','Image Generator'],['/terrain','Terrain Builder'],['/objects','Object Creator'],['/object-builder','Object Builder'],['/materials','Material Generator'],['/particles','Particle Generator'],['/tweens','Tween Editor'],['/player','Player settings'],['/settings','Settings']];
  for(const [path,label] of routes){
   await page.goto(`${base}${path}?scene=${encodeURIComponent(scene)}`);
   const balance=page.getByRole("button",{name:"fal balance",exact:true});await expect(balance).toHaveCount(1);await expect(balance).toBeVisible();await expect(balance).toContainText("$8.76");assert.ok(await balance.evaluate(el=>Boolean(el.closest("header"))));
   const rail=page.getByRole("navigation",{name:"IDE sections"});await expect(rail).toBeVisible();await expect(page.locator(".editor-rail")).toHaveCount(1);
   await expect(rail.getByRole("link",{name:label,exact:true})).toHaveAttribute("aria-current","page");
   await expect(rail.locator("svg")).toHaveCount(13);
   assert.equal(new URL(await rail.getByRole("link",{name:"Terrain Builder",exact:true}).getAttribute("href")??"",base).searchParams.get("scene"),scene);
   assert.ok(await page.locator("main").evaluate(el=>el.getBoundingClientRect().left>=58));
  }
  const rail=page.getByRole("navigation",{name:"IDE sections"});
  await rail.getByRole("link",{name:"Image Generator",exact:true}).click();await expect(page).toHaveURL(/\/images\?scene=/);
  await page.setViewportSize({width:390,height:844});await expect(rail.getByRole("link",{name:"Particle Generator",exact:true})).toBeInViewport();
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  await page.goto(`${base}/settings`);await expect(rail).toBeVisible();assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  const balance=page.getByRole("button",{name:"fal balance",exact:true});await expect(balance).toBeInViewport();balanceState="no-key";await balance.click();await expect(balance).toContainText("$0");balanceState="unavailable";await balance.click();await expect(balance).toContainText("Unavailable");await expect(balance).toHaveAttribute("title",/billing:usage:read/);balanceState="ready";await page.evaluate(()=>window.dispatchEvent(new Event("fal-settings-changed")));await expect(balance).toContainText("$8.76");
  await page.setViewportSize({width:1600,height:1100});
  for(const [path,field,target] of [['/materials','Material name','Image Generator'],['/particles','Effect name','Tween Editor'],['/object-builder','Object name','Material Generator'],['/tweens','Tween name','Image Generator']]){
   await page.goto(`${base}${path}`);await page.getByLabel(field,{exact:true}).fill('Navigation smoke unsaved');await page.getByLabel(field,{exact:true}).blur();
   let dismissed=false;page.once("dialog",async dialog=>{assert.match(dialog.message(),/unsaved/);dismissed=true;await dialog.dismiss();});
   await rail.getByRole("link",{name:target,exact:true}).click();assert.ok(dismissed);await expect(page).toHaveURL(`${base}${path}`);await expect(page.getByLabel(field,{exact:true})).toHaveValue('Navigation smoke unsaved');
   page.once("dialog",dialog=>dialog.accept());await rail.getByRole("link",{name:target,exact:true}).click();await expect(page).not.toHaveURL(`${base}${path}`);
  }
  await page.goto(`${base}/?scene=${encodeURIComponent(scene)}&view=source`);await expect(page.locator(".editor-rail")).toHaveCount(1);await expect(rail.getByRole("button",{name:"Source editor",exact:true})).toHaveAttribute("aria-pressed","true");
  await expect(page.getByRole("button",{name:"fal balance",exact:true})).toContainText("$8.76");await page.screenshot({path:"/tmp/emergent-fal-balance-header.png"});await page.setViewportSize({width:390,height:844});await expect(page.getByRole("button",{name:"fal balance",exact:true})).toBeInViewport({ratio:1});assert.ok(await page.locator("header.topbar").evaluate(el=>Array.from(el.children).every(child=>{const r=child.getBoundingClientRect();return r.width===0||(r.left>=0&&r.right<=innerWidth);})));await page.screenshot({path:"/tmp/emergent-fal-balance-mobile.png"});
  assert.deepEqual(errors,[]);console.log("Editor rail smoke passed: all editor/settings pages, icons and active destinations, scene context, cross-editor navigation, mobile layout, draft guards, and main Source entry. No saves or paid calls.");
 }finally{await browser.close();}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
