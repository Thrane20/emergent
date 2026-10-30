import "server-only";
import {createHash,randomUUID} from "node:crypto";
import {isDeepStrictEqual} from "node:util";
import {readFile} from "node:fs/promises";
import {execFile} from "node:child_process";
import path from "node:path";
import {parse,stringify} from "smol-toml";
import {containedFile} from "./object-files";
import {readProjectBinding,type ProjectSnapshot} from "./project";
import {raymarchedSettings,type RaymarchedSettings} from "./raymarched-terrain";
import {foundationRoot} from "./project-code-context";
import {projectScenes} from "./project-scenes";
import {prepareSceneAssetPlacement} from "./scene-asset-placement";
import {applyChanges,checkCandidate,type FileChange} from "./statement-files";
const revision=(source:string)=>createHash("sha256").update(source).digest("hex");
export function raymarchedLibrary(project:ProjectSnapshot){return project.files.filter(f=>f.path.endsWith(".entity.toml")&&f.content&&/^kind\s*=\s*["']raymarched-terrain["']/m.test(f.content)).map(f=>({path:f.path,name:f.displayName??f.name.replace(/\.entity\.toml$/,""),version:revision(f.content!)}));}
export async function readRaymarchedAsset(project:ProjectSnapshot,assetPath:unknown){
 if(typeof assetPath!=="string"||!assetPath.endsWith(".entity.toml")||!project.assetRoots.some(root=>assetPath.startsWith(root+"/")))throw new Error("Choose a project ray-marched terrain asset.");
 const source=await readFile(await containedFile(project.root,assetPath,256*1024),"utf8"),asset=parse(source);
 if(asset.schema_version!==1||asset.kind!=="raymarched-terrain")throw new Error("Choose a ray-marched terrain asset.");
 return {assetPath,source,asset,settings:raymarchedSettings(asset.raymarched_terrain),name:typeof asset.name==="string"?asset.name:"Ray-marched terrain",revision:revision(source)};
}
export async function prepareRaymarchedSave(project:ProjectSnapshot,input:{settings:unknown;name:unknown;assetPath?:unknown;revision?:unknown;scenePath?:unknown;sceneRevision?:unknown;action?:unknown}){
 if(input.action!==undefined&&!['save','place'].includes(String(input.action)))throw new Error("Choose save or place.");
 const current=input.assetPath===undefined?undefined:await readRaymarchedAsset(project,input.assetPath);
 if(current&&current.revision!==input.revision)throw new Error("Terrain changed externally. Reload before saving or placing.");
 const settings=input.action==='place'&&current?current.settings:raymarchedSettings(input.settings);
 const name=input.action==='place'&&current?current.name:input.name;
 if(typeof name!=="string"||!name.trim()||name.trim().length>64||/[\p{Cc}\p{Cf}]/u.test(name))throw new Error("Use a terrain name of 1–64 printable characters.");
 const root=project.assetRoots[0];if(!root)throw new Error("Project has no asset root.");
 const id=current?.asset.id??randomUUID(),assetPath=current?.assetPath??`${root}/terrains/${id}.entity.toml`;
 let content=current&&input.action==='place'?current.source:stringify({...current?.asset,schema_version:1,id,name:name.trim(),kind:"raymarched-terrain",raymarched_terrain:settings});
 if(current&&input.action!=='place'){
  const header=/^\[raymarched_terrain\][ \t]*(?:#[^\r\n]*)?\r?$/m.exec(current.source);if(!header)throw new Error("Use a [raymarched_terrain] table before editing this asset.");
  const rest=current.source.slice(header.index+header[0].length),next=rest.search(/^\s*\[/m),end=next<0?current.source.length:header.index+header[0].length+next;
  let preserved=current.source.slice(0,header.index)+stringify({raymarched_terrain:settings})+"\n"+current.source.slice(end);
  const first=preserved.search(/^\[/m),top=preserved.slice(0,first),tail=preserved.slice(first);
  preserved=(/^name\s*=/m.test(top)?top.replace(/^name\s*=.*$/m,`name = ${JSON.stringify(name.trim())}`):top+`name = ${JSON.stringify(name.trim())}\n`)+tail;
  if(!isDeepStrictEqual(parse(preserved),parse(content)))throw new Error("Could not safely update terrain settings. Use the TOML editor.");content=preserved;
 }
 const changes:FileChange[]=[{path:assetPath,before:current?.source??null,content}];
 let placement:Awaited<ReturnType<typeof prepareSceneAssetPlacement>>|undefined;
 if(input.scenePath!==undefined){placement=await prepareSceneAssetPlacement(project,{assetPath,scenePath:input.scenePath,sceneRevision:input.sceneRevision});changes.push(placement.change);}
 if(input.action==='place'&&(!current||!placement))throw new Error("Choose a saved terrain and destination scene.");
 return {changes,assetPath,placement};
}
const state=globalThis as typeof globalThis&{emergentScenePlacementActive?:boolean;emergentRaymarchedQueue?:{tail:Promise<unknown>;pending:number}};
export async function saveRaymarched(project:ProjectSnapshot,input:Parameters<typeof prepareRaymarchedSave>[1]){
 if(state.emergentScenePlacementActive)throw new Error("Another scene edit is saving.");state.emergentScenePlacementActive=true;
 try{const prepared=await prepareRaymarchedSave(project,input),token=randomUUID(),probe=`${project.assetRoots[0]}/terrains/${token}.scene.toml`,scenes=projectScenes(project).map(s=>s.path);
 if(scenes.length>126)throw new Error("Terrain validation supports at most 126 project scenes.");
 await checkCandidate(project,[...prepared.changes,{path:probe,before:null,content:stringify({schema_version:1,id:token,entities:[{id:token,asset:prepared.assetPath}]})}],{root:await foundationRoot(project)},[...scenes,probe]);
 const binding=await readProjectBinding();if(binding.id!==project.id||binding.root!==project.root)throw new Error("Linked project changed. Nothing saved.");
 await applyChanges(project,prepared.changes);const {source,asset,...saved}=await readRaymarchedAsset(project,prepared.assetPath);void source;void asset;
 return {...saved,id:prepared.placement?.instanceId,scenePath:prepared.placement?.change.path};
 }finally{state.emergentScenePlacementActive=false;}
}
export async function previewRaymarched(project:ProjectSnapshot,settings:RaymarchedSettings,yaw=35,pitch=30){
 settings=raymarchedSettings(settings);if(!Number.isFinite(yaw)||Math.abs(yaw)>360||!Number.isFinite(pitch)||pitch<5||pitch>85)throw new Error("Invalid preview camera.");
 const queue=state.emergentRaymarchedQueue??={tail:Promise.resolve(),pending:0};if(queue.pending>=24)throw new Error("Terrain preview queue is busy.");queue.pending++;
 const task=queue.tail.catch(()=>{}).then(async()=>{const root=await foundationRoot(project);return new Promise<Buffer>((resolve,reject)=>{
const child=execFile("cargo",["run","--quiet","--release","--manifest-path",path.join(root,"Cargo.toml"),"--example","raymarched_terrain_preview","--features","graphics"],{cwd:root,encoding:"buffer",timeout:120000,maxBuffer:4*1024*1024,windowsHide:true},(error,stdout,stderr)=>{if(error)reject(new Error(stderr.toString().slice(-3000)||error.message));else if(!stdout.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])))reject(new Error("Invalid native terrain preview."));else resolve(stdout);});
 child.stdin?.on("error",()=>{});child.stdin?.end(stringify({settings,yaw,pitch}));});});queue.tail=task;
 try{return await task;}finally{queue.pending--;}
}
