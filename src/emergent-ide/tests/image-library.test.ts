import assert from "node:assert/strict";
import test from "node:test";
import {projectImages,spriteFrameAt,imageLibrarySettings} from "../src/lib/image-library";
import type {ProjectFile,ProjectSnapshot} from "../src/lib/project";
import {texturePrompt,textureRequest} from "../src/lib/texture-job-types";

test("image library discovers existing and generated images across declared roots without exposing other files",()=>{
 const files=[
  {path:"assets/textures/generated.png",name:"generated.png",displayName:"Forest",modifiedAt:"2026-10-28"},
  {path:"art/reference.JPG",name:"reference.JPG",modifiedAt:"2026-10-27"},
  {path:"assets/models/reference.webp",name:"reference.webp",modifiedAt:"2026-10-26"},
  {path:"src/private.png",name:"private.png",modifiedAt:"2026-10-29"},
  {path:"artifacts/other.png",name:"other.png",modifiedAt:"2026-10-29"},
  {path:"assets/textures/texture.asset.json",name:"texture.asset.json",modifiedAt:"2026-10-29"},
 ] as ProjectFile[];
 const images=projectImages({assetRoots:["assets","art/"],files} as ProjectSnapshot);
 assert.deepEqual(images.map(image=>[image.path,image.name]),[["assets/textures/generated.png","Forest"],["art/reference.JPG","reference"],["assets/models/reference.webp","reference"]]);
 assert.deepEqual(Object.keys(images[0]).sort(),["kind","modifiedAt","name","path"]);
});

test("general image prompts are sent verbatim without surface texture instructions",()=>{
 const input=textureRequest({name:"Forest",prompt:"A moonlit forest with a winding river",seed:42,count:1,tileFriendly:false});
 assert.equal(texturePrompt(input),input.prompt);
});

test("sprite sheet settings validate library labels, native grid limits and playback bounds",async()=>{
 const {imageLibrarySettings}=await import("../src/lib/image-library");
 assert.deepEqual(imageLibrarySettings({kind:"sprite",sprite:{columns:2,rows:3,frames:5,fps:8}}),{kind:"sprite",sprite:{columns:2,rows:3,frames:5,fps:8}});
 for(const patch of [{kind:"video"},{sprite:{columns:0,rows:1,frames:1,fps:12}},{sprite:{columns:17,rows:1,frames:1,fps:12}},{sprite:{columns:2,rows:2,frames:5,fps:12}},{sprite:{columns:2,rows:2,frames:4,fps:NaN}},{sprite:{columns:2,rows:2,frames:4,fps:0}}])assert.throws(()=>imageLibrarySettings({kind:"sprite",sprite:{columns:1,rows:1,frames:1,fps:12},...patch}));
});

test("sprite labels and atlas settings persist beside the unchanged image and reject unsafe paths",async t=>{
 const {mkdir,mkdtemp,readFile,rm,symlink,writeFile}=await import("node:fs/promises"),{execFileSync}=await import("node:child_process"),path=await import("node:path"),os=await import("node:os");
 const {configureLibraryImage}=await import("../src/lib/image-library-assets"),{readProjectSnapshot}=await import("../src/lib/project");
 const root=await mkdtemp(path.join(os.tmpdir(),"image-library-metadata-")),previous=process.env.EMERGENT_PROJECT_PATH;
 t.after(async()=>{if(previous===undefined)delete process.env.EMERGENT_PROJECT_PATH;else process.env.EMERGENT_PROJECT_PATH=previous;await rm(root,{recursive:true,force:true});});process.env.EMERGENT_PROJECT_PATH=root;
 await mkdir(path.join(root,"assets"));await mkdir(path.join(root,"src"));await writeFile(path.join(root,"emergent.toml"),'id="sheet-test"\nname="Sheet test"\nasset_roots=["assets"]\nentry_scene="assets/main.scene.toml"\n');
 execFileSync("ffmpeg",["-v","error","-f","lavfi","-i","color=red:s=4x4","-frames:v","1","-y",path.join(root,"assets/sheet.png")]);const original=await readFile(path.join(root,"assets/sheet.png")),project=await readProjectSnapshot(),settings={kind:"sprite",sprite:{columns:2,rows:2,frames:3,fps:8}};
 await configureLibraryImage(project,"assets/sheet.png",settings);const image=projectImages(await readProjectSnapshot())[0];assert.equal(image.kind,"sprite");assert.deepEqual(image.sprite,settings.sprite);assert.deepEqual(await readFile(path.join(root,"assets/sheet.png")),original);
 await assert.rejects(configureLibraryImage(project,"assets/sheet.png",{...settings,sprite:{...settings.sprite,columns:3}}),/divide/);await assert.rejects(configureLibraryImage(project,"../outside.png",settings),/project library/);
 await writeFile(path.join(root,"outside.png"),original);await symlink(path.join(root,"outside.png"),path.join(root,"assets/link.png"));await assert.rejects(configureLibraryImage(project,"assets/link.png",settings),/symbolic/);
 await writeFile(path.join(root,"assets/other.png"),original);await symlink(path.join(root,"outside.png"),path.join(root,"assets/other.png.library.json"));await assert.rejects(configureLibraryImage(project,"assets/other.png",settings),/symbolic/);assert.deepEqual(await readFile(path.join(root,"outside.png")),original);
 await configureLibraryImage(project,"assets/sheet.png",{...settings,kind:"image"});const reverted=projectImages(await readProjectSnapshot()).find(i=>i.path==="assets/sheet.png")!;assert.equal(reverted.kind,"image");assert.equal(reverted.sprite,undefined);
});

test("ping-pong playback reflects the configured frames and handles static and two-frame sheets",()=>{
 assert.deepEqual(Array.from({length:13},(_,step)=>spriteFrameAt(step,4,true)),[0,1,2,3,2,1,0,1,2,3,2,1,0]);
 assert.deepEqual(Array.from({length:9},(_,step)=>spriteFrameAt(step,4)),[0,1,2,3,0,1,2,3,0]);
 assert.deepEqual(Array.from({length:6},(_,step)=>spriteFrameAt(step,2,true)),[0,1,0,1,0,1]);
 for(let frames=1;frames<=256;frames++)for(let step=0;step<frames*4;step++)assert.ok(spriteFrameAt(step,frames,true)<frames);
 assert.equal(spriteFrameAt(1000,1,true),0);assert.equal(spriteFrameAt(3.9,4,true),3);
 assert.equal(imageLibrarySettings({kind:"sprite",sprite:{columns:3,rows:2,frames:5,fps:8,ping_pong:true}}).sprite.ping_pong,true);
 assert.throws(()=>imageLibrarySettings({kind:"sprite",sprite:{columns:2,rows:2,frames:4,fps:8,ping_pong:"yes"}}));
});
