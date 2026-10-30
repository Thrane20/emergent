import "server-only";
import {execFile} from "node:child_process";
import {mkdtemp,writeFile,rm} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {promisify} from "node:util";
import {stringify} from "smol-toml";
import type {ProjectSnapshot} from "./project";
import {foundationRoot} from "./project-code-context";
import {decodeTerrainFields,type TerrainSettings} from "./terrain";
const execute=promisify(execFile);
const state=globalThis as typeof globalThis&{emergentTerrainQueue?:{tail:Promise<unknown>;pending:number}};
const queue=state.emergentTerrainQueue??={tail:Promise.resolve(),pending:0};
export async function runTerrainTool(project:ProjectSnapshot,settings:TerrainSettings,compare:boolean){
 if(queue.pending>=24)throw new Error("Terrain generation queue is busy. Try again shortly.");queue.pending++;
 const task=queue.tail.catch(()=>{}).then(()=>generateTerrain(project,settings,compare));queue.tail=task;
 try{return await task;}finally{queue.pending--;}
}
async function generateTerrain(project:ProjectSnapshot,settings:TerrainSettings,compare:boolean){
 let directory:string|undefined;
 try{
  directory=await mkdtemp(path.join(os.tmpdir(),"emergent-terrain-"));const candidate=path.join(directory,"terrain.toml");
  const recipe=Object.fromEntries(Object.entries(settings).filter(([key])=>!key.startsWith("sun")));
  await writeFile(candidate,stringify(recipe));const root=await foundationRoot(project);
  const result=await execute("cargo",["run","--release","--quiet","--manifest-path",path.join(root,"Cargo.toml"),"--example","terrain_builder","--","--settings",candidate,...(compare?["--compare"]:[])],{cwd:root,shell:false,encoding:"buffer",timeout:120000,maxBuffer:8+(compare?3:1)*(12+settings.resolution**2*4),windowsHide:true});
  return decodeTerrainFields(result.stdout.buffer.slice(result.stdout.byteOffset,result.stdout.byteOffset+result.stdout.byteLength),settings,compare);
 }catch(error){if(error&&typeof error==="object"&&"stderr"in error)throw new Error(String(error.stderr).slice(-6000)||"Rust terrain generation failed.");throw error;}
 finally{if(directory)await rm(directory,{recursive:true,force:true});}
}
