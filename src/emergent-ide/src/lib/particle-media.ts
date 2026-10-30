import "server-only";
import {falModels} from "./fal-models";
import {createHash,randomUUID} from "node:crypto";
import {mkdir,readFile,readdir,realpath,unlink,writeFile} from "node:fs/promises";
import path from "node:path";
import {execFile} from "node:child_process";
import {promisify} from "node:util";
import {createFalClient} from "@fal-ai/client";
import {objectApiKey} from "./object-settings";
import {singleSubmissionFetch} from "./object-provider";
import {containedFile,ideDataDirectory,missing,writeJson,boundedBody} from "./object-files";
import {projectImages,DEFAULT_SPRITE_SHEET} from "./image-library";
import {configureLibraryImage} from "./image-library-assets";
import {TextureJobs} from "./texture-jobs";
import {importTexture} from "./texture-assets";
import {type ProjectSnapshot} from "./project";
import {PARTICLE_VIDEO_ENDPOINT,PARTICLE_MATTE_ENDPOINT,particleMediaRequest,particleVideoInput,particleMatteInput,type ParticleMediaJob} from "./particle-media-types";
const execute=promisify(execFile),uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/;
export interface ParticleMediaProvider{upload(bytes:Buffer,type:string):Promise<string>;submit(endpoint:string,input:Record<string,unknown>):Promise<string>;status(endpoint:string,id:string):Promise<"queued"|"generating"|"ready">;result(endpoint:string,id:string):Promise<Buffer>}
export async function falParticleMediaProvider():Promise<ParticleMediaProvider>{const key=await objectApiKey();if(!key)throw new Error("Configure the saved fal key in Settings.");return createParticleMediaProvider(key);}
export function createParticleMediaProvider(key:string,transport:typeof fetch=fetch):ParticleMediaProvider{const client=createFalClient({credentials:key,retry:{maxRetries:0},fetch:singleSubmissionFetch(transport)});return {
 async upload(bytes,type){return client.storage.upload(new Blob([new Uint8Array(bytes)],{type}));},
 async submit(endpoint,input){const r=await client.queue.submit(endpoint,{input,abortSignal:AbortSignal.timeout(30000)});if(!r.request_id)throw new Error("fal returned no request ID.");return r.request_id;},
 async status(endpoint,requestId){const r=await client.queue.status(endpoint,{requestId,logs:false,abortSignal:AbortSignal.timeout(15000)});return r.status==="COMPLETED"?"ready":r.status==="IN_QUEUE"?"queued":"generating";},
 async result(endpoint,requestId){const r=await client.queue.result(endpoint,{requestId,abortSignal:AbortSignal.timeout(15000)}),data=r.data as {video?:{url?:string};warning?:string};if(data.warning)throw new Error(`fal output warning: ${data.warning}`);let url:URL;try{url=new URL(data.video?.url??"");}catch{throw new Error("fal returned no video.");}if(url.protocol!=="https:"||url.username||url.password||(url.port&&url.port!=="443")||!(url.hostname==="fal.media"||url.hostname.endsWith(".fal.media")))throw new Error("Unsupported fal download host.");const response=await transport(url,{redirect:"error",signal:AbortSignal.timeout(90000)});if(!response.ok)throw new Error("Video download failed. Refresh to resume.");return boundedBody(response,64*1024*1024);}
 };}
function videoSignature(bytes:Buffer){if(!bytes.subarray(0,4).equals(Buffer.from([0x1a,0x45,0xdf,0xa3]))&&bytes.toString("ascii",4,8)!=="ftyp")throw new Error("Choose an MP4/MOV or WebM container.");}
export async function probeParticleVideo(file:string){videoSignature(await readFile(file));const {stdout}=await execute("ffprobe",["-v","error","-protocol_whitelist","file,pipe","-show_streams","-show_format","-of","json",file],{timeout:15000,maxBuffer:128*1024,shell:false});const metadata=JSON.parse(stdout),stream=metadata.streams?.find((s:{codec_type:string})=>s.codec_type==="video");const seconds=Number(metadata.format?.duration??stream?.duration);if(!stream||!Number.isFinite(seconds)||seconds<=0||seconds>30||!Number.isInteger(stream.width)||!Number.isInteger(stream.height)||stream.width<1||stream.height<1||stream.width>2048||stream.height>2048)throw new Error("Use a video up to 30 seconds and 2048 pixels per axis.");return {seconds,codec:String(stream.codec_name),width:stream.width,height:stream.height};}
export async function bakeParticleVideo(input:string,output:string,requireAlpha:boolean){
 const meta=await probeParticleVideo(input),decoder=meta.codec==="vp9"?["-c:v","libvpx-vp9"]:[];
 if(requireAlpha){
 const {stdout:sample}=await execute("ffmpeg",["-v","error","-nostdin",...decoder,"-protocol_whitelist","file,pipe","-i",input,"-an","-vf","fps=2,scale=64:64,format=rgba","-frames:v","8","-f","rawvideo","-pix_fmt","rgba","pipe:1"],{timeout:15000,maxBuffer:256*1024,encoding:"buffer",shell:false});
 let transparent=0,visible=0,partial=0;for(let i=3;i<sample.length;i+=4){const a=sample[i];if(a<250)transparent++;if(a>0)visible++;if(a>0&&a<250)partial++;}
 if(transparent<10||visible<10||partial<10)throw new Error("Video has no usable soft alpha. Remove its background first, or bake for additive fire only.");
 }
 // The clip is sampled at 8 fps; short clips hold their last frame, with no claim of a seamless loop.
 await execute("ffmpeg",["-v","error","-nostdin","-threads","1",...decoder,"-protocol_whitelist","file,pipe","-i",input,"-an","-vf","fps=8,scale=252:252:force_original_aspect_ratio=decrease,format=rgba,pad=256:256:(ow-iw)/2:(oh-ih)/2:color=black@0,tpad=stop_mode=clone:stop_duration=4,trim=duration=4,tile=8x4:nb_frames=32","-frames:v","1","-threads","1","-y",output],{timeout:60000,maxBuffer:128*1024,shell:false});
 const {stdout:pixels}=await execute("ffmpeg",["-v","error","-nostdin","-i",output,"-frames:v","1","-f","rawvideo","-pix_fmt","rgba","pipe:1"],{timeout:15000,maxBuffer:16*1024*1024,encoding:"buffer",shell:false});
 if(pixels.length!==2048*1024*4)throw new Error("Unexpected atlas size.");let transparent=0,visible=0,partial=0;
 // Ignore padding: transparent borders alone must never qualify an opaque clip as smoke alpha.
 for(let cell=0;cell<32;cell++)for(let y=8;y<248;y++)for(let x=8;x<248;x++){const a=pixels[(((Math.floor(cell/8)*256+y)*2048)+(cell%8*256+x))*4+3];if(a<250)transparent++;if(a>0)visible++;if(a>0&&a<250)partial++;}
 const alpha=transparent>100&&visible>100;
 if(requireAlpha&&(!alpha||partial<100))throw new Error("Video has no usable soft alpha. Remove its background first, or bake for additive fire only.");
 return {columns:8,rows:4,frames:32,fps:8,alpha:requireAlpha&&alpha};
}
export class ParticleMediaJobs{
 private locks=new Map<string,Promise<unknown>>();
 constructor(private data=ideDataDirectory(),private provider:()=>Promise<ParticleMediaProvider>=falParticleMediaProvider,private bake=bakeParticleVideo,private save=importTexture){}
 private async folder(p:ProjectSnapshot,id?:string){if(id!==undefined&&!uuid.test(id))throw new Error("Invalid particle media job ID.");const scope=createHash("sha256").update(`${p.id}\0${await realpath(p.root)}`).digest("hex");return path.join(this.data,"particle-media",scope,...(id?[id]:[]));}
 private async serial<T>(key:string,fn:()=>Promise<T>){const previous=this.locks.get(key)??Promise.resolve(),next=previous.catch(()=>{}).then(fn);this.locks.set(key,next);try{return await next;}finally{if(this.locks.get(key)===next)this.locks.delete(key);}}
 async get(p:ProjectSnapshot,id:string):Promise<ParticleMediaJob>{const j=JSON.parse(await readFile(path.join(await this.folder(p,id),"job.json"),"utf8"));if(j.id!==id||j.projectId!==p.id||j.schema_version!==1)throw new Error("Invalid media journal.");return j;}
 private async persist(p:ProjectSnapshot,j:ParticleMediaJob){await writeJson(path.join(await this.folder(p,j.id),"job.json"),j);return j;}
 async list(p:ProjectSnapshot){let ids:string[];try{ids=await readdir(await this.folder(p));}catch(e){if(missing(e))return [];throw e;}return (await Promise.all(ids.filter(id=>uuid.test(id)).map(async id=>{try{return await this.get(p,id);}catch(e){if(missing(e))return null;throw e;}}))).filter((job):job is ParticleMediaJob=>job!==null&&!job.deleted).sort((a,b)=>b.createdAt.localeCompare(a.createdAt)).slice(0,100);}
 async create(p:ProjectSnapshot,id:string,value:unknown){const input=particleMediaRequest(value),folder=await this.folder(p,id);return this.serial(folder,async()=>{
 try{const j=await this.get(p,id);if(j.deleted)throw new Error("This result was deleted. Start a new generation.");if(j.stage!==input.stage||j.source!==input.source||j.prompt!==input.prompt||j.seed!==input.seed||j.output!==input.output||j.endpoint!==(input.stage==="animate"?input.model??PARTICLE_VIDEO_ENDPOINT:PARTICLE_MATTE_ENDPOINT))throw new Error("This job ID belongs to another request.");return j;}catch(e){if(!missing(e))throw e;}
 const endpoint=input.stage==="animate"?input.model??PARTICLE_VIDEO_ENDPOINT:PARTICLE_MATTE_ENDPOINT;
 // Validate model compatibility before uploading or claiming a paid submission.
 if(input.stage==="animate"&&endpoint!==PARTICLE_VIDEO_ENDPOINT)await falModels.get("animation",endpoint);
 const provider=await this.provider();let bytes:Buffer,type:string,spriteFamily:string|undefined;
 if(input.output==="sprite"){
  const source=projectImages(p).find(image=>image.path===input.source&&image.kind==="sprite");if(!source)throw new Error("Choose a saved Sprite for animation.");spriteFamily=await this.serial(`${await this.folder(p)}:family:${source.path}`,async()=>{let family=source.family;try{const existing=JSON.parse(await readFile(await containedFile(p.root,`${source.path}.library.json`,8192),"utf8"));if(typeof existing.family==="string"&&uuid.test(existing.family))family=existing.family;}catch(e){if(!missing(e))throw e;}family??=randomUUID();await configureLibraryImage(p,source.path,{kind:"sprite",sprite:source.sprite??DEFAULT_SPRITE_SHEET,family});return family;});
 }
 if(input.stage==="animate"){if(input.source.startsWith("texture-job:")){
 const match=/^texture-job:([a-f0-9-]{36}):([a-f0-9-]{36})$/.exec(input.source);if(!match)throw new Error("Choose a valid generated sprite candidate.");
 const file=await new TextureJobs(this.data).artifact(p,match[1],match[2]);bytes=await readFile(file);type=/\.png$/i.test(file)?"image/png":"image/jpeg";
 }else{if(!p.assetRoots.some(r=>input.source.startsWith(`${r}/`))||!/[.](png|jpe?g)$/i.test(input.source))throw new Error("Choose a project PNG/JPEG image.");bytes=await readFile(await containedFile(p.root,input.source,16*1024*1024));type=/\.png$/i.test(input.source)?"image/png":"image/jpeg";}}
 else{const parent=await this.get(p,input.source);bytes=await readFile(await this.artifact(p,parent.id));type=parent.stage==="matte"?"video/webm":"video/mp4";}
 if(input.output==="sprite"){
  const sheet=projectImages(p).find(image=>image.path===input.source)?.sprite;
  if(sheet&&(sheet.columns>1||sheet.rows>1)){const file=await containedFile(p.root,input.source,16*1024*1024);const result=await execute("ffmpeg",["-v","error","-nostdin","-protocol_whitelist","file,pipe","-i",file,"-vf",`crop=iw/${sheet.columns}:ih/${sheet.rows}:0:0`,"-frames:v","1","-f","image2pipe","-vcodec","png","pipe:1"],{encoding:"buffer",timeout:15000,maxBuffer:16*1024*1024,shell:false});bytes=result.stdout;type="image/png";}
 }
 const url=await provider.upload(bytes,type);await mkdir(folder,{recursive:true});await writeFile(path.join(folder,"submission.claim"),"claimed",{flag:"wx"});
 const settings=input.stage==="animate"?(await falModels.input("animation",endpoint,{prompt:input.prompt,seed:input.seed,imageUrl:url})??particleVideoInput(url,input.prompt,input.seed)):particleMatteInput(url);
 const job:ParticleMediaJob={schema_version:1,id,projectId:p.id,...input,settings,endpoint,requestId:null,...(spriteFamily?{spriteFamily}:{}),state:"submitting",error:null,createdAt:new Date().toISOString()};await this.persist(p,job);
 try{job.requestId=await provider.submit(job.endpoint,settings);job.state="queued";}catch{job.state="uncertain";job.error="Submission could not be confirmed. Check fal before starting a new paid attempt. This job will not be resubmitted.";}
 return this.persist(p,job);
 });}
 async refresh(p:ProjectSnapshot,id:string){const folder=await this.folder(p,id);return this.serial(folder,async()=>{
  const j=await this.get(p,id);if(j.deleted)throw new Error("This media result was deleted.");
  if(j.state==="submitting"){j.state="uncertain";j.error="Restart interrupted submission. Check fal; no automatic resubmission.";}
  else if(j.output==="sprite"&&j.file&&(!j.atlas||j.state==="failed"||j.state==="baking")){return this.finishSprite(p,j);}
  else if(j.requestId&&j.state!=="ready")try{const provider=await this.provider(),state=await provider.status(j.endpoint,j.requestId);j.state=state;j.error=null;if(state==="ready"){const bytes=await provider.result(j.endpoint,j.requestId);videoSignature(bytes);const file=j.stage==="matte"?"source.webm":"source.mp4";await writeFile(path.join(folder,file),bytes);await probeParticleVideo(path.join(folder,file));j.file=file;}}
  catch(e){j.state="failed";j.error=e instanceof Error?e.message:"Video operation failed. Refresh to resume.";}
  if(j.output==="sprite"&&j.state==="ready"&&!j.atlas)return this.finishSprite(p,j);
  return this.persist(p,j);
 });}
 private async finishSprite(p:ProjectSnapshot,j:ParticleMediaJob){
  j.state="baking";j.error=null;await this.persist(p,j);
  try{await this.bakeAtlas(p,j,false);j.state="ready";j.error=null;}catch(e){j.state="failed";j.error=`Atlas preparation failed: ${e instanceof Error?e.message:"unknown error"}. Check animation to retry without regenerating.`;}
  return this.persist(p,j);
 }
 async artifact(p:ProjectSnapshot,id:string){const j=await this.get(p,id);if(j.deleted||j.state!=="ready"||!j.file||!["source.mp4","source.webm","source.mov"].includes(j.file))throw new Error("Video is not ready.");return containedFile(await this.folder(p,id),j.file,64*1024*1024);}
 async import(p:ProjectSnapshot,id:string,bytes:Buffer){const folder=await this.folder(p,id);return this.serial(folder,async()=>{try{await this.get(p,id);throw new Error("This media job ID is already in use. Import with a new ID.");}catch(e){if(!missing(e))throw e;}videoSignature(bytes);await mkdir(folder,{recursive:true});const file="source.mov";await writeFile(path.join(folder,file),bytes,{flag:"wx"});try{await probeParticleVideo(path.join(folder,file));}catch(e){throw new Error(`Invalid particle video: ${e instanceof Error?e.message:"probe failed"}`);}return this.persist(p,{schema_version:1,id,projectId:p.id,stage:"import",source:"uploaded-video",prompt:"",seed:0,endpoint:"local-import",requestId:null,state:"ready",error:null,createdAt:new Date().toISOString(),file});});}
 async discard(p:ProjectSnapshot,id:string){const folder=await this.folder(p,id);return this.serial(folder,async()=>{
  const j=await this.get(p,id);if(j.state!=="ready"&&j.state!=="failed")throw new Error("Only completed or failed media results can be deleted.");
  j.deleted=true;await this.persist(p,j);
  // Saved flipbooks retain their source clip and provenance for later recovery.
  if(!j.atlas)for(const file of ["source.mp4","source.webm","source.mov","atlas.png"]){await unlink(path.join(/* turbopackIgnore: true */ folder,file)).catch(e=>{if(!missing(e))throw e;});}
  return {id};
 });}
 async forgetAtlas(p:ProjectSnapshot,relative:string){for(const listed of await this.list(p))if(listed.output==="sprite"&&listed.atlas?.path===relative){const folder=await this.folder(p,listed.id);await this.serial(folder,async()=>{const job=await this.get(p,listed.id);if(job.atlas?.path===relative){job.deleted=true;await this.persist(p,job);}});}}
 async atlas(p:ProjectSnapshot,id:string,requireAlpha:boolean){const folder=await this.folder(p,id);return this.serial(folder,async()=>this.bakeAtlas(p,await this.get(p,id),requireAlpha));}
 private async bakeAtlas(p:ProjectSnapshot,j:ParticleMediaJob,requireAlpha:boolean){const folder=await this.folder(p,j.id);if(j.deleted)throw new Error("This media result was deleted.");if(j.atlas){if(j.atlas.alpha!==requireAlpha)throw new Error("This clip was already baked with a different alpha mode. Import another copy to change modes.");await configureLibraryImage(p,j.atlas.path,{kind:"sprite",sprite:{columns:j.atlas.columns,rows:j.atlas.rows,frames:j.atlas.frames,fps:j.atlas.fps},...(j.spriteFamily?{family:j.spriteFamily}:{})});return j;}const input=await containedFile(folder,j.file??"",64*1024*1024),output=path.join(folder,"atlas.png"),layout=await this.bake(input,output,requireAlpha),saved=await this.save(p,await readFile(output),j.output==="sprite"?`${path.basename(j.source,path.extname(j.source)).slice(0,40)} animation`:"particle-atlas",j.id);j.atlas={...layout,path:saved.path};await configureLibraryImage(p,saved.path,{kind:"sprite",sprite:{columns:layout.columns,rows:layout.rows,frames:layout.frames,fps:layout.fps},...(j.spriteFamily?{family:j.spriteFamily}:{})});await writeJson(path.join(p.root,path.dirname(saved.path),"particle-media.asset.json"),{...j,atlas:j.atlas,alphaMode:requireAlpha?"straight":"additive-only",loop:"unverified",sourceClipRetained:true,sourceClipLocation:"ide-job-journal",modelRevision:null});return this.persist(p,j);}

}
const state=globalThis as typeof globalThis&{emergentParticleMedia?:ParticleMediaJobs};export const particleMedia=state.emergentParticleMedia instanceof ParticleMediaJobs?state.emergentParticleMedia:(state.emergentParticleMedia=new ParticleMediaJobs());
