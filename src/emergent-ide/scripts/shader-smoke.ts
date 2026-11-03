import assert from "node:assert/strict";
import {parse} from "smol-toml";
import {smokeProjectRoot} from "./project-fixture";
import {readFile,unlink} from "node:fs/promises";
import path from "node:path";
import {chromium,expect} from "@playwright/test";
import {shaderTemplate} from "../src/lib/shader-graph";
async function main(){
const base=process.env.EMERGENT_IDE_BASE_URL??"http://localhost:3001",root=smokeProjectRoot();
const projectId=String(parse(await readFile(path.join(root,"emergent.toml"),"utf8")).id);
const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});
const page=await browser.newPage({viewport:{width:1600,height:1100}}),created:string[]=[],errors:string[]=[];
page.on("pageerror",e=>errors.push(e.message));page.on("console",m=>{if(m.type()==="error"&&!m.text().includes("favicon"))errors.push(m.text());});
try{
 await page.goto(`${base}/shaders`);await expect(page.getByRole("heading",{name:"Preview",exact:true})).toBeVisible({timeout:120000});
 const canvas=page.getByRole("img",{name:"Live shader preview"});await expect.poll(async()=>Number(await canvas.getAttribute("data-time"))).toBeGreaterThan(.1);
 await expect(page.getByLabel("Ask shader AI",{exact:true})).toBeEnabled();
 await expect(page.getByRole("region",{name:"Shader library",exact:true})).toBeVisible();
 await expect(page.getByRole("combobox",{name:"Shader library",exact:true})).toHaveCount(0);
 await page.getByRole("button",{name:"New blank shader",exact:true}).click();
 assert.equal(await page.locator(".shader-card[data-node]").count(),3,"new tile creates a blank graph");
 const stage=page.getByRole("region",{name:"Shader preview",exact:true}),controls=page.getByRole("region",{name:"Preview controls",exact:true});
 const imageBefore=(await canvas.boundingBox())!;
 await controls.evaluate(element=>{element.scrollTop=element.scrollHeight;});
 const imageAfter=(await canvas.boundingBox())!;assert.equal(imageAfter.y,imageBefore.y,"preview stays fixed while controls scroll");
 assert.ok((await stage.boundingBox())!.y+(await stage.boundingBox())!.height<=1100,"preview card stays within viewport");
 await controls.evaluate(element=>{element.scrollTop=0;});
 await page.setViewportSize({width:1440,height:800});
 await expect.poll(async()=>{const r=(await stage.boundingBox())!;return r.y+r.height;}).toBeLessThanOrEqual(800);
 const shortGraph=(await page.getByLabel("Shader node graph",{exact:true}).boundingBox())!,graphPanel=(await page.getByRole("region",{name:"Shader graph",exact:true}).boundingBox())!;
 assert.ok(shortGraph.y+shortGraph.height<=graphPanel.y+graphPanel.height,"the complete graph viewport fits inside its card");
 await controls.evaluate(element=>{element.scrollTop=element.scrollHeight;});
 await expect(page.getByRole("button",{name:"Generate draft",exact:true})).toBeInViewport();
 await controls.evaluate(element=>{element.scrollTop=0;});
 await page.screenshot({path:"/tmp/emergent-shader-layout-short.png"});
 await page.setViewportSize({width:820,height:900});
 assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),"narrow layout must not overflow horizontally");
 await page.setViewportSize({width:1600,height:1100});

 const graphSurface=page.getByLabel("Shader node graph",{exact:true});
 const scale=()=>page.locator(".shader-graph-layer").evaluate(element=>new DOMMatrix(getComputedStyle(element).transform).a);
 const beforeZoom=await scale();await page.getByRole("button",{name:"Zoom in graph",exact:true}).click();assert.ok(await scale()>beforeZoom);
 await page.getByRole("button",{name:"Zoom out graph",exact:true}).click();assert.ok(Math.abs(await scale()-beforeZoom)<.001);
 await page.getByRole("button",{name:"Fit all and center graph",exact:true}).click();
 assert.ok(await graphSurface.evaluate(element=>{const viewport=element.getBoundingClientRect();return [...element.querySelectorAll(".shader-card")].every(node=>{const r=node.getBoundingClientRect();return r.left>=viewport.left&&r.right<=viewport.right&&r.top>=viewport.top&&r.bottom<=viewport.bottom;});}),"fit includes every node and the output");
 await page.getByRole("button",{name:"Pause",exact:true}).click();
 await page.getByLabel("Load shader example",{exact:true}).selectOption("force-field");
 await expect(page.getByRole("button",{name:"Pause",exact:true})).toBeVisible();
 await expect.poll(async()=>Number(await canvas.getAttribute("data-time"))).toBeGreaterThan(.1);
 await page.getByRole("button",{name:"Pause",exact:true}).click();
 const brightness=async(time:number)=>{
  await page.getByLabel("Preview time",{exact:true}).fill(String(time));
  await expect.poll(async()=>Number(await canvas.getAttribute("data-time"))).toBeCloseTo(time,4);
  const png=(await canvas.screenshot()).toString("base64");
  return page.evaluate(async png=>{const image=new Image();image.src=`data:image/png;base64,${png}`;await image.decode();const surface=new OffscreenCanvas(image.width,image.height),context=surface.getContext("2d")!;context.drawImage(image,0,0);const pixels=context.getImageData(Math.floor(image.width*.3),Math.floor(image.height*.3),Math.floor(image.width*.4),Math.floor(image.height*.4)).data;let total=0;for(let i=0;i<pixels.length;i+=4)total+=pixels[i+2];return total/(pixels.length/4);},png);
 };
 const bright=await brightness(Math.PI/4),dim=await brightness(3*Math.PI/4);
 assert.ok(bright-dim>10,`force field pulse must visibly change pixels: bright=${bright}, dim=${dim}`);

 await page.getByLabel("Load shader example",{exact:true}).selectOption("water");await expect(page.getByLabel("Preview shape")).toHaveValue("plane");
 await page.getByRole("button",{name:"Pause",exact:true}).click();await page.getByLabel("Preview time",{exact:true}).fill("1.2");
 await page.getByLabel("Parameter height default",{exact:true}).fill("0.3");await expect(page.locator(".shader-shell [role=alert]")).toHaveCount(0);
 await page.getByRole("button",{name:"Undo",exact:true}).click();await expect(page.getByLabel("Parameter height default",{exact:true})).toHaveValue("0.12");
 // Node graph: add from the palette, wire by dragging sockets, unplug by dragging a connected input away.
 const centre=async(label:string)=>{const box=(await page.getByLabel(label,{exact:true}).boundingBox())!;return [box.x+box.width/2,box.y+box.height/2] as const;};
 const dragWire=async(from:string,to:string|readonly [number,number])=>{const a=await centre(from),b=typeof to==="string"?await centre(to):to;await page.mouse.move(...a);await page.mouse.down();await page.mouse.move(b[0],b[1],{steps:8});await page.mouse.up();};
 await page.getByRole("toolbar",{name:"Add shader node"}).getByRole("button",{name:"Sine",exact:true}).click();
 await expect(page.locator(".shader-shell [role=alert]")).toContainText("Connect Radians on sine-1");
 await dragWire("time output","sine-1 Radians input");await expect(page.locator(".shader-shell [role=alert]")).toHaveCount(0);
 await page.locator(".shader-graph-surface").click({position:{x:(await graphSurface.boundingBox())!.width-8,y:8}});
 await dragWire("sine-1 output","Surface Emission input");await expect(page.getByLabel("Output Emission",{exact:true})).toHaveValue("sine-1");
 await dragWire("Surface Emission input",[(await centre("Surface Emission input"))[0]-160,(await centre("Surface Emission input"))[1]+120]);
 await expect(page.locator(".shader-shell [role=alert]")).toContainText("Connect Emission on Surface output");
 await page.getByRole("button",{name:"Undo",exact:true}).click();await expect(page.getByLabel("Output Emission",{exact:true})).toHaveValue("sine-1");
 await dragWire("displace output","phase A input");await expect(page.locator(".shader-shell [role=alert]")).toContainText("loop");
 await page.screenshot({path:"/tmp/emergent-shader-graph.png"});
 await page.getByLabel("Shader name",{exact:true}).fill("");await expect(page.locator(".shader-shell [role=alert]")).toBeVisible();

 await page.route("**/api/shaders/ai",route=>route.fulfill({json:{message:"Draft changed; review before saving.",graph:{...shaderTemplate("water"),name:"AI water draft"}}}));
 await page.getByLabel("Ask shader AI",{exact:true}).fill("Make a water shader from scratch.");await page.getByRole("button",{name:"Generate draft",exact:true}).click();await expect(page.getByLabel("Shader name",{exact:true})).toHaveValue("AI water draft");
 const save=page.waitForResponse(r=>r.url().endsWith("/api/shaders")&&r.request().method()==="POST",{timeout:180000});await page.getByRole("button",{name:"Save new shader",exact:true}).click();const response=await save;assert.equal(response.status(),201,await response.text());const asset=await response.json();created.push(asset.path);
 await expect(page.getByRole("button",{name:"Create material",exact:true})).toBeEnabled({timeout:10000});
 const material=page.waitForResponse(r=>r.url().endsWith("/api/materials")&&r.request().method()==="POST",{timeout:180000});await page.getByRole("button",{name:"Create material",exact:true}).click();const materialResponse=await material;assert.equal(materialResponse.status(),201,await materialResponse.text());const materialAsset=await materialResponse.json();created.push(materialAsset.path);assert.equal(materialAsset.material.shader,asset.path);
 const conflict=await page.request.post(`${base}/api/shaders`,{data:{projectId,path:asset.path,revision:"stale",graph:asset.graph}});assert.equal(conflict.status(),400);
 await page.screenshot({path:"/tmp/emergent-shader-editor.png",fullPage:false});
 await page.getByRole("link",{name:"Open material ↗",exact:true}).click();await expect(page.getByLabel("Material shader",{exact:true})).toHaveValue(asset.path,{timeout:120000});await expect(page.getByLabel("Shader Wave height (m)",{exact:true})).toBeVisible();
 assert.deepEqual(errors,[]);console.log("PASS: shader templates, animation, undo, invalid-draft retention, mocked AI draft, native-validated save, material creation, conflict guard and material controls");
}finally{await browser.close();for(const asset of created.reverse())await unlink(path.join(root,asset));}

}
main().catch(error=>{console.error(error);process.exitCode=1;});
