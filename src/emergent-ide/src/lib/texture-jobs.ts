import "server-only";
import {falModels} from "./fal-models";
import {createHash,randomUUID} from "node:crypto";
import {mkdir,readFile,readdir,realpath,unlink,writeFile} from "node:fs/promises";
import path from "node:path";
import type {ProjectSnapshot} from "./project";
import {containedFile,ideDataDirectory,missing,writeJson} from "./object-files";
import {ObjectCreatorError} from "./object-types";
import {ProviderFailure} from "./object-provider";
import {falTextureProvider,type TextureProvider} from "./texture-provider";
import {configureLibraryImage} from "./image-library-assets";
import {DEFAULT_SPRITE_SHEET} from "./image-library";
import {importTexture,validateTextureImage} from "./texture-assets";
import {TEXTURE_ENDPOINT,TEXTURE_GENERATION_SETTINGS,texturePrompt,textureRequest,type TextureJob} from "./texture-job-types";
const uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/;
const now=()=>new Date().toISOString();
export class TextureJobs{
 private locks=new Map<string,Promise<unknown>>();
 constructor(private data=ideDataDirectory(),private provider:()=>Promise<TextureProvider>=falTextureProvider,private validate=validateTextureImage,private saveImage=importTexture){}
 private async folder(project:ProjectSnapshot,id?:string){if(id!==undefined&&!uuid.test(id))throw new ObjectCreatorError("Invalid texture job ID.");const scope=createHash("sha256").update(`${project.id}\0${await realpath(project.root)}`).digest("hex");return path.join(this.data,"texture-jobs",scope,...(id?[id]:[]));}
 private async serial<T>(key:string,action:()=>Promise<T>):Promise<T>{const previous=this.locks.get(key)??Promise.resolve(),next=previous.catch(()=>{}).then(action);this.locks.set(key,next);try{return await next;}finally{if(this.locks.get(key)===next)this.locks.delete(key);}}
 async get(project:ProjectSnapshot,id:string):Promise<TextureJob>{try{const job=JSON.parse(await readFile(path.join(await this.folder(project,id),"job.json"),"utf8")) as TextureJob;if(job.schema_version!==1||job.id!==id||job.projectId!==project.id)throw new ObjectCreatorError("Invalid texture job journal.");return job;}catch(error){if(missing(error))throw new ObjectCreatorError("Texture generation was not found.",404);throw error;}}
 private async persist(project:ProjectSnapshot,job:TextureJob){job.updatedAt=now();await writeJson(path.join(await this.folder(project,job.id),"job.json"),job);return job;}
 async list(project:ProjectSnapshot){let ids:string[];try{ids=(await readdir(await this.folder(project))).filter(id=>uuid.test(id));}catch(error){if(missing(error))return [];throw error;}const jobs=(await Promise.all(ids.map(async id=>{try{return await this.get(project,id);}catch(error){if(error instanceof ObjectCreatorError&&error.status===404)return null;throw error;}}))).filter((job):job is TextureJob=>job!==null);return jobs.filter(job=>job.state!=="ready"||job.outputs.length>0).sort((a,b)=>b.createdAt.localeCompare(a.createdAt)).slice(0,100);}
 async create(project:ProjectSnapshot,id:string,value:unknown){const folder=await this.folder(project,id),input=textureRequest(value);return this.serial(folder,async()=>{
  try{const existing=await this.get(project,id);if(JSON.stringify(existing.input)!==JSON.stringify(input))throw new ObjectCreatorError("This generation ID belongs to a different prompt. Start a new attempt.",409);return existing;}catch(error){if(!(error instanceof ObjectCreatorError&&error.status===404))throw error;}
  const endpoint=input.model??TEXTURE_ENDPOINT,settings=await falModels.input("image",endpoint,{prompt:texturePrompt(input),seed:input.seed,count:input.count,panorama:input.kind==="skybox"});
  const provider=await this.provider();await mkdir(folder,{recursive:true,mode:0o700});
  try{await writeFile(path.join(folder,"submission.claim"),"claimed\n",{flag:"wx",mode:0o600});}catch{throw new ObjectCreatorError("Submission is already being recorded. Refresh history.",409);}
  const job:TextureJob={schema_version:1,id,projectId:project.id,input,submittedPrompt:texturePrompt(input),endpoint,settings:settings??{...TEXTURE_GENERATION_SETTINGS,...(input.kind==="skybox"?{image_size:{width:1024,height:512}}:{}),prompt:texturePrompt(input),seed:input.seed,num_images:input.count},state:"submitting",requestId:null,createdAt:now(),updatedAt:now(),error:null,outputs:[]};await this.persist(project,job);
  try{job.requestId=await provider.submit(job.submittedPrompt,input.seed,input.count,job.endpoint,job.settings);job.state="queued";}
  catch(error){job.state=error instanceof ProviderFailure&&error.terminal?"failed":"uncertain";job.error=job.state==="failed"?error instanceof Error?error.message:"fal rejected this request.":(error instanceof ProviderFailure?`${error.message} `:"")+"Submission could not be confirmed. Check your fal dashboard before a new paid attempt; this job will not be resubmitted.";}
  return this.persist(project,job);
 });}
 async refresh(project:ProjectSnapshot,id:string){const folder=await this.folder(project,id);return this.serial(folder,async()=>{
  const job=await this.get(project,id);
  if(job.state==="submitting"){job.state="uncertain";job.error="The IDE restarted before confirming submission. Check fal before another paid attempt.";}
  else if(job.requestId&&["queued","generating","downloading","failed"].includes(job.state)){
   try{const provider=await this.provider(),status=await provider.status(job.requestId,job.endpoint);job.error=null;
    if(status!=="completed")job.state=status;
    else{job.state="downloading";await this.persist(project,job);const images=await provider.result(job.requestId,job.endpoint);if(images.length<1||images.length>job.input.count)throw new ObjectCreatorError("Provider returned an invalid number of texture candidates.");
     const outputs:TextureJob["outputs"]=[];
     for(let index=0;index<images.length;index++){let extension:string;try{extension=await this.validate(project,images[index],job.input.kind==="skybox");}catch(error){throw new ObjectCreatorError(`Generated image validation failed: ${error instanceof Error?error.message:"invalid PNG/JPEG"}`);}const file=`result-${index}.${extension}`;await writeFile(path.join(folder,file),images[index],{mode:0o600});outputs.push({id:randomUUID(),index,file,saveName:null,savedPath:null});}job.outputs=outputs;job.state="ready";
    }
   }catch(error){if(error instanceof ObjectCreatorError||error instanceof ProviderFailure&&error.terminal)job.state="failed";job.error=error instanceof ProviderFailure||error instanceof ObjectCreatorError?error.message:"Unable to check or validate the images. Check this request again to resume.";}
  }return this.persist(project,job);
 });}
 async artifact(project:ProjectSnapshot,id:string,resultId:string){const job=await this.get(project,id),output=job.outputs.find(item=>item.id===resultId);if(job.state!=="ready"||!output)throw new ObjectCreatorError("Texture result is not ready.",404);return containedFile(await this.folder(project,id),output.file,16*1024*1024);}
 async forgetSavedImage(project:ProjectSnapshot,relative:string){
  for(const listed of await this.list(project))if(listed.outputs.some(output=>output.savedPath===relative)){
   const folder=await this.folder(project,listed.id);await this.serial(folder,async()=>{const job=await this.get(project,listed.id);for(const output of job.outputs)if(output.savedPath===relative)output.savedPath=null;await this.persist(project,job);});
  }
 }
 async discard(project:ProjectSnapshot,id:string,resultId:string){const folder=await this.folder(project,id);return this.serial(folder,async()=>{
  const job=await this.get(project,id);if(job.state!=="ready")throw new ObjectCreatorError("Only completed texture results can be deleted.",409);
  const output=job.outputs.find(o=>o.id===resultId);if(!output)return job;
  let file:string|undefined;try{file=await containedFile(folder,output.file,16*1024*1024);}catch(error){if(!missing(error))throw error;}
  job.outputs=job.outputs.filter(o=>o.id!==resultId);await this.persist(project,job);
  if(file)await unlink(file).catch(error=>{if(!missing(error))throw error;});return job;
 });}
 async save(project:ProjectSnapshot,id:string,resultId:string,name:string){const folder=await this.folder(project,id);return this.serial(folder,async()=>{
  const job=await this.get(project,id),output=job.outputs.find(item=>item.id===resultId);if(job.state!=="ready"||!output||!job.requestId)throw new ObjectCreatorError("Choose a completed texture candidate.");if(output.savedPath)return job;
  const normalized=textureRequest({...job.input,name}).name;
  output.saveName??=normalized;await this.persist(project,job);
  const bytes=await readFile(await this.artifact(project,id,resultId)),saved=await this.saveImage(project,bytes,output.saveName,output.id);
  const descriptorPath=path.join(project.root,path.dirname(saved.path),"texture.asset.json");
  try{const existing=JSON.parse(await readFile(descriptorPath,"utf8"));if(existing.jobId!==job.id||existing.id!==output.id)throw new ObjectCreatorError("This texture identity already has different provenance.",409);}catch(error){if(!missing(error))throw error;}
  await writeJson(descriptorPath,{schema_version:1,kind:"generated-texture",id:output.id,jobId:job.id,name:output.saveName,image:saved.path,provider:"fal",endpoint:job.endpoint,requestId:job.requestId,prompt:job.submittedPrompt,inputPrompt:job.input.prompt,seed:job.input.seed,candidateIndex:output.index,settings:job.settings??{...TEXTURE_GENERATION_SETTINGS,num_images:job.input.count},modelRevision:null,createdAt:job.createdAt,savedAt:now()});
  if(job.input.kind==="sprite"||job.input.kind==="skybox")await configureLibraryImage(project,saved.path,{kind:job.input.kind,sprite:DEFAULT_SPRITE_SHEET});
  output.savedPath=saved.path;return this.persist(project,job);
 });}
}
const state=globalThis as typeof globalThis&{emergentTextureJobs?:TextureJobs};export const textureJobs=state.emergentTextureJobs instanceof TextureJobs?state.emergentTextureJobs:(state.emergentTextureJobs=new TextureJobs());
