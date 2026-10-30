import "server-only";
import {prepareSceneAssetPlacement} from "./scene-asset-placement";
import {createHash,randomUUID} from "node:crypto";
import {readFile,mkdtemp,rm,stat} from "node:fs/promises";
import {execFile} from "node:child_process";
import os from "node:os";
import path from "node:path";
import {parse,stringify} from "smol-toml";
import {containedFile} from "./object-files";
import {particleAsset,particleDefinition} from "./particles";
import {readProjectBinding,type ProjectSnapshot} from "./project";
import {foundationRoot} from "./project-code-context";
import {applyChanges,checkCandidate,type FileChange} from "./statement-files";
import {projectScenes} from "./project-scenes";
const hash=(source:string)=>createHash("sha256").update(source).digest("hex");
async function source(project:ProjectSnapshot,p:string,limit=128*1024){if(!project.assetRoots.some(root=>p.startsWith(`${root}/`)))throw new Error("Choose a project asset.");return readFile(await containedFile(project.root,p,limit),"utf8");}
export async function readParticleAsset(project:ProjectSnapshot,assetPath:string){
 if(!assetPath.endsWith(".entity.toml"))throw new Error("Choose a particle entity asset.");const entitySource=await source(project,assetPath),entity=parse(entitySource);
 if(entity.kind!=="particle-effect"||typeof entity.particles!=="string"||!entity.particles.endsWith(".particle.toml"))throw new Error("Choose a Particle Generator effect.");
 const effectPath=entity.particles,effectSource=await source(project,effectPath),definition=particleAsset(effectSource);
 return {assetPath,effectPath,definition,revision:hash(JSON.stringify([entitySource,effectSource])),entity,entitySource,effectSource};
}
export function particleLibrary(project:ProjectSnapshot){return project.files.filter(f=>f.path.endsWith(".entity.toml")&&f.content&&/^kind\s*=\s*["']particle-effect["']/m.test(f.content)).map(f=>({path:f.path,name:f.displayName??f.name,version:f.modifiedAt}));}
export async function prepareParticleSave(project:ProjectSnapshot,input:{definition:unknown;assetPath?:unknown;revision?:unknown;scenePath?:unknown;sceneRevision?:unknown}){
 const definition=particleDefinition(input.definition),current=typeof input.assetPath==="string"?await readParticleAsset(project,input.assetPath):undefined;
 if(input.assetPath!==undefined&&typeof input.assetPath!=="string")throw new Error("Invalid particle asset path.");
 if(current&&input.revision!==current.revision)throw new Error("Effect changed externally. Reopen it before saving.");
 const root=project.assetRoots[0];if(!root)throw new Error("Project has no asset root.");
 const id=current?.definition.id??randomUUID(),assetPath=current?.assetPath??`${root}/particles/${id}.entity.toml`,effectPath=current?.effectPath??`${root}/particles/${id}.particle.toml`;
 const effectSource=stringify({...definition,id}),entitySource=stringify({...current?.entity,schema_version:1,id:current?.entity.id??id,name:definition.name,kind:"particle-effect",particles:effectPath});
 const changes:FileChange[]=[{path:effectPath,before:current?.effectSource??null,content:effectSource},{path:assetPath,before:current?.entitySource??null,content:entitySource}];
 let instanceId:string|undefined;
 if(input.scenePath!==undefined){const placed=await prepareParticlePlacement(project,{assetPath,scenePath:input.scenePath,sceneRevision:input.sceneRevision});instanceId=placed.instanceId;changes.push(placed.change);}

 return {changes,result:{assetPath,effectPath,instanceId,definition:{...definition,id},revision:hash(JSON.stringify([entitySource,effectSource]))}};
}
export const prepareParticlePlacement=prepareSceneAssetPlacement;

export async function placeParticleAsset(project:ProjectSnapshot,input:{assetPath:unknown;scenePath:unknown;sceneRevision:unknown}) {
 if(state.emergentScenePlacementActive)throw new Error("Another scene edit is saving.");state.emergentScenePlacementActive=true;
 try{
 if(typeof input.assetPath!=="string")throw new Error("Choose a saved effect.");
 const asset=await readParticleAsset(project,input.assetPath),stamp=await spriteRevision(project,asset.definition),{change,instanceId}=await prepareParticlePlacement(project,{...input,assetPath:asset.assetPath});
 await checkCandidate(project,[change],{root:await foundationRoot(project)},[change.path]);
 if((await readParticleAsset(project,asset.assetPath)).revision!==asset.revision||await spriteRevision(project,asset.definition)!==stamp)throw new Error("Saved effect changed during placement. Reload the library before adding it.");
 const binding=await readProjectBinding();if(binding.id!==project.id||binding.root!==project.root)throw new Error("Linked project changed. Nothing placed.");
 await applyChanges(project,[change]);return {instanceId,sceneRevision:hash(change.content),assetPath:asset.assetPath,name:asset.definition.name};
 }finally{state.emergentScenePlacementActive=false;}
}
async function spriteRevision(project:ProjectSnapshot,definition:ReturnType<typeof particleDefinition>) {
 const hash=createHash("sha256");
 for(const asset of [...new Set(definition.emitters.flatMap(e=>e.texture?[e.texture]:[]))].sort()) {
 if(!project.assetRoots.some(root=>asset.startsWith(`${root}/`)))throw new Error("Sprites must be in a declared asset root.");
 hash.update(asset).update(await readFile(await containedFile(project.root,asset,16*1024*1024)));
 }return hash.digest("hex");
}
const state=globalThis as typeof globalThis&{emergentScenePlacementActive?:boolean};
export async function saveParticleAsset(project:ProjectSnapshot,input:Parameters<typeof prepareParticleSave>[1]){
 if(state.emergentScenePlacementActive)throw new Error("Another scene edit is saving.");state.emergentScenePlacementActive=true;
 try{const {changes,result}=await prepareParticleSave(project,input),stamp=await spriteRevision(project,result.definition),id=randomUUID(),probe=`${project.assetRoots[0]}/particles/${id}.scene.toml`;
 if(projectScenes(project).length>126)throw new Error("Particle saving supports at most 126 scenes.");
 await checkCandidate(project,[...changes,{path:probe,before:null,content:stringify({schema_version:1,id,entities:[{id,asset:result.assetPath}]})}],{root:await foundationRoot(project)},[probe,...projectScenes(project).map(s=>s.path)]);
 if(await spriteRevision(project,result.definition)!==stamp)throw new Error("A sprite changed during validation. Save again after reviewing it.");
 const binding=await readProjectBinding();if(binding.id!==project.id||binding.root!==project.root)throw new Error("Linked project changed. Nothing saved.");await applyChanges(project,changes);return result;
 }finally{state.emergentScenePlacementActive=false;}
}
const captures=globalThis as typeof globalThis&{emergentParticleCaptures?:{tail:Promise<unknown>;pending:number}};
const captureQueue=captures.emergentParticleCaptures??={tail:Promise.resolve(),pending:0};
export async function captureParticles(project:ProjectSnapshot,value:unknown,seconds:unknown,thumbnail=false){
 const definition=particleDefinition(value);
 if(typeof seconds!=="number"||!Number.isFinite(seconds)||seconds<0||seconds>30)throw new Error("Capture time must be 0–30 seconds.");
 if(captureQueue.pending>=24)throw new Error("Effect preview queue is busy. Try again shortly.");
 captureQueue.pending++;
 const result=captureQueue.tail.catch(()=>{}).then(()=>renderParticles(project,definition,seconds,thumbnail));captureQueue.tail=result;
 try{return await result;}finally{captureQueue.pending--;}
}
async function renderParticles(project:ProjectSnapshot,value:unknown,seconds:unknown,thumbnail:boolean){
if(typeof seconds!=="number"||!Number.isFinite(seconds)||seconds<0||seconds>30)throw new Error("Capture time must be 0–30 seconds.");const definition=particleDefinition(value);const folder=await mkdtemp(path.join(os.tmpdir(),"emergent-particle-preview-"));
 try{const root=await foundationRoot(project),output=path.join(folder,"preview.png");await new Promise<void>((resolve,reject)=>{const child=execFile("cargo",["run","--quiet","--manifest-path",path.join(root,"Cargo.toml"),"--features","graphics","--example","particle_tool","--",project.root,output,String(seconds),...(thumbnail?["--thumbnail"]:[])],{cwd:root,shell:false,timeout:120000,maxBuffer:32768},(error,_stdout,stderr)=>error?reject(new Error(stderr.slice(-4000)||error.message)):resolve());child.stdin?.on("error",()=>{});child.stdin?.end(stringify(definition));});if((await stat(output)).size>16*1024*1024)throw new Error("Capture exceeds size limit.");return readFile(output);
 }finally{await rm(folder,{recursive:true,force:true});}
}
export async function particleSceneRevision(project:ProjectSnapshot,scenePath:string){return hash(await source(project,scenePath,256*1024));}
