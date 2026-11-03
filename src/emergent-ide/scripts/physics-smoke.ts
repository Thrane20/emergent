import { smokeProjectRoot } from "./project-fixture";
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { readFile, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium, expect, request } from "@playwright/test";
import { parse } from "smol-toml";

async function main(){
  const base=process.env.EMERGENT_IDE_BASE_URL||"http://localhost:3001";
  const root=smokeProjectRoot();
  const projectId=String(parse(await readFile(path.join(root,"emergent.toml"),"utf8")).id);
  const id=`physics-smoke-${randomUUID().slice(0,8)}`,scene=`assets/scenes/${id}.scene.toml`;
  const source=`schema_version=1\nid="${id}"\n[[entities]]\nid="falling-cube"\nasset="assets/entities/physics-cube.entity.toml"\n[[entities]]\nid="ground"\nasset="assets/entities/physics-ground.entity.toml"\n`;
  const original=await readFile(path.join(root,"assets/entities/physics-cube.entity.toml"),"utf8");
  const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL||"chrome",headless:true});
  const api=await request.newContext({baseURL:base,timeout:120000});
  let session:string|null=null;
  try{
    await writeFile(path.join(root,scene),source,{flag:"wx"});
    const page=await browser.newPage({viewport:{width:1600,height:1100}});
    const errors:string[]=[];page.on("pageerror",e=>errors.push(e.message));
    await page.goto(base);await page.getByLabel("Scene viewer",{exact:true}).selectOption(scene);
    
    const canvas=page.getByLabel("Live Rust inspection viewport",{exact:true});
    await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(3);
    session=await canvas.getAttribute("data-session");assert.ok(session);
    const worker=await canvas.getAttribute("data-worker");
    assert.equal(await canvas.getAttribute("data-tick"),"0");
    await page.getByRole("region",{name:"Scene instances"}).getByRole("button",{name:"Select instance falling-cube",exact:true}).click();
    const inspector=page.locator(".physics-inspector");
    await expect(inspector.getByLabel("Physics enabled",{exact:true})).toBeChecked();
    await expect(inspector.getByLabel("Physics body",{exact:true})).toHaveValue("dynamic");
    const live=page.waitForResponse(r=>r.url().endsWith("/api/inspection")&&r.request().method()==="PUT"&&r.request().postDataJSON()?.command?.op==="physics",{timeout:120000});
    await inspector.getByLabel("Gravity scale",{exact:true}).fill("0.5");
    const saved=page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement")&&r.request().method()==="PUT",{timeout:120000});
    const response=await saved;assert.equal(response.status(),200,await response.text());
    const updated=await live;assert.equal(updated.status(),200,await updated.text());
    await expect(canvas).toHaveAttribute("data-session",session);
    await expect(canvas).toHaveAttribute("data-worker",worker!);

    await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(3);
    const parsed=parse(await readFile(path.join(root,scene),"utf8"));
    assert.equal(((parsed.entities as Record<string,unknown>[])[0].physics as Record<string,unknown>).gravity_scale,0.5);
    assert.equal(await readFile(path.join(root,"assets/entities/physics-cube.entity.toml"),"utf8"),original);
    assert.equal(await canvas.getAttribute("data-tick"),"0");
    await page.screenshot({path:"/tmp/emergent-physics-inspector.png"});
    const colours=await canvas.evaluate(element=>{const c=element as HTMLCanvasElement;return new Set(c.getContext("2d")!.getImageData(0,0,c.width,c.height).data.filter((_,i)=>i%4===0)).size;});
    assert.ok(colours>2,"native frame must contain rendered scene pixels");
    assert.deepEqual(errors,[]);
    console.log("Physics inspector save/restart, native tick-zero stream and unchanged asset verified.");
  }finally{
    if(session)await api.delete("/api/inspection",{data:{projectId,id:session}}).catch(()=>{});
    await api.dispose();await browser.close();await unlink(path.join(root,scene));
  }
}
main().catch(error=>{console.error(error);process.exitCode=1;});
