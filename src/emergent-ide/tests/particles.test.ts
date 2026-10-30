import assert from "node:assert/strict";
import test from "node:test";
import {createHash,randomUUID} from "node:crypto";
import {mkdir,mkdtemp,readFile,readdir,rm,writeFile} from "node:fs/promises";
import {execFile} from "node:child_process";
import {promisify} from "node:util";
import path from "node:path";
import os from "node:os";
import {parse,stringify} from "smol-toml";
import {particleDefinition,particlePreset,blankParticleSystem,ParticleSimulation} from "../src/lib/particles";
import {prepareParticleSave,readParticleAsset} from "../src/lib/particle-assets";
import {ParticleMediaJobs,bakeParticleVideo,createParticleMediaProvider,type ParticleMediaProvider} from "../src/lib/particle-media";
import {particleMatteInput,particleVideoInput} from "../src/lib/particle-media-types";
import {createProjectAsset} from "../src/lib/project-assets";
import type {ProjectSnapshot} from "../src/lib/project";
const execute=promisify(execFile);
test("blank particle systems are valid independent drafts with no emitted particles",()=>{
 const blank=particleDefinition(blankParticleSystem()),other=blankParticleSystem();
 assert.equal(blank.id,"draft");assert.equal(blank.emitters.length,1);assert.equal(blank.emitters[0].rate,0);assert.equal(blank.emitters[0].burst,0);
 const simulation=new ParticleSimulation(blank.emitters[0]);for(let i=0;i<120;i++)simulation.step();assert.equal(simulation.particles.length,0);
 blank.emitters[0].position[0]=12;assert.equal(other.emitters[0].position[0],0);
});
test("particle recipes validate limits and deterministic finite one-shot simulation",()=>{
 for(const preset of ["fire","smoke","dust","embers"]){const d=particleDefinition(particlePreset(preset));assert.deepEqual(particleDefinition(parse(stringify(d))),d);for(const e of d.emitters){const a=new ParticleSimulation(e),b=new ParticleSimulation(e);for(let i=0;i<600;i++){a.step();b.step();assert.ok(a.particles.length<=e.max_particles);}assert.deepEqual(a.particles,b.particles);assert.ok(a.particles.every(p=>p.position.every(Number.isFinite)));}}
 const d=particlePreset("smoke");for(const patch of [{max_particles:100001},{rate:NaN},{lifetime:[2,1]},{frames:2},{columns:0},{texture:"../escape.png"},{seed:-1},{unknown:1}])assert.throws(()=>particleDefinition({...d,emitters:[{...d.emitters[0],...patch}]}));
 for(const blend of ["alpha","additive","screen","multiply"] as const){const recipe={...d,emitters:[{...d.emitters[0],blend}]};assert.equal(particleDefinition(parse(stringify(recipe))).emitters[0].blend,blend);}
 assert.throws(()=>particleDefinition({...d,emitters:[{...d.emitters[0],blend:"invalid"}]}));
 const e={...d.emitters[0],looping:false,rate:0,burst:5,lifetime:[.1,.1]};const sim=new ParticleSimulation(e);sim.step();assert.equal(sim.particles.length,5);for(let i=0;i<60;i++)sim.step();assert.equal(sim.particles.length,0);
});
test("particle save preserves durable identity, detects stale revisions, and rejects a changed scene",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-particle-assets-"));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(path.join(root,"assets/particles"),{recursive:true});const project={id:"test",root,assetRoots:["assets"],files:[]} as unknown as ProjectSnapshot;
 await assert.rejects(createProjectAsset(project,"effects.particle-emitter","Smoke"),/Particle Generator/);
 const initial=await prepareParticleSave(project,{definition:particlePreset("fire")});for(const c of initial.changes)await writeFile(path.join(root,c.path),c.content);const entity=parse(await readFile(path.join(root,initial.result.assetPath),"utf8"));entity.id="separate-entity-identity";await writeFile(path.join(root,initial.result.assetPath),stringify(entity));const saved=await readParticleAsset(project,initial.result.assetPath);assert.equal(saved.definition.emitters.length,3);
 const updated=await prepareParticleSave(project,{definition:{...saved.definition,name:"Updated fire"},assetPath:saved.assetPath,revision:saved.revision});assert.equal(updated.result.assetPath,saved.assetPath);assert.equal(updated.result.definition.id,saved.definition.id);assert.equal(parse(updated.changes.find(c=>c.path===saved.assetPath)!.content).id,"separate-entity-identity");
 await writeFile(path.join(root,saved.effectPath),saved.effectSource+"\n# External edit\n");await assert.rejects(prepareParticleSave(project,{definition:saved.definition,assetPath:saved.assetPath,revision:saved.revision}),/externally/);
 await writeFile(path.join(root,"assets/main.scene.toml"),"schema_version=1\nid='main'\nentities=[]\n");project.files=[{path:"assets/main.scene.toml",name:"main"} as ProjectSnapshot["files"][number]];await assert.rejects(prepareParticleSave(project,{definition:particlePreset("fire"),scenePath:"assets/main.scene.toml",sceneRevision:"stale"}),/changed/);
 const before="# Keep this scene comment\nschema_version=1\nid='main'\nentities=[] # Empty scene\n";await writeFile(path.join(root,"assets/main.scene.toml"),before);const placed=await prepareParticleSave(project,{definition:particlePreset("fire"),scenePath:"assets/main.scene.toml",sceneRevision:createHash("sha256").update(before).digest("hex")});const after=placed.changes.find(c=>c.path==="assets/main.scene.toml")!.content;assert.ok(after.startsWith("# Keep this scene comment\nschema_version=1\nid='main'\n"));assert.equal((parse(after).entities as unknown[]).length,1);

});
test("paid media submission is idempotent across restarts and uncertain submissions never repeat",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-particle-jobs-"));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(path.join(root,"assets"));await writeFile(path.join(root,"assets/sprite.png"),Buffer.from([137,80,78,71,13,10,26,10]));const p={id:"test",root,assetRoots:["assets"]} as ProjectSnapshot;
 let submitted=0;const provider:ParticleMediaProvider={upload:async()=>"https://fal.media/source.png",submit:async()=>{submitted++;return "provider-request";},status:async()=>"queued",result:async()=>Buffer.alloc(0)};
 const id=randomUUID(),input={stage:"animate",source:"assets/sprite.png",prompt:"rising smoke",seed:1},data=path.join(root,"private");let jobs=new ParticleMediaJobs(data,async()=>provider);await Promise.all([jobs.create(p,id,input),jobs.create(p,id,input)]);assert.equal(submitted,1);jobs=new ParticleMediaJobs(data,async()=>provider);assert.equal((await jobs.create(p,id,input)).requestId,"provider-request");await jobs.refresh(p,id);assert.equal(submitted,1);
 const failId=randomUUID();provider.submit=async()=>{submitted++;throw new Error("network uncertain");};assert.equal((await jobs.create(p,failId,input)).state,"uncertain");await jobs.refresh(p,failId);await jobs.create(p,failId,input);assert.equal(submitted,2);await assert.rejects(jobs.create(p,id,{...input,prompt:"different"}),/another/);
 assert.equal(particleMatteInput("https://fal.media/x").background_color,"Transparent");assert.equal(particleMatteInput("x").auto_zoom,false);assert.equal(particleVideoInput("x","smoke",1).enable_output_safety_checker,true);
});
test("local video baking preserves soft alpha and refuses opaque smoke",{timeout:90000},async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-particle-bake-"));t.after(()=>rm(root,{recursive:true,force:true}));
 const alpha=path.join(root,"alpha.mov"),opaque=path.join(root,"opaque.mp4");
 await execute("ffmpeg",["-v","error","-f","lavfi","-i","nullsrc=s=64x64:r=8:d=1,format=rgba,geq=r=255:g=160:b=80:a='255*max(0,1-hypot(X-32,Y-32)/30)'","-c:v","qtrle","-y",alpha],{timeout:30000});
 await execute("ffmpeg",["-v","error","-f","lavfi","-i","color=red:s=64x64:r=8:d=1","-c:v","libx264","-pix_fmt","yuv420p","-y",opaque],{timeout:30000});
 const layout=await bakeParticleVideo(alpha,path.join(root,"alpha.png"),true);assert.deepEqual(layout,{columns:8,rows:4,frames:32,fps:8,alpha:true});assert.ok((await readFile(path.join(root,"alpha.png"))).length>100);
 await assert.rejects(bakeParticleVideo(opaque,path.join(root,"bad.png"),true),/soft alpha/);assert.equal((await bakeParticleVideo(opaque,path.join(root,"additive.png"),false)).alpha,false);
});

test("fal video schemas, warning handling, download boundaries and duplicate submission guard",async()=>{
 let paid=0,downloaded=0,url="https://fal.media/clip.webm",warning:string|undefined=undefined,submitted:Record<string,unknown>={};
 const transport:typeof fetch=async(input,init)=>{const target=String(input);if(target.startsWith("https://fal.media/")){downloaded++;return new Response(new Uint8Array([1,2,3]));}if(target.includes("/status"))return Response.json({status:"COMPLETED"});if(init?.method?.toUpperCase()==="POST"){paid++;submitted=JSON.parse(String(init.body));return Response.json({request_id:"video-request",status:"IN_QUEUE"});}return Response.json({video:{url},warning});};
 const provider=createParticleMediaProvider("fake-key",transport),endpoint="bria/video/background-removal/v3";
 assert.equal(await provider.submit(endpoint,particleMatteInput("https://fal.media/source.mp4")),"video-request");assert.equal(submitted.background_color,"Transparent");assert.equal(submitted.auto_zoom,false);assert.equal(await provider.status(endpoint,"video-request"),"ready");assert.equal((await provider.result(endpoint,"video-request")).length,3);await assert.rejects(provider.submit(endpoint,{}));assert.equal(paid,1);
 url="http://127.0.0.1/private";await assert.rejects(provider.result(endpoint,"video-request"),/Unsupported/);assert.equal(downloaded,1);url="https://fal.media/clip.webm";warning="Fell back to black";await assert.rejects(provider.result(endpoint,"video-request"),/warning/);assert.equal(downloaded,1);
});

test("effect swatches cache native stills, invalidate on recipe edits, and place saved assets without rewriting recipes",{timeout:120000},async t=>{
 const {particleThumbnail}=await import("../src/lib/particle-thumbnails");
 const {prepareParticlePlacement}=await import("../src/lib/particle-assets");
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-particle-swatches-"));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(path.join(root,"assets/particles"),{recursive:true});
 const project={id:"swatches",root,assetRoots:["assets"],files:[{path:"assets/main.scene.toml",name:"Main"}]} as ProjectSnapshot;
 const prepared=await prepareParticleSave(project,{definition:particlePreset("fire")});for(const change of prepared.changes)await writeFile(path.join(root,change.path),change.content);
 const before="# Preserve scene comments\nschema_version=1\nid='main'\nentities=[]\n";await writeFile(path.join(root,"assets/main.scene.toml"),before);
 let rendered=0;const png=Buffer.from([137,80,78,71,13,10,26,10]);const render:typeof import("../src/lib/particle-assets").captureParticles=async(_project,_definition,seconds,thumbnail)=>{rendered++;assert.equal(seconds,2);assert.equal(thumbnail,true);return png;};const data=path.join(root,"private");
 const [a,b]=await Promise.all([particleThumbnail(project,prepared.result.assetPath,render,data),particleThumbnail(project,prepared.result.assetPath,render,data)]);assert.deepEqual(a,png);assert.deepEqual(b,png);assert.equal(rendered,1);
 await particleThumbnail(project,prepared.result.assetPath,render,data);assert.equal(rendered,1);
 const original=await readFile(path.join(root,prepared.result.effectPath),"utf8");const placement=await prepareParticlePlacement(project,{assetPath:prepared.result.assetPath,scenePath:"assets/main.scene.toml",sceneRevision:createHash("sha256").update(before).digest("hex")});assert.equal((parse(placement.change.content).entities as {asset:string}[])[0].asset,prepared.result.assetPath);assert.equal(await readFile(path.join(root,prepared.result.effectPath),"utf8"),original);
 await writeFile(path.join(root,prepared.result.effectPath),original+"\n# changed recipe revision\n");await particleThumbnail(project,prepared.result.assetPath,render,data);assert.equal(rendered,2);
 const burst=particlePreset("smoke");burst.emitters[0]={...burst.emitters[0],looping:false,rate:0,burst:1,lifetime:[1/60,1/60]};await writeFile(path.join(root,prepared.result.effectPath),stringify(burst));await particleThumbnail(project,prepared.result.assetPath,async(_p,_d,seconds,thumbnail)=>{assert.equal(seconds,1/60);assert.equal(thumbnail,true);return png;},data);
 await assert.rejects(particleThumbnail(project,"../outside.entity.toml",render,data));assert.equal(rendered,2);
});

test("discarded video results stay deleted without resubmission and retain saved flipbooks and source provenance",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-particle-discard-"));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(path.join(root,"assets"));await writeFile(path.join(root,"assets/sprite.png"),Buffer.from([137,80,78,71,13,10,26,10]));
 const clip=path.join(root,"clip.mp4");await execute("ffmpeg",["-v","error","-f","lavfi","-i","color=red:s=32x32:r=8:d=1","-c:v","libx264","-pix_fmt","yuv420p","-y",clip]);
 const bytes=await readFile(clip),project={id:"delete-test",name:"Delete Test",root,assetRoots:["assets"],files:[],connections:[],entities:[],outputs:[],entryScene:"assets/main.scene.toml"} as ProjectSnapshot;let submitted=0;
 const provider:ParticleMediaProvider={upload:async()=>"https://fal.media/source.png",submit:async()=>{submitted++;return "provider-id";},status:async()=>"ready",result:async()=>bytes};
 const jobs=new ParticleMediaJobs(path.join(root,"private"),async()=>provider,async(_input,output)=>{await writeFile(output,Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aBmcAAAAASUVORK5CYII=","base64"));return {columns:1,rows:1,frames:1,fps:1,alpha:true};},async(p,data)=>{await mkdir(path.join(p.root,"assets/atlas"),{recursive:true});await writeFile(path.join(p.root,"assets/atlas/atlas.png"),data);return {path:"assets/atlas/atlas.png",name:"Atlas"};});
 const input={stage:"animate",source:"assets/sprite.png",prompt:"rising smoke",seed:1},id=randomUUID();await jobs.create(project,id,input);await assert.rejects(jobs.discard(project,id),/completed/);await jobs.refresh(project,id);const temporary=await jobs.artifact(project,id);
 await assert.rejects(jobs.discard({...project,id:"other-project"},id));await assert.rejects(jobs.discard(project,"../escape"));
 await jobs.discard(project,id);await jobs.discard(project,id);await assert.rejects(readFile(temporary),{code:"ENOENT"});assert.equal((await jobs.list(project)).length,0);await assert.rejects(jobs.artifact(project,id));await assert.rejects(jobs.refresh(project,id),/deleted/);await assert.rejects(jobs.create(project,id,input),/deleted/);await assert.rejects(jobs.atlas(project,id,true),/deleted/);assert.equal(submitted,1);
 const bakedId=randomUUID();await jobs.create(project,bakedId,input);await jobs.refresh(project,bakedId);const source=await jobs.artifact(project,bakedId),baked=await jobs.atlas(project,bakedId,true);await jobs.discard(project,bakedId);assert.ok((await readFile(path.join(root,baked.atlas!.path))).subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])));assert.deepEqual(await readFile(source),bytes);assert.equal((await jobs.list(project)).length,0);assert.equal(submitted,2);
});

test("animation requests preserve the entered prompt and explicit website-quality settings",()=>{
 const prompt="The blue smoke swirls menacingly - nothing but smoke, no additional images, no text",input=particleVideoInput("https://fal.media/source.png",prompt,10127565);
 assert.equal(input.prompt,prompt);assert.equal(input.resolution,"720p");assert.equal(input.num_inference_steps,40);assert.equal(input.guidance_scale,3.5);assert.equal(input.shift,5);assert.equal(input.interpolator_model,"film");assert.equal(input.num_interpolated_frames,0);assert.equal(input.video_quality,"high");assert.equal(input.video_write_mode,"balanced");assert.equal(input.seed,10127565);assert.equal(input.enable_safety_checker,true);assert.equal(input.enable_output_safety_checker,true);assert.ok(!("negative_prompt" in input));
});

test("emitter ping-pong playback persists while legacy effects keep forward playback",()=>{
 const legacy=particlePreset("smoke");delete legacy.emitters[0].ping_pong;assert.equal(particleDefinition(legacy).emitters[0].ping_pong,false);
 legacy.emitters[0].ping_pong=true;const parsed=particleDefinition(parse(stringify(legacy)));assert.equal(parsed.emitters[0].ping_pong,true);
 assert.throws(()=>particleDefinition({...legacy,emitters:[{...legacy.emitters[0],ping_pong:"yes"}]}));
});

test("failed media jobs can be deleted permanently without extra provider calls",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"failed-particle-media-"));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(path.join(root,"assets"));await writeFile(path.join(root,"assets/sprite.png"),"source");
 const p={id:"failed-media",root,assetRoots:["assets"]} as ProjectSnapshot;let submitted=0,checks=0;
 const jobs=new ParticleMediaJobs(path.join(root,"private"),async()=>({upload:async()=>"https://fal.media/input.png",submit:async()=>{submitted++;return "request";},status:async()=>{checks++;return "ready";},result:async()=>{throw new Error("Unsupported fal download host.");}}));
 const id=randomUUID(),input={stage:"animate",source:"assets/sprite.png",prompt:"lightning ball",seed:1};await jobs.create(p,id,input);await assert.rejects(jobs.discard(p,id),/completed or failed/);const failed=await jobs.refresh(p,id);assert.equal(failed.state,"failed");assert.match(failed.error!,/Unsupported fal download host/);
 const history=(await readdir(path.join(root,"private/particle-media")))[0],folder=path.join(root,"private/particle-media",history,id);await writeFile(path.join(folder,"source.mp4"),"temporary failed download");
 await jobs.discard(p,id);await jobs.discard(p,id);assert.deepEqual(await jobs.list(p),[]);await assert.rejects(readFile(path.join(folder,"source.mp4")),{code:"ENOENT"});
 const restarted=new ParticleMediaJobs(path.join(root,"private"));assert.deepEqual(await restarted.list(p),[]);await assert.rejects(restarted.refresh(p,id),/deleted/);await assert.rejects(restarted.create(p,id,input),/deleted/);assert.equal(submitted,1);assert.equal(checks,1);assert.equal(await readFile(path.join(root,"assets/sprite.png"),"utf8"),"source");assert.ok(await readFile(path.join(folder,"submission.claim")));
});
