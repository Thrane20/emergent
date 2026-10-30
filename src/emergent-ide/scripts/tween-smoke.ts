import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {mkdir,mkdtemp,readFile,readdir,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {parse} from "smol-toml";
import {chromium,expect} from "@playwright/test";

async function main(){
  const temporary=await mkdtemp(path.join(os.tmpdir(),"emergent-tween-ui-")),project=path.join(temporary,"game"),root=path.resolve("../.."),port=process.env.EMERGENT_TWEEN_TEST_PORT??"3005",base=`http://localhost:${port}`;
  await mkdir(path.join(project,"assets/scenes"),{recursive:true});await mkdir(path.join(project,"src"));
  await writeFile(path.join(project,"emergent.toml"),'id="tween-ui"\nname="Tween demo"\nasset_roots=["assets"]\nentry_scene="assets/scenes/main.scene.toml"\n');
  await writeFile(path.join(project,"assets/scenes/main.scene.toml"),'schema_version=1\nid="main"\nentities=[]\n');
  await writeFile(path.join(project,"Cargo.toml"),`[dependencies]\nemergent={path=${JSON.stringify(root)}}\n`);
  const server=spawn(process.execPath,[path.resolve("node_modules/next/dist/bin/next"),"start","--port",port],{env:{...process.env,EMERGENT_PROJECT_PATH:project,EMERGENT_IDE_DATA_DIR:path.join(temporary,"private")},stdio:["ignore","pipe","pipe"]});
  let log="";server.stdout.on("data",chunk=>{log=(log+chunk).slice(-8000);});server.stderr.on("data",chunk=>{log=(log+chunk).slice(-8000);});
  const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});
  try{
    let ready=false;for(let i=0;i<120;i++){if(server.exitCode!==null)throw new Error(log);try{const response=await fetch(`${base}/tweens`,{signal:AbortSignal.timeout(1000)});if(response.ok){ready=true;break;}}catch{}await new Promise(resolve=>setTimeout(resolve,500));}assert.ok(ready,log);
    const page=await browser.newPage({viewport:{width:1480,height:1180}}),errors:string[]=[];page.on("pageerror",error=>errors.push(error.message));
    await page.goto(`${base}/tweens`);await expect(page.getByRole("heading",{name:"Motion preview",exact:true})).toBeVisible();
    const object=page.getByLabel("Animated preview object",{exact:true});await expect(object).toHaveAttribute("data-value",/./);
    const first=await object.getAttribute("data-value");await expect.poll(()=>object.getAttribute("data-value")).not.toBe(first);
    await page.getByRole("button",{name:"Cubic Out",exact:true}).click();await expect(page.getByRole("button",{name:"Cubic Out",exact:true})).toHaveAttribute("aria-pressed","true");
    await page.getByLabel("Tween duration",{exact:true}).fill("0");await expect(page.locator(".tween-error[role=alert]")).toContainText("Duration");await expect(page.getByRole("button",{name:"Save selected variant",exact:true})).toBeDisabled();
    await page.getByLabel("Tween duration",{exact:true}).fill("1.25");await page.getByRole("button",{name:"Pause",exact:true}).click();
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));const paused=await object.getAttribute("data-value");
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await object.getAttribute("data-value"),paused);
    const slider=page.getByRole("slider",{name:"Scrub tween progress",exact:true});await slider.press("Home");await expect(object).toHaveAttribute("data-value","-3.0000");await slider.press("ArrowRight");await expect(object).not.toHaveAttribute("data-value","-3.0000");
    await page.getByLabel("Tween playback",{exact:true}).selectOption("once");await page.getByRole("button",{name:"Restart",exact:true}).click();await expect(page.getByRole("button",{name:"Play",exact:true})).toBeVisible();await expect(object).toHaveAttribute("data-value","3.0000");
    await page.getByLabel("Tween name",{exact:true}).fill("Scout motion");
    const save=page.waitForResponse(response=>response.url().endsWith("/api/tweens")&&response.request().method()==="POST");await page.getByRole("button",{name:"Save selected variant",exact:true}).click();const response=await save;assert.equal(response.status(),201,await response.text());const asset=await response.json();
    const source=parse(await readFile(path.join(project,asset.path),"utf8"));assert.equal((source.tween as Record<string,unknown>).easing,"cubic-out");assert.equal((source.tween as Record<string,unknown>).duration_seconds,1.25);
    await expect(page.locator(".tween-saved[role=status]")).toContainText("Saved Scout motion");await page.getByRole("link",{name:"Open saved variant",exact:false}).click();
    await expect(page.getByLabel("Tween name",{exact:true})).toHaveValue("Scout motion");await expect(page.getByRole("button",{name:"Cubic Out",exact:true})).toHaveAttribute("aria-pressed","true");await expect(page.getByLabel("Tween duration",{exact:true})).toHaveValue("1.25");
    const again=page.waitForResponse(response=>response.url().endsWith("/api/tweens")&&response.request().method()==="POST");await page.getByRole("button",{name:"Save selected variant",exact:true}).click();assert.equal((await again).status(),201);await expect(page.locator(".tween-saved[role=status]")).toContainText("Saved Scout motion");assert.equal((await readdir(path.join(project,"assets/tweens"))).length,2);
    const rejected=await page.request.post(`${base}/api/tweens`,{headers:{Origin:"https://elsewhere.invalid"},data:{projectId:"tween-ui",name:"Rejected",settings:asset.settings}});assert.equal(rejected.status(),400);assert.equal((await readdir(path.join(project,"assets/tweens"))).length,2);
    await page.screenshot({path:path.join(os.tmpdir(),"emergent-tween-editor.png"),fullPage:true});assert.deepEqual(errors,[]);
    console.log("Tween UI smoke passed: moving cube, curve selection, pause/scrub, once endpoint, invalid duration, native-validated save/reopen and duplicate display names. Temporary project only.");
  }finally{await browser.close();server.kill("SIGTERM");await new Promise<void>(resolve=>{if(server.exitCode!==null)resolve();else server.once("exit",()=>resolve());});await rm(temporary,{recursive:true,force:true});}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
