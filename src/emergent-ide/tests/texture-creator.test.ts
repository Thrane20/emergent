import assert from "node:assert/strict";
import test,{type TestContext} from "node:test";
import {randomUUID} from "node:crypto";
import {mkdir,mkdtemp,readFile,readdir,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {ParticleMediaJobs,type ParticleMediaProvider} from "../src/lib/particle-media";
import {deleteLibraryImage} from "../src/lib/image-library-assets";
import {TextureJobs} from "../src/lib/texture-jobs";
import {createTextureProvider,type TextureProvider} from "../src/lib/texture-provider";
import {falModels} from "../src/lib/fal-models";
import {ProviderFailure} from "../src/lib/object-provider";
import {textureRequest,TEXTURE_ENDPOINT} from "../src/lib/texture-job-types";
import {readProjectSnapshot,type ProjectSnapshot} from "../src/lib/project";
const input={name:"Grass",prompt:"Soft moss and earth",seed:42,count:2,tileFriendly:true};
async function fixture(t:TestContext){
 const temporary=await mkdtemp(path.join(os.tmpdir(),"emergent-texture-jobs-"));t.after(()=>rm(temporary,{recursive:true,force:true}));const root=path.join(temporary,"game");await mkdir(path.join(root,"assets"),{recursive:true});await mkdir(path.join(root,"src"));await writeFile(path.join(root,"emergent.toml"),'id="textures"\nname="Textures"\nentry_scene="assets/main.scene.toml"\nasset_roots=["assets"]');
 const project:ProjectSnapshot={root,id:"textures",name:"Textures",entryScene:"assets/main.scene.toml",assetRoots:["assets"],files:[],entities:[],connections:[],outputs:[]};let submissions=0,saves=0;
 const provider:TextureProvider={submit:async()=>{submissions++;return "fal-request";},status:async()=>"completed",result:async()=>[Buffer.from("png-one"),Buffer.from("png-two")]};
 const validate=async()=>"png" as const;
 const saveImage=async(project:ProjectSnapshot,bytes:Buffer,name:string,id?:string)=>{saves++;const relative=`assets/textures/${id}/${name}.png`;await mkdir(path.dirname(path.join(project.root,relative)),{recursive:true});try{await writeFile(path.join(project.root,relative),bytes,{flag:"wx"});}catch(error){if((error as NodeJS.ErrnoException).code!=="EEXIST")throw error;assert.deepEqual(await readFile(path.join(project.root,relative)),bytes);}return {path:relative,name:`${name}.png`};};
 const data=path.join(temporary,"data"),jobs=new TextureJobs(data,async()=>provider,validate,saveImage);
 return {root,temporary,project,provider,data,jobs,validate,saveImage,submissions:()=>submissions,saves:()=>saves};
}
test("texture batches resume, submit once, save independent candidates and expose project images",async t=>{
 const f=await fixture(t),id=randomUUID();const [first,duplicate]=await Promise.all([f.jobs.create(f.project,id,input),f.jobs.create(f.project,id,input)]);assert.equal(f.submissions(),1);assert.equal(first.requestId,duplicate.requestId);assert.match(first.submittedPrompt,/tileable/);
 const restarted=new TextureJobs(f.data,async()=>f.provider,f.validate,f.saveImage),ready=await restarted.refresh(f.project,id);assert.equal(ready.state,"ready");assert.equal(ready.outputs.length,2);
 const saved=await restarted.save(f.project,id,ready.outputs[0].id,"Ground"),second=await restarted.save(f.project,id,ready.outputs[1].id,"Ground");assert.notEqual(saved.outputs[0].savedPath,second.outputs[1].savedPath);
 await restarted.save(f.project,id,ready.outputs[0].id,"Another name");assert.equal(f.saves(),2);
 const descriptor=JSON.parse(await readFile(path.join(f.root,path.dirname(second.outputs[1].savedPath!),"texture.asset.json"),"utf8"));assert.equal(descriptor.prompt,ready.submittedPrompt);assert.equal(descriptor.seed,42);assert.equal(descriptor.kind,"generated-texture");assert.equal(descriptor.modelRevision,null);
 const previous=process.env.EMERGENT_PROJECT_PATH;process.env.EMERGENT_PROJECT_PATH=f.root;try{const snapshot=await readProjectSnapshot();assert.equal(snapshot.files.filter(file=>file.kind==="texture"&&file.displayName==="Ground").length,2);assert.ok(snapshot.connections.some(edge=>edge.target===second.outputs[1].savedPath));}finally{if(previous===undefined)delete process.env.EMERGENT_PROJECT_PATH;else process.env.EMERGENT_PROJECT_PATH=previous;}
 await assert.rejects(restarted.create(f.project,id,{...input,prompt:"Different"}),/different prompt/);await assert.rejects(restarted.artifact(f.project,id,"../job.json"));
 const other={...f.project,root:path.join(f.temporary,"other")};await mkdir(other.root);await assert.rejects(restarted.get(other,id),/not found/);
});
test("uncertain submissions and restart intent never submit again",async t=>{
 const f=await fixture(t);let calls=0;f.provider.submit=async()=>{calls++;throw new Error("lost response");};const id=randomUUID(),job=await f.jobs.create(f.project,id,input);assert.equal(job.state,"uncertain");await f.jobs.create(f.project,id,input);await f.jobs.refresh(f.project,id);assert.equal(calls,1);
 const files=await readdir(path.join(f.data,"texture-jobs")),folder=path.join(f.data,"texture-jobs",files[0],id);await writeFile(path.join(folder,"job.json"),JSON.stringify({...job,state:"submitting"}));assert.equal((await new TextureJobs(f.data,async()=>f.provider).refresh(f.project,id)).state,"uncertain");assert.equal(calls,1);
 const orphan=path.join(path.dirname(folder),randomUUID());await mkdir(orphan);await writeFile(path.join(orphan,"submission.claim"),"claimed");assert.equal((await f.jobs.list(f.project)).length,1);
});
test("observation errors resume the same request and invalid generated images do not become ready",async t=>{
 const f=await fixture(t),id=randomUUID();await f.jobs.create(f.project,id,input);f.provider.status=async()=>{throw new ProviderFailure("temporary",false);};assert.equal((await f.jobs.refresh(f.project,id)).state,"queued");
 f.provider.status=async()=>"completed";const bad=new TextureJobs(f.data,async()=>f.provider,async()=>{throw new Error("corrupt pixels");},f.saveImage);const failed=await bad.refresh(f.project,id);assert.equal(failed.state,"failed");assert.equal(failed.outputs.length,0);assert.match(failed.error!,/validation failed/);assert.equal((await f.jobs.refresh(f.project,id)).state,"ready");assert.equal(f.submissions(),1);
});
test("failed credentials create no job and interrupted saves recover their original identity/name",async t=>{
 const f=await fixture(t),id=randomUUID(),missing=new TextureJobs(f.data,async()=>{throw new Error("missing credentials");});await assert.rejects(missing.create(f.project,id,input),/credentials/);assert.equal((await f.jobs.list(f.project)).length,0);
 await f.jobs.create(f.project,id,input);let ready=await f.jobs.refresh(f.project,id);ready=await f.jobs.save(f.project,id,ready.outputs[0].id,"Ground");const previousPath=ready.outputs[0].savedPath;ready.outputs[0].savedPath=null;
 const scope=(await readdir(path.join(f.data,"texture-jobs")))[0];await writeFile(path.join(f.data,"texture-jobs",scope,id,"job.json"),JSON.stringify(ready));const recovered=await f.jobs.save(f.project,id,ready.outputs[0].id,"Changed name");assert.equal(recovered.outputs[0].savedPath,previousPath);assert.equal(recovered.outputs[0].saveName,"Ground");
});
test("fal texture transport uses the documented schema and blocks duplicate paid posts and unsafe URLs",async()=>{
 let paid=0,downloaded=0,submitted:Record<string,unknown>={},host="https://fal.media/image.png";
 const transport:typeof fetch=async(input,init)=>{const url=String(input);if(url.startsWith("https://fal.media/")){downloaded++;return new Response(new Uint8Array([1,2,3]));}if(url.includes("/status"))return Response.json({status:"COMPLETED"});if(init?.method?.toUpperCase()==="POST"){paid++;submitted=JSON.parse(String(init.body));return Response.json({request_id:"test-request",status:"IN_QUEUE"});}return Response.json({images:[{url:host,width:1024,height:1024,content_type:"image/png"}],has_nsfw_concepts:[false],seed:42,prompt:"moss"});};
 const provider=createTextureProvider("fake-key",transport);assert.equal(await provider.submit("moss",42,2),"test-request");assert.equal(submitted.num_images,2);assert.equal(submitted.output_format,"png");assert.equal(submitted.image_size,"square_hd");assert.equal(await provider.status("test-request"),"completed");assert.equal((await provider.result("test-request")).length,1);await assert.rejects(provider.submit("again",42,1));assert.equal(paid,1);host="http://127.0.0.1/secret";await assert.rejects(provider.result("test-request"),/unsupported/);assert.equal(downloaded,1);
});
test("texture generation inputs are bounded and prompts remain plain text",()=>{assert.equal(textureRequest(input).count,2);assert.equal(TEXTURE_ENDPOINT,"fal-ai/flux/schnell");for(const invalid of [{name:""},{prompt:"x"},{prompt:"x".repeat(2001)},{count:5},{seed:-1},{tileFriendly:"yes"}])assert.throws(()=>textureRequest({...input,...invalid}));});

test("discarding completed candidates removes only temporary files, preserves saved copies and submission claims",async t=>{
 const f=await fixture(t),id=randomUUID();await f.jobs.create(f.project,id,input);await assert.rejects(f.jobs.discard(f.project,id,"anything"),/completed/);
 const ready=await f.jobs.refresh(f.project,id),first=ready.outputs[0],second=ready.outputs[1],file=await f.jobs.artifact(f.project,id,first.id);
 const saved=await f.jobs.save(f.project,id,first.id,"Keep"),savedPath=saved.outputs[0].savedPath!;
 assert.equal((await f.jobs.discard(f.project,id,first.id)).outputs.length,1);await assert.rejects(readFile(file),{code:"ENOENT"});assert.equal((await readFile(path.join(f.root,savedPath))).toString(),"png-one");
 await assert.rejects(f.jobs.artifact(f.project,id,first.id));assert.equal((await readFile(await f.jobs.artifact(f.project,id,second.id))).toString(),"png-two");
 await assert.rejects(f.jobs.discard({...f.project,id:"other-project"},id,second.id),/not found/);await assert.rejects(f.jobs.discard(f.project,"../escape",second.id),/Invalid/);
 await rm(await f.jobs.artifact(f.project,id,second.id));await f.jobs.discard(f.project,id,second.id);await f.jobs.discard(f.project,id,second.id);assert.equal((await f.jobs.list(f.project)).length,0);
 await f.jobs.refresh(f.project,id);await f.jobs.create(f.project,id,input);assert.equal(f.submissions(),1);assert.equal((await f.jobs.get(f.project,id)).outputs.length,0);
});


test("video generation resolves the clicked unsaved sprite candidate within its owning project",async t=>{
 const f=await fixture(t),imageId=randomUUID();await f.jobs.create(f.project,imageId,input);const ready=await f.jobs.refresh(f.project,imageId),selected=ready.outputs[1];let uploads=0,videos=0;
 const provider:ParticleMediaProvider={upload:async bytes=>{uploads++;assert.equal(bytes.toString(),"png-two");return "https://fal.media/candidate.png";},submit:async()=>{videos++;return "video-request";},status:async()=>"queued",result:async()=>Buffer.alloc(0)};
 const media=new ParticleMediaJobs(f.data,async()=>provider),id=randomUUID(),request={stage:"animate",source:`texture-job:${imageId}:${selected.id}`,prompt:"Leaves tumbling gently",seed:42};
 const queued=await media.create(f.project,id,request);assert.equal(queued.source,request.source);assert.equal(queued.state,"queued");await media.create(f.project,id,request);assert.equal(videos,1);assert.equal(uploads,1);assert.equal((await f.jobs.get(f.project,imageId)).outputs[1].savedPath,null);assert.equal(f.saves(),0);
 await assert.rejects(media.create({...f.project,id:"other-project"},randomUUID(),request));await assert.rejects(media.create(f.project,randomUUID(),{...request,source:"texture-job:../../escape"}));assert.equal(videos,1);assert.equal(uploads,1);
 await f.jobs.discard(f.project,imageId,selected.id);await assert.rejects(media.create(f.project,randomUUID(),request));assert.equal(videos,1);assert.equal(uploads,1);
});


test("library image deletion allows its own provenance, blocks consumers, and clears saved candidate state",async t=>{
 const f=await fixture(t),id=randomUUID();await f.jobs.create(f.project,id,input);const ready=await f.jobs.refresh(f.project,id),resultId=ready.outputs[0].id,saved=await f.jobs.save(f.project,id,resultId,"Library tile"),image=saved.outputs[0].savedPath!,descriptor=path.posix.join(path.posix.dirname(image),"texture.asset.json");
 f.project.files=[{path:image,name:"Library tile.png",modifiedAt:"2026-10-29"},{path:descriptor,name:"texture.asset.json",content:await readFile(path.join(f.root,descriptor),"utf8"),modifiedAt:"2026-10-29"}] as ProjectSnapshot["files"];
 const provenance={source:descriptor,target:image,kind:"asset-reference",label:"references path"} as ProjectSnapshot["connections"][number];
 f.project.connections=[provenance,{...provenance,source:"assets/used.material.toml"}];await assert.rejects(deleteLibraryImage(f.project,image),/referenced/);assert.ok(await readFile(path.join(f.root,image)));
 f.project.connections=[provenance,{...provenance,source:"assets/used.entity.toml",target:descriptor}];await assert.rejects(deleteLibraryImage(f.project,image),/metadata is still referenced/);
 f.project.connections=[provenance];await assert.rejects(deleteLibraryImage(f.project,"../escape.png"),/library/);await assert.rejects(deleteLibraryImage(f.project,descriptor),/library/);
 await deleteLibraryImage(f.project,image);await f.jobs.forgetSavedImage(f.project,image);await assert.rejects(readFile(path.join(f.root,image)),{code:"ENOENT"});assert.ok(await readFile(path.join(f.root,descriptor)));assert.equal((await f.jobs.get(f.project,id)).outputs[0].savedPath,null);assert.ok(await readFile(await f.jobs.artifact(f.project,id,resultId)));
 const restored=await f.jobs.save(f.project,id,resultId,"Library tile");assert.equal(restored.outputs[0].savedPath,image);assert.equal((await readFile(path.join(f.root,image))).toString(),"png-one");assert.equal(f.submissions(),1);
});

test("chosen image models survive restart and retain their actual submitted settings in provenance",async t=>{
 const f=await fixture(t),endpoint="fal-ai/test-selected/image",settings={prompt:"model prompt",num_images:2},seen:string[]=[];
 t.mock.method(falModels,"input",async()=>settings);
 f.provider.submit=async(_prompt,_seed,_count,model,actual)=>{seen.push(model!);assert.deepEqual(actual,settings);return "chosen-request";};
 f.provider.status=async(_id,model)=>{seen.push(model!);return "completed";};f.provider.result=async(_id,model)=>{seen.push(model!);return [Buffer.from("png-one"),Buffer.from("png-two")];};
 const id=randomUUID(),created=await f.jobs.create(f.project,id,{...input,model:endpoint});assert.equal(created.endpoint,endpoint);
 await assert.rejects(f.jobs.create(f.project,id,{...input,model:"fal-ai/another/image"}),/different prompt/);
 const restarted=new TextureJobs(f.data,async()=>f.provider,f.validate,f.saveImage),ready=await restarted.refresh(f.project,id),saved=await restarted.save(f.project,id,ready.outputs[0].id,"Chosen");assert.deepEqual(seen,[endpoint,endpoint,endpoint]);
 const descriptor=JSON.parse(await readFile(path.join(f.root,path.dirname(saved.outputs[0].savedPath!),"texture.asset.json"),"utf8"));assert.equal(descriptor.endpoint,endpoint);assert.deepEqual(descriptor.settings,settings);
});

test("skybox requests persist a 2:1 generation size and resume the same paid submission",async t=>{
 const f=await fixture(t),id=randomUUID();let submitted:Record<string,unknown>|undefined;
 f.provider.submit=async(_prompt,_seed,_count,_endpoint,settings)=>{submitted=settings;return "skybox-request";};
 const request={...input,kind:"skybox",tileFriendly:false};const job=await f.jobs.create(f.project,id,request);assert.equal(job.input.kind,"skybox");assert.deepEqual(submitted?.image_size,{width:1024,height:512});assert.match(job.submittedPrompt,/equirectangular/);assert.equal((await f.jobs.create(f.project,id,request)).requestId,"skybox-request");
});
