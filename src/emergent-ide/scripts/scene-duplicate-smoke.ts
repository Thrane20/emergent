import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {setDefaultResultOrder} from "node:dns";
import {once} from "node:events";
import {mkdtemp,readFile,rm,writeFile} from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import {setTimeout} from "node:timers/promises";
import {chromium,expect} from "@playwright/test";
import {parse} from "smol-toml";
import {prepareTestProject} from "./project-fixture";

async function main(){
 setDefaultResultOrder("ipv4first");
 const port=3001,base=`http://localhost:${port}`;
 const probe=net.createServer();probe.listen(port,"127.0.0.1");await once(probe,"listening");await new Promise<void>(resolve=>probe.close(()=>resolve()));
 const directory=await mkdtemp(path.join(os.tmpdir(),"emergent-duplicate-")),root=path.join(directory,"game"),scene="assets/scenes/main.scene.toml";
 let server:ReturnType<typeof spawn>|undefined,browser:Awaited<ReturnType<typeof chromium.launch>>|undefined,output="";
 try{
  await prepareTestProject(root,"emergent-duplicate-smoke");
  await writeFile(path.join(root,scene),'# copy smoke\nschema_version=1\nid="main"\n[[entities]]\nid="cube"\nname="Cube"\nasset="assets/entities/cube.entity.toml"\n[entities.transform]\ntranslation=[2,3,4]\n[[entities]]\nid="child"\nname="Child"\nparent="cube"\nasset="assets/entities/cube.entity.toml"\n');
  const projectId=parse(await readFile(path.join(root,"emergent.toml"),"utf8")).id;
  const assetBefore=await readFile(path.join(root,"assets/entities/cube.entity.toml"),"utf8");
  server=spawn(process.execPath,["node_modules/next/dist/bin/next","start","--hostname","127.0.0.1","--port",String(port)],{env:{...process.env,EMERGENT_PROJECT_PATH:root,EMERGENT_IDE_DATA_DIR:path.join(directory,"data"),EMERGENT_NEXT_DIST_DIR:process.env.EMERGENT_NEXT_DIST_DIR??".next-duplicate-build"},stdio:["ignore","pipe","pipe"]});
  server.stdout?.on("data",bytes=>{output=(output+bytes).slice(-12000);});server.stderr?.on("data",bytes=>{output=(output+bytes).slice(-12000);});
  let ready=false;for(let i=0;i<120;i++){if(server.exitCode!==null)throw new Error(output);if(await fetch(base,{signal:AbortSignal.timeout(1000)}).then(r=>r.ok,()=>false)){ready=true;break;}await setTimeout(250);}assert.ok(ready,output);
  browser=await chromium.launch({channel:"chrome",headless:true,args:["--host-resolver-rules=MAP localhost 127.0.0.1"]});
  const page=await browser.newPage(),errors:string[]=[];page.setDefaultTimeout(120000);page.on("pageerror",error=>errors.push(error.message));
  await page.route("**/api/fal-balance*",route=>route.fulfill({json:{state:"no-key",balance:0,currency:"USD",message:"Smoke",checkedAt:null}}));
  await page.goto(`${base}/?scene=${scene}`);
  const tree=page.getByRole("tree",{name:"Scene graph"});await expect(tree.getByRole("treeitem")).toHaveCount(2);
  const initial=await (await page.request.get(`${base}/api/scenes/placement?projectId=${projectId}&path=${scene}`)).json();
  const copied=page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement")&&r.request().method()==="PUT");
  await tree.getByRole("button",{name:"Duplicate Cube",exact:true}).click();const first=await copied;assert.ok(first.ok(),await first.text());
  const copy=await first.json();await expect(tree.getByRole("treeitem")).toHaveCount(4);await expect(tree.getByRole("treeitem",{name:copy.id,exact:true})).toHaveAttribute("aria-selected","true");
  assert.equal(copy.placement.instances.find((item:{id:string})=>item.id===copy.id).name,"Cube (1)");
  const second=page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement")&&r.request().method()==="PUT");
  const selectedCopy=tree.getByRole("button",{name:`Select instance ${copy.id}`,exact:true});await expect(selectedCopy).toBeEnabled();await selectedCopy.press("d");assert.ok((await second).ok());await expect(tree.getByRole("treeitem")).toHaveCount(6);
  await expect(tree.getByRole("button",{name:"Duplicate Cube (2)",exact:true})).toBeVisible();
  const before=await readFile(path.join(root,scene),"utf8");
  await page.getByPlaceholder("Find…",{exact:true}).fill("d");await page.getByPlaceholder("Find…",{exact:true}).press("d");await page.getByPlaceholder("Find…",{exact:true}).fill("");
  assert.equal(await readFile(path.join(root,scene),"utf8"),before);
  const stale=await page.request.put(`${base}/api/scenes/placement`,{data:{projectId,path:scene,id:"cube",revision:initial.revision,action:"duplicate"}});assert.equal(stale.status(),400);assert.match((await stale.json()).error,/changed externally/);
  assert.equal(await readFile(path.join(root,scene),"utf8"),before);assert.equal(await readFile(path.join(root,"assets/entities/cube.entity.toml"),"utf8"),assetBefore);assert.deepEqual(errors,[]);
  console.log("Object duplication smoke passed: button, D, selection, numbered copy of a copy, children, typing guard, native validation, stale revision rejection and unchanged source assets.");
 }catch(error){console.error(output);throw error;}
 finally{await browser?.close();if(server&&server.exitCode===null){const stopped=once(server,"exit");server.kill("SIGTERM");await stopped;}await rm(directory,{recursive:true,force:true});}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
