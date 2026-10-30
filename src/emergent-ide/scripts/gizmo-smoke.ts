import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {readFile,writeFile,unlink} from "node:fs/promises";
import path from "node:path";
import {chromium,expect} from "@playwright/test";
import {parse} from "smol-toml";
import {assetContents,assetTemplate} from "../src/lib/asset-catalog";
import {imageRect} from "../src/lib/viewport-coordinates";

async function main(){
 const base=process.env.EMERGENT_IDE_BASE_URL||"http://localhost:3000",root=path.resolve(process.env.EMERGENT_PROJECT_PATH||"../../projects/first_project");
 const projectId=parse(await readFile(path.join(root,"emergent.toml"),"utf8")).id as string,id=`gizmo-smoke-${randomUUID().slice(0,8)}`,asset=`assets/entities/${id}.entity.toml`,scene=`assets/scenes/${id}.scene.toml`;
 const lamp=`assets/entities/${id}-lamp.entity.toml`;
 const text=assetContents(assetTemplate("object.cube")!,id);
 const sceneText=`schema_version=1\nid="${id}"\n[[entities]]\nid="near"\nasset="${asset}"\n[[entities]]\nid="far"\nasset="${asset}"\n[entities.transform]\ntranslation=[-3,-2.142857,-3.857143]\n[[entities]]\nid="lamp"\nasset="${lamp}"\n[entities.transform]\ntranslation=[3,2,0]\n`;
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL||"chrome",headless:true});let session:string|null=null;let debugPage:import("@playwright/test").Page|undefined;
 try{
  await writeFile(path.join(root,asset),text,{flag:"wx"});await writeFile(path.join(root,lamp),assetContents(assetTemplate("light.point")!,id+"-lamp"),{flag:"wx"});await writeFile(path.join(root,scene),sceneText,{flag:"wx"});
  const page=await browser.newPage({viewport:{width:1600,height:1100}}),errors:string[]=[];debugPage=page;page.on("pageerror",error=>{errors.push(error.message);console.log("Browser error",error.message);});
  page.on("response",response=>{if(response.url().includes("/api/inspection")&&response.status()>=400)void response.json().then(data=>{if(!response.ok())console.log("Command rejected",data);});});await page.goto(base);console.log("IDE opened");
  await page.getByLabel("Scene viewer",{exact:true}).selectOption(scene);
  const canvas=page.getByLabel("Live Rust inspection viewport",{exact:true});
  await expect.poll(async()=>{const message=await page.locator(".inspection-error").count()?await page.locator(".inspection-error").textContent():"";if(message)throw new Error(message);return Number(await canvas.getAttribute("data-frame"));},{timeout:30000}).toBeGreaterThan(3);console.log("Native frames ready");session=await canvas.getAttribute("data-session");const worker=await canvas.getAttribute("data-worker");
  const box=(await canvas.boundingBox())!,rect=imageRect(box.width,box.height);const screen=(p:number[])=>({x:box.x+rect.left+p[0]*rect.width,y:box.y+rect.top+p[1]*rect.height});
  const click=screen([0.5,0.5]);await page.mouse.click(click.x,click.y);
  const selected=page.getByRole("treeitem",{name:"near",exact:true});await expect(selected).toHaveAttribute("aria-selected","true");console.log("Nearest object picked");
  assert.equal(await page.getByRole("treeitem",{name:"far",exact:true}).getAttribute("aria-selected"),"false","nearest geometry must occlude far instance");
  async function points(tool:string,axis:string){await expect(page.locator(".inspection-gizmos")).toHaveCSS("opacity","1",{timeout:15000});const handle=page.getByRole("button",{name:`${tool} ${axis} handle`,exact:true});await expect(handle).toBeVisible({timeout:15000});return (await handle.getAttribute("points"))!.split(" ").map(p=>p.split(",").map(Number).map((n,i)=>n/[960,540][i]));}
  async function dragHandle(tool:string,axis:string,cancel:false|"escape"|"pointer"=false,conflict=false,reject=false){
   console.log("Dragging",tool,axis,cancel?"cancel":"save");
   await page.getByRole("group",{name:"Viewport tool"}).getByRole("button",{name:tool,exact:true}).click();
   const p=await points(tool.toLowerCase(),axis);const start=tool==="Rotate"?p[Math.floor(p.length/8)]:p[0].map((n,i)=>n+(p[1][i]-n)*0.8);
   const end=tool==="Rotate"?p[Math.floor(p.length/4)]:start.map((n,i)=>n+(p[1][i]-p[0][i])*0.35);
   const a=screen(start),b=screen(end),before=await readFile(path.join(root,scene),"utf8");
   const completed=page.waitForResponse(r=>r.url().endsWith("/api/inspection")&&r.request().method()==="PUT"&&r.request().postDataJSON()?.command?.op===((cancel||conflict||reject)?"cancel":"finish"),{timeout:120000});
   void completed.catch(()=>{});
   await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y,{steps:8});
   await expect(page.getByRole("button",{name:"Load",exact:true})).toBeDisabled();
   assert.equal(await readFile(path.join(root,scene),"utf8"),before,"pointer moves must not write files");
   const stale=await page.request.put(`${base}/api/inspection`,{data:{projectId,id:session,command:{op:"cancel",token:"another-gesture"}}});assert.equal(stale.status(),400,"another gesture cannot cancel this drag");
   if(reject){await page.mouse.up();await expect(page.getByRole("button",{name:"Load",exact:true})).toBeEnabled({timeout:120000});await expect(page.locator(".inspection-error")).toContainText("without shear");assert.equal(await readFile(path.join(root,scene),"utf8"),before);}
   else if(conflict){await writeFile(path.join(root,scene),before+"\n# external edit during drag\n");const saved=page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement")&&r.request().method()==="PUT",{timeout:120000});await page.mouse.up();assert.equal((await saved).status(),400);await expect(page.getByRole("button",{name:"Load",exact:true})).toBeEnabled({timeout:120000});await expect(page.locator(".inspection-error")).toContainText("changed externally");assert.equal(await readFile(path.join(root,scene),"utf8"),before+"\n# external edit during drag\n");}
   else if(cancel){if(cancel==="pointer")await page.getByRole("button",{name:`${tool.toLowerCase()} ${axis} handle`,exact:true}).dispatchEvent("pointercancel",{pointerId:1});else await page.keyboard.press("Escape");await page.mouse.up();await expect(page.getByRole("button",{name:"Load",exact:true})).toBeEnabled({timeout:120000});assert.equal(await readFile(path.join(root,scene),"utf8"),before);}
   else{const saved=page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement")&&r.request().method()==="PUT",{timeout:120000});await page.mouse.up();const response=await saved;assert.equal(response.status(),200,await response.text());await expect(page.getByRole("button",{name:"Load",exact:true})).toBeEnabled({timeout:120000});assert.notEqual(await readFile(path.join(root,scene),"utf8"),before);}
   const completedResponse=await completed;assert.equal(completedResponse.status(),200,await completedResponse.text());const state=await completedResponse.json();
   const expected=(parse(await readFile(path.join(root,scene),"utf8")).entities as {transform?:Record<string,number[]>}[])[0].transform!;
   for(const key of ["translation","rotation_degrees","scale"])for(let axis=0;axis<3;axis++)assert.ok(Math.abs(state.result.transform[key][axis]-expected[key][axis])<0.001,"retained transform must match saved/restored source");
  }
  await dragHandle("Translate","X");
  let entries=parse(await readFile(path.join(root,scene),"utf8")).entities as {id:string;transform?:{translation:number[];rotation_degrees:number[];scale:number[]}}[];assert.ok(entries[0].transform!.translation[0]>0);assert.equal(entries[1].transform!.translation[0],-3);
  await dragHandle("Rotate","Z");entries=parse(await readFile(path.join(root,scene),"utf8")).entities as typeof entries;assert.ok(Math.abs(entries[0].transform!.rotation_degrees[2])>5);
  await dragHandle("Scale","X");entries=parse(await readFile(path.join(root,scene),"utf8")).entities as typeof entries;assert.ok(entries[0].transform!.scale[0]>1);
  await dragHandle("Translate","Y","escape");
  await dragHandle("Scale","X","pointer");
  await page.getByRole("group",{name:"Viewport tool"}).getByRole("button",{name:"Translate",exact:true}).click();
  const marker=page.locator(".inspection-gizmos text").filter({hasText:"lamp"});await expect(marker).toBeVisible();
  const position=(await marker.evaluate(element=>element.parentElement!.getAttribute("transform")))!.match(/translate\(([^ ]+) ([^)]+)\)/)!;
  const lampPoint=screen([Number(position[1])/960,Number(position[2])/540]);await page.mouse.click(lampPoint.x,lampPoint.y);await expect(page.getByRole("treeitem",{name:"lamp",exact:true})).toHaveAttribute("aria-selected","true");
  await page.getByRole("button",{name:"Select instance near",exact:true}).click();await points("translate","X");
  // A clipped-ring fixture exercises keyboard indexing while native Rust still handles the edit.
  const clippedRing=async(route:import("@playwright/test").Route)=>{
   const body=route.request().postDataJSON();
   if(body?.command?.op==="select"&&body.command.tool==="rotate"){
    const response=await route.fetch(),data=await response.json();
    if(data.result)for(const handle of data.result.handles)handle.points=handle.points.slice(0,4);
    await route.fulfill({response,json:data});
   }else await route.continue();
  };
  await page.route("**/api/inspection",clippedRing);
  await page.getByRole("group",{name:"Viewport tool"}).getByRole("button",{name:"Rotate",exact:true}).click();
  assert.equal((await points("rotate","Z")).length,4);
  const keyboardSaved=page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement")&&r.request().method()==="PUT",{timeout:120000});
  await page.getByRole("button",{name:"rotate Z handle",exact:true}).focus();await page.keyboard.press("ArrowRight");
  const keyboardResult=await keyboardSaved;assert.equal(keyboardResult.status(),200,await keyboardResult.text());
  await expect(page.getByRole("button",{name:"Load",exact:true})).toBeEnabled({timeout:120000});
  await page.unrouteAll({behavior:"wait"});
  const inspector=page.locator(".transform-inspector");
  await page.getByRole("button",{name:"Select instance far",exact:true}).click();
  await inspector.getByLabel("Rotation (degrees) Z",{exact:true}).fill("90");await inspector.getByLabel("Uniform scale",{exact:true}).check();await inspector.getByLabel("Scale X",{exact:true}).fill("2");
  await expect(inspector.getByLabel("Position X",{exact:true})).toBeEnabled({timeout:120000});
  await page.getByRole("button",{name:"Select instance near",exact:true}).click();await inspector.getByLabel("Parent",{exact:true}).selectOption("far");
  await expect(selected).toHaveAttribute("aria-level","2",{timeout:120000});await expect(inspector.getByLabel("Position X",{exact:true})).toBeEnabled({timeout:120000});
  const beforeGlobal=(parse(await readFile(path.join(root,scene),"utf8")).entities as typeof entries)[0].transform!.translation;
  await page.getByRole("group",{name:"Transform space"}).getByRole("button",{name:"Global",exact:true}).click();
  await dragHandle("Translate","X");
  const afterGlobal=(parse(await readFile(path.join(root,scene),"utf8")).entities as typeof entries)[0].transform!.translation;
  assert.ok(Math.abs(beforeGlobal[0]-afterGlobal[0])<0.001,"world X must preserve local X under a Z90 parent");assert.ok(afterGlobal[1]<beforeGlobal[1],"world X must move along negative local Y under a Z90 parent");
  await dragHandle("Scale","X",false,false,true);
  await dragHandle("Scale","Z");
  await page.screenshot({path:"/tmp/emergent-picking-gizmos.png"});
  await dragHandle("Translate","Z",false,true);
  assert.equal(await canvas.getAttribute("data-session"),session);assert.equal(await canvas.getAttribute("data-worker"),worker);assert.equal(await canvas.getAttribute("data-tick"),"0");assert.equal(await readFile(path.join(root,asset),"utf8"),text);
  await expect.poll(async()=>Number(await canvas.getAttribute("data-fps")),{timeout:10000}).toBeGreaterThan(20);
  await page.screenshot({path:"/tmp/emergent-picking-gizmos-conflict.png"});assert.deepEqual(errors,[]);
  console.log(`Gizmo smoke passed: nearest-object pick, three tools, clipped-ring keyboard editing, pointer/Escape cancellation, parented Global drags, shear rejection, save-on-release, independent instances and retained tick-zero worker; browser ${await canvas.getAttribute("data-fps")} fps.`);
 }catch(error){
  if(debugPage){await debugPage.screenshot({path:"/tmp/emergent-gizmo-failure.png"}).catch(()=>{});console.log("Viewport error:",await debugPage.locator(".inspection-error").allTextContents());}
  if(session)console.log("Worker status:",await(await fetch(`${base}/api/inspection?projectId=${projectId}&id=${session}`)).json());
  throw error;
 }finally{if(session)await fetch(`${base}/api/inspection`,{method:"DELETE",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,id:session})}).catch(()=>{});await browser.close();for(const file of[scene,asset,lamp])await unlink(path.join(root,file)).catch(e=>{if(e.code!=="ENOENT")throw e;});}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
