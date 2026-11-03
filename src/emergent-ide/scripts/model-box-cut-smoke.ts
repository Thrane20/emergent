import assert from "node:assert/strict";
import {mkdir,mkdtemp,readFile,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import * as THREE from "three";
import {GLTFLoader} from "three/addons/loaders/GLTFLoader.js";
import {chromium,expect} from "@playwright/test";
import {parse} from "smol-toml";
import {prepareTestProject,smokeProjectRoot} from "./project-fixture";
const folder="assets/models/box-cut-qa";
async function prepare(){
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-box-cut-qa-"));await prepareTestProject(root,"box-cut-qa");
 const geometry=new THREE.BoxGeometry(2,2,2),arrays=[geometry.getAttribute("position").array,geometry.getAttribute("normal").array,geometry.getAttribute("uv").array,geometry.index!.array];
 let offset=0;const views=arrays.map(array=>{const view={buffer:0,byteOffset:offset,byteLength:array.byteLength};offset+=array.byteLength;return view;});
 const document={asset:{version:"2.0"},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0,name:"QA cube"}],meshes:[{primitives:[{attributes:{POSITION:0,NORMAL:1,TEXCOORD_0:2},indices:3,material:0}]}],materials:[{doubleSided:true,pbrMetallicRoughness:{baseColorFactor:[0.45,0.7,0.95,1],metallicFactor:0,roughnessFactor:0.8}}],buffers:[{byteLength:offset}],bufferViews:views,accessors:[{bufferView:0,componentType:5126,count:24,type:"VEC3",min:[-1,-1,-1],max:[1,1,1]},{bufferView:1,componentType:5126,count:24,type:"VEC3"},{bufferView:2,componentType:5126,count:24,type:"VEC2"},{bufferView:3,componentType:5123,count:36,type:"SCALAR"}]};
 const raw=Buffer.from(JSON.stringify(document)),json=Buffer.concat([raw,Buffer.alloc((4-raw.length%4)%4,32)]),bytes=Buffer.concat(arrays.map(array=>Buffer.from(array.buffer,array.byteOffset,array.byteLength)));
 const header=Buffer.alloc(12),j=Buffer.alloc(8),b=Buffer.alloc(8);header.writeUInt32LE(0x46546c67);header.writeUInt32LE(2,4);header.writeUInt32LE(28+json.length+bytes.length,8);j.writeUInt32LE(json.length);j.writeUInt32LE(0x4e4f534a,4);b.writeUInt32LE(bytes.length);b.writeUInt32LE(0x004e4942,4);
 await mkdir(path.join(root,folder),{recursive:true});await writeFile(path.join(root,folder,"model.glb"),Buffer.concat([header,j,json,b,bytes]));await writeFile(path.join(root,folder,"object.asset.json"),JSON.stringify({schema_version:1,kind:"generated-model",id:"11111111-1111-4111-8111-111111111111",name:"Box Cut QA",model:`${folder}/model.glb`}));geometry.dispose();console.log(root);
}
async function main(){
 if(process.argv.includes("--prepare"))return prepare();
 const root=smokeProjectRoot();assert.ok(path.basename(root).startsWith("emergent-box-cut-qa-"),"Use only the disposable project from --prepare");
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL??"chrome",headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1600,height:1100}}),errors:string[]=[];page.on("pageerror",error=>errors.push(error.message));
  await page.route("**/api/fal-balance*",route=>route.fulfill({json:{state:"no-key",balance:0,currency:"USD",message:"No key",checkedAt:null}}));
  await page.goto(`${process.env.EMERGENT_NAV_TEST_URL??"http://localhost:3001"}/models`);
  const activate=page.getByRole("button",{name:"Cut with box",exact:true}),canvas=page.getByLabel("Model parts 3D viewport"),handles=page.getByRole("button",{name:/Resize cut box corner/});
  await expect(activate).toBeEnabled({timeout:20000});const size=(axis:string)=>page.getByLabel(`Box size ${axis}`,{exact:true});
  async function draw(){await activate.click();const r=(await canvas.boundingBox())!;await page.mouse.move(r.x+r.width*.43,r.y+r.height*.42);await page.mouse.down();await page.mouse.move(r.x+r.width*.63,r.y+r.height*.6,{steps:10});await page.mouse.up();await expect(handles).toHaveCount(8);}
  await draw();assert.ok(Math.abs(Number(await size("X").inputValue())-Number(await size("Z").inputValue()))<.00001,"initial depth equals width");await expect(page.getByText("UNSAVED",{exact:true})).toHaveCount(0);
  const depth=Number(await size("Z").inputValue()),r=(await canvas.boundingBox())!;await page.mouse.move(r.x+45,r.y+100);await page.mouse.down();await page.mouse.move(r.x+180,r.y+150,{steps:20});await page.mouse.up();
  const corner=handles.nth(7);await corner.focus();await corner.press("ArrowRight");await expect.poll(async()=>Math.abs(Number(await size("Z").inputValue())-depth)).toBeGreaterThan(.00001);
  const before=await size("X").inputValue(),h=(await corner.boundingBox())!;await page.mouse.move(h.x+h.width/2,h.y+h.height/2);await page.mouse.down();await page.mouse.move(h.x+h.width/2+25,h.y+h.height/2-15,{steps:10});await page.mouse.up();await expect.poll(()=>size("X").inputValue()).not.toBe(before);
  await page.screenshot({path:"/tmp/emergent-box-cut-handles.png",fullPage:true});
  await page.getByRole("button",{name:"Cancel box cut",exact:true}).click();await expect(handles).toHaveCount(0);await expect(page.getByText("UNSAVED",{exact:true})).toHaveCount(0);
  await activate.click();await page.mouse.move(r.x+100,r.y+100);await page.mouse.down();await page.mouse.move(r.x+160,r.y+160);await canvas.press("Escape");await page.mouse.up();await expect(activate).toHaveAttribute("aria-pressed","false");await expect(handles).toHaveCount(0);
  await draw();for(const axis of ["X","Y","Z"]){await page.getByLabel(`Box centre ${axis}`,{exact:true}).fill("0");await size(axis).fill(axis==="Z"?"4":"1");}
  await page.getByRole("button",{name:"Cut",exact:true}).click();await expect(page.getByText("UNSAVED",{exact:true})).toBeVisible();await expect(handles).toHaveCount(0);await expect(page.getByText(/Removed the surfaces inside the box/)).toBeVisible();await page.screenshot({path:"/tmp/emergent-box-cut-result.png",fullPage:true});
  await page.getByRole("button",{name:"Save To Object Library",exact:true}).click();await expect(page.getByText("Saved 1 part to Object Library.",{exact:true})).toBeVisible({timeout:180000});
  const document=parse(await readFile(path.join(root,folder,"parts/parts.toml"),"utf8")) as unknown as {parts:{id:string;asset:string;pivot:number[]}[]};assert.equal(document.parts.length,1);assert.deepEqual(document.parts[0].pivot,[0,0,0]);
  const bytes=await readFile(path.join(root,document.parts[0].asset.replace(/\.entity\.toml$/,".glb"))),gltf=await new GLTFLoader().parseAsync(bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength),"");let area=0;
  gltf.scene.traverse(object=>{if(!(object instanceof THREE.Mesh))return;const g=object.geometry,p=g.getAttribute("position"),index=g.index!;assert.equal(p.count,g.getAttribute("uv").count);for(let i=0;i<index.count;i+=3){const a=new THREE.Vector3().fromBufferAttribute(p,index.getX(i)),b=new THREE.Vector3().fromBufferAttribute(p,index.getX(i+1)),c=new THREE.Vector3().fromBufferAttribute(p,index.getX(i+2));area+=b.sub(a).cross(c.sub(a)).length()/2;}});
  assert.ok(area>0&&area<23.9,`saved cube must have surface removed, got ${area}/24`);
  await page.reload();await expect(activate).toBeEnabled();await expect(page.getByText("UNSAVED",{exact:true})).toHaveCount(0);const reopened=parse(await readFile(path.join(root,folder,"parts/parts.toml"),"utf8")) as unknown as {parts:{id:string}[]};assert.equal(reopened.parts[0].id,document.parts[0].id);assert.deepEqual(errors,[]);
  console.log(`PASS rectangle/depth, eight corners, orbit/depth and pointer resizing, Cancel/Escape, explicit Cut, native-validated save/reopen; surface area ${area.toFixed(4)}/24. No paid calls.`);
 }finally{await browser.close();}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
