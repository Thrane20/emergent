import { smokeProjectRoot } from "./project-fixture";
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { readFile, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium, expect, request } from "@playwright/test";
import { assetContents, assetTemplate } from "../src/lib/asset-catalog";

async function main(){
  const base=process.env.EMERGENT_IDE_BASE_URL||"http://localhost:3001";
  const root=smokeProjectRoot();
  const manifest=await readFile(path.join(root,"emergent.toml"),"utf8"),projectId=manifest.match(/^id\s*=\s*"([^"]+)"/m)![1];
  const id=`inspection-smoke-${randomUUID().slice(0,8)}`,scene=`assets/scenes/${id}.scene.toml`,asset=`assets/entities/${id}.entity.toml`;
  const original=assetContents(assetTemplate("object.cube")!,id);
  const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL||"chrome",headless:true});
  const api=await request.newContext({baseURL:base,timeout:120000});let session:string|null=null;
  try{
    await writeFile(path.join(root,asset),original,{flag:"wx"});
    await writeFile(path.join(root,scene),`schema_version=1\nid="${id}"\n[[entities]]\nid="cube"\nasset="${asset}"\n`,{flag:"wx"});
    const page=await browser.newPage({viewport:{width:1600,height:1100}});
    const errors:string[]=[];page.on("pageerror",error=>errors.push(error.message));
    await page.goto(base);await page.getByLabel("Scene viewer",{exact:true}).selectOption(scene);
    
    await expect(page.getByRole("button",{name:/^(Inspect|Restart inspection|Stop inspection)$/})).toHaveCount(0);
    const canvas=page.getByLabel("Live Rust inspection viewport",{exact:true});
    await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(3);
    session=await canvas.getAttribute("data-session");assert.ok(session);
    const worker=Number(await canvas.getAttribute("data-worker"));assert.ok(worker>0);
    assert.equal(await canvas.getAttribute("data-tick"),"0");
    const pixels=()=>canvas.evaluate(element=>(element as HTMLCanvasElement).toDataURL("image/png"));
    const before=await pixels(),first=Number(await canvas.getAttribute("data-frame"));
    await expect.poll(async()=>Number(await canvas.getAttribute("data-frame"))).toBeGreaterThan(first+5);
    assert.ok(await pixels()===before,"gameplay rotation must remain stopped across frames");
    const bounds=(await canvas.boundingBox())!;
    await page.mouse.move(bounds.x+bounds.width/2,bounds.y+bounds.height/2);await page.mouse.down();await page.mouse.move(bounds.x+bounds.width/2+80,bounds.y+bounds.height/2+30,{steps:6});await page.mouse.up();
    await expect.poll(async()=>Number(await canvas.getAttribute("data-frame"))).toBeGreaterThan(first+10);
    assert.ok(await pixels()!==before,"orbit input must change the real Rust frame");
    const orbitImage=await pixels(),panFrame=Number(await canvas.getAttribute("data-frame"));
    await page.keyboard.down("Shift");await page.mouse.down();await page.mouse.move(bounds.x+bounds.width/2+110,bounds.y+bounds.height/2+50,{steps:4});await page.mouse.up();await page.keyboard.up("Shift");
    await expect.poll(async()=>Number(await canvas.getAttribute("data-frame"))).toBeGreaterThan(panFrame+2);
    assert.ok(await pixels()!==orbitImage,"pan input must change the real Rust frame");
    await canvas.dispatchEvent("wheel",{deltaY:150});
    await page.getByRole("region",{name:"Scene instances"}).getByRole("button",{name:"Select instance cube",exact:true}).click();
    const inspector=page.locator(".transform-inspector");await inspector.getByLabel("Position X",{exact:true}).fill("2");
    const beforeEdit=await pixels(),editFrame=Number(await canvas.getAttribute("data-frame"));
    const saved=page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement")&&r.request().method()==="PUT",{timeout:120000});
    const save=await saved;assert.equal(save.status(),200,await save.text());
    await expect(inspector.getByLabel("Position X",{exact:true})).toBeEnabled({timeout:120000});
    await expect.poll(async()=>Number(await canvas.getAttribute("data-frame"))).toBeGreaterThan(editFrame+2);
    assert.ok(await pixels()!==beforeEdit,"saved placement must update the retained renderer");
    assert.equal(await canvas.getAttribute("data-session"),session);assert.equal(Number(await canvas.getAttribute("data-worker")),worker);
    assert.equal(await readFile(path.join(root,asset),"utf8"),original);
    let status=await (await api.get(`/api/inspection?projectId=${projectId}&id=${session}`)).json();
    assert.equal(status.tick,0);assert.equal(status.workerPid,worker);assert.equal(status.diagnostics.split("Inspection constructed once").length-1,1);
    const rejected=await api.put("/api/inspection",{data:{projectId,id:session,command:{op:"transform",id:"missing",translation:[0,0,0],rotation_degrees:[0,0,0],scale:[1,1,1]}}});assert.equal(rejected.status(),400);
    await page.screenshot({path:"/tmp/emergent-persistent-inspection.png"});
    const old=session;await page.getByRole("button",{name:"Load",exact:true}).click();
    await expect(canvas).not.toHaveAttribute("data-session",old,{timeout:120000});
    await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(2);
    session=await canvas.getAttribute("data-session");
    const stale=await api.put("/api/inspection",{data:{projectId,id:old,command:{op:"view",yaw:0,pitch:0,distance:12,target:[0,0,0],projection:"perspective",fieldOfView:45,orthographicSize:12}}});assert.equal(stale.status(),400);
    assert.deepEqual(errors,[]);
    const stopped = page.waitForResponse(response=>response.url().endsWith("/api/inspection")&&response.request().method()==="DELETE");
    await page.getByRole("button",{name:"Source",exact:true}).click();
    assert.equal((await stopped).status(),200);
    await expect(canvas).toHaveCount(0);
    status=await (await api.get(`/api/inspection?projectId=${projectId}&id=${session}`)).json();assert.equal(status.state,"stopped");session=null;
    await page.getByRole("button",{name:"Scene",exact:true}).click();
    await expect.poll(async()=>Number(await canvas.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(2);
    session=await canvas.getAttribute("data-session");assert.ok(session);assert.notEqual(session,old);
    const stoppedAgain=page.waitForResponse(response=>response.url().endsWith("/api/inspection")&&response.request().method()==="DELETE");
    await page.getByRole("button",{name:"Source",exact:true}).click();
    assert.equal((await stoppedAgain).status(),200);session=null;
    await page.close();

    // Record a warm reference-scene baseline; no source edits or provider calls.
    const entry=manifest.match(/^entry_scene\s*=\s*"([^"]+)"/m)![1];
    status=await (await api.post("/api/inspection",{data:{projectId,path:entry}})).json();session=status.id;
    let after=0;const samples:{at:number;renderUs:number;encodeUs:number;bytes:number}[]=[];
    while(samples.length<25){
      const response=await api.get(`/api/inspection/frame?projectId=${projectId}&id=${session}&after=${after}`);if(response.status()===204)continue;
      assert.equal(response.status(),200,response.ok()?"":await response.text());const headers=response.headers();after=Number(headers["x-inspection-frame"]);
      samples.push({at:Date.now(),renderUs:Number(headers["x-inspection-render-us"]),encodeUs:Number(headers["x-inspection-encode-us"]),bytes:(await response.body()).length});
    }
    const warm=samples.slice(5),median=(values:number[])=>values.sort((a,b)=>a-b)[Math.floor(values.length/2)];
    const start=Date.now();const cameraResponse=await api.put("/api/inspection",{data:{projectId,id:session,command:{op:"view",yaw:0.4,pitch:0.3,distance:12,target:[0,0,0],projection:"perspective",fieldOfView:45,orthographicSize:12}}});
    assert.equal(cameraResponse.status(),200,await cameraResponse.text());
    const measurements={profile:"release",resolution:"960x540",pacedHz:30,deliveredFps:(warm.length-1)*1000/(warm.at(-1)!.at-warm[0].at),medianRenderReadbackMs:median(warm.map(s=>s.renderUs))/1000,medianPngEncodeMs:median(warm.map(s=>s.encodeUs))/1000,medianPngBytes:median(warm.map(s=>s.bytes)),commandAckMs:Date.now()-start};
    console.log("Inspection reference measurements:",JSON.stringify(measurements));
    // No frame/status/control traffic during this wait: the inactivity lease must reap the worker.
    await new Promise(resolve=>setTimeout(resolve,36000));
    status=await (await api.get(`/api/inspection?projectId=${projectId}&id=${session}`)).json();assert.equal(status.state,"stopped","idle inspection must expire");session=null;
    console.log("Persistent inspection smoke passed: retained worker, tick zero, orbit/zoom, saved inspector edit, restart/stale rejection, stop and idle expiry.");
  }finally{
    if(session)await api.delete("/api/inspection",{data:{projectId,id:session}}).catch(()=>{});
    await browser.close();await api.dispose();
    for(const file of [scene,asset])await unlink(path.join(root,file)).catch(error=>{if(error.code!=="ENOENT")throw error;});
  }
}
main().catch(error=>{console.error(error);process.exitCode=1;});
