import "server-only";
import {createHash,randomUUID} from "node:crypto";
import {mkdir,readFile,readdir,rename,rm,writeFile} from "node:fs/promises";
import path from "node:path";
import {captureParticles,readParticleAsset} from "./particle-assets";
import {containedFile,ideDataDirectory,missing} from "./object-files";
import type {ProjectSnapshot} from "./project";
const state=globalThis as typeof globalThis&{emergentParticleThumbnails?:Map<string,Promise<Buffer>>};
const active=state.emergentParticleThumbnails??=new Map<string,Promise<Buffer>>();
export async function particleThumbnail(project:ProjectSnapshot,assetPath:string,render=captureParticles,data=ideDataDirectory()):Promise<Buffer>{
 const asset=await readParticleAsset(project,assetPath),stamp=createHash("sha256").update("particle-thumbnail-v1\0").update(asset.revision);
 for(const sprite of [...new Set(asset.definition.emitters.flatMap(e=>e.texture?[e.texture]:[]))].sort()){if(!project.assetRoots.some(root=>sprite.startsWith(`${root}/`)))throw new Error("Sprite is outside project asset roots.");stamp.update(sprite).update(await readFile(await containedFile(project.root,sprite,16*1024*1024)));}
 const revision=stamp.digest("hex"),scope=createHash("sha256").update(JSON.stringify([project.root,project.id,assetPath])).digest("hex"),folder=path.join(data,"particle-thumbnails",scope),key=`${folder}:${revision}`;
 if(active.has(key))return active.get(key)!;
 const task=(async()=>{
 try{return await readFile(await containedFile(folder,`${revision}.png`,16*1024*1024));}catch(e){if(!missing(e))throw e;}
 const burstOnly=asset.definition.emitters.every(e=>!e.looping&&e.rate===0),seconds=burstOnly?Math.min(2,Math.max(...asset.definition.emitters.map(e=>e.lifetime[1]))*.4):2;
 const image=await render(project,asset.definition,Math.max(1/60,seconds),true);if(!image.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])))throw new Error("Thumbnail renderer returned an invalid PNG.");
 await mkdir(folder,{recursive:true});const temporary=path.join(folder,`${randomUUID()}.png`);await writeFile(temporary,image);await rename(temporary,path.join(folder,`${revision}.png`));await Promise.all((await readdir(folder)).filter(file=>/^[a-f0-9]{64}\.png$/.test(file)&&file!==`${revision}.png`).map(file=>rm(path.join(folder,file),{force:true})));return image;
 })();active.set(key,task);try{return await task;}finally{if(active.get(key)===task)active.delete(key);}
}
