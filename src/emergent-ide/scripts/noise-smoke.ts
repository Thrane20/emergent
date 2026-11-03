import { smokeProjectRoot } from "./project-fixture";
import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {readFile,writeFile,unlink} from "node:fs/promises";
import path from "node:path";
import {chromium,expect} from "@playwright/test";
import {parse} from "smol-toml";
async function main(){
 const base=process.env.EMERGENT_IDE_BASE_URL||"http://localhost:3001",root=smokeProjectRoot();
 const projectId=parse(await readFile(path.join(root,"emergent.toml"),"utf8")).id as string,name=`Noise smoke ${randomUUID().slice(0,8)}`,asset=`assets/filters/${name.toLowerCase().replaceAll(" ","-")}.noise.toml`;
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL||"chrome",headless:true});let debugPage:import("@playwright/test").Page|undefined;
 try{
  const page=await browser.newPage({viewport:{width:1600,height:1100}}),errors:string[]=[];debugPage=page;page.on("pageerror",error=>errors.push(error.message));await page.goto(base);
  await page.getByRole("button",{name:"Add asset",exact:false}).click();const dialog=page.getByRole("dialog",{name:"Add a project asset"});
  await dialog.locator("summary").filter({hasText:"Filters"}).click();await dialog.getByRole("button",{name:"Noise function",exact:false}).click();await dialog.getByRole("textbox",{name:"Name",exact:true}).fill(name);
  assert.equal(await dialog.getByText("Include in selected scene",{exact:true}).count(),0);await dialog.getByRole("button",{name:"Create asset",exact:true}).click();
  await expect(page.getByRole("heading",{name:"Noise filter",exact:true})).toBeVisible();console.log("Noise asset created");
  const initial=await readFile(path.join(root,asset),"utf8");const canvas=page.getByRole("img",{name:"Grayscale noise field",exact:true});
  async function generate(){console.log("Generating field");const done=page.waitForResponse(r=>r.url().endsWith("/api/filters/noise")&&r.request().method()==="POST",{timeout:120000});void done.catch(()=>{});await page.getByRole("button",{name:"Generate preview",exact:true}).click();const response=await done;assert.equal(response.status(),200,response.ok()?"":await response.text());await expect(canvas).toBeVisible();return canvas.evaluate(element=>{const c=element as HTMLCanvasElement;return c.toDataURL();});}
  const first=await generate();assert.equal(await readFile(path.join(root,asset),"utf8"),initial);
  assert.equal(await generate(),first,"same inputs must reproduce the same field");
  const grayscale=await canvas.evaluate(element=>{const c=element as HTMLCanvasElement,data=c.getContext("2d")!.getImageData(0,0,c.width,c.height).data;let min=255,max=0;for(let i=0;i<data.length;i+=4){if(data[i]!==data[i+1]||data[i]!==data[i+2]||data[i+3]!==255)return false;min=Math.min(min,data[i]);max=Math.max(max,data[i]);}return max>min;});assert.equal(grayscale,true);
  await page.getByLabel("Noise seed",{exact:true}).fill("42");const changed=await generate();assert.notEqual(changed,first);assert.equal((parse(await readFile(path.join(root,asset),"utf8")).noise as {seed:number}).seed,42);
  await page.getByLabel("Noise algorithm",{exact:true}).selectOption("open-simplex");assert.notEqual(await generate(),changed);
  await page.getByLabel("Noise algorithm",{exact:true}).selectOption("worley");await generate();
  await page.getByLabel("Noise dimensions",{exact:true}).selectOption("3");const volume=await generate();await expect(canvas).toHaveAttribute("data-depth","32");
  await page.getByLabel("Noise Z slice",{exact:true}).fill("7");await expect(canvas).toHaveAttribute("data-slice","7");assert.notEqual(await canvas.evaluate(c=>(c as HTMLCanvasElement).toDataURL()),volume,"volume slices must contain different noise");
  const stored=parse(await readFile(path.join(root,asset),"utf8")).noise as {seed:number;dimensions:number;algorithm:string};assert.deepEqual([stored.seed,stored.dimensions,stored.algorithm],[42,3,"worley"]);
  await page.reload();await page.getByPlaceholder("Find…").fill(path.basename(asset));await page.getByRole("button",{name:path.basename(asset),exact:true}).click();await expect(page.getByLabel("Noise seed",{exact:true})).toHaveValue("42");await expect(page.getByLabel("Noise dimensions",{exact:true})).toHaveValue("3");
  const last=await generate();await page.getByLabel("Noise octaves",{exact:true}).fill("0");await expect(page.locator(".noise-error")).toContainText("octaves");assert.equal(await canvas.evaluate(c=>(c as HTMLCanvasElement).toDataURL()),last);
  await page.getByLabel("Noise octaves",{exact:true}).fill("4");await page.getByLabel("Noise seed",{exact:true}).fill("99");const external=await readFile(path.join(root,asset),"utf8")+"\n# externally updated\n";await writeFile(path.join(root,asset),external);
  await expect(page.locator(".noise-error")).toContainText("changed externally");assert.equal(await readFile(path.join(root,asset),"utf8"),external);
  const bad=await page.request.post(`${base}/api/filters/noise`,{data:{projectId,path:"../outside.noise.toml",settings:stored}});assert.equal(bad.status(),400);
  await page.screenshot({path:"/tmp/emergent-noise-volume.png"});
  page.once("dialog",dialog=>dialog.accept());page.once("dialog",dialog=>void dialog.accept());await page.getByRole("button",{name:"TOML",exact:true}).click();
  const editor=page.getByLabel("TOML source",{exact:true});await expect(editor).toHaveValue(external);
  await editor.fill(external.replace("frequency = 1","frequency = -1"));await page.getByRole("button",{name:"Save TOML",exact:true}).click();await expect(page.locator(".toml-error")).toContainText("Frequency",{timeout:120000});assert.equal(await readFile(path.join(root,asset),"utf8"),external);
  await editor.fill(external+"\n# raw noise editor save\n");const raw=page.waitForResponse(r=>r.url().endsWith("/api/project-toml")&&r.request().method()==="PUT",{timeout:120000});await page.getByRole("button",{name:"Save TOML",exact:true}).click();const rawResult=await raw;assert.equal(rawResult.status(),200,await rawResult.text());await expect(page.locator(".toml-message")).toContainText("Generate a new preview");
  await page.getByRole("button",{name:"Preview",exact:true}).click();await expect(page.getByLabel("Noise seed",{exact:true})).toHaveValue("42");
  await page.getByLabel("Noise dimensions",{exact:true}).selectOption("2");await page.getByLabel("Noise algorithm",{exact:true}).selectOption("perlin");await generate();
  await page.screenshot({path:"/tmp/emergent-noise-filter.png"});assert.deepEqual(errors,[]);console.log("Noise filter smoke passed: modal category, real Rust deterministic/seeded algorithms, grayscale, full 3D volume slices, draft-only generation, persistence and rejected invalid/stale edits.");
 }catch(error){if(debugPage){console.log("Noise errors:",await debugPage.locator(".noise-error").allTextContents());await debugPage.screenshot({path:"/tmp/emergent-noise-failure.png"}).catch(()=>{});}throw error;}finally{await browser.close();await unlink(path.join(root,asset)).catch(error=>{if(error.code!=="ENOENT")throw error;});}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
