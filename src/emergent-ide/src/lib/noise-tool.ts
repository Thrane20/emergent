import "server-only";
import {execFile} from "node:child_process";
import {mkdtemp,writeFile,rm} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {promisify} from "node:util";
import type {ProjectSnapshot} from "./project";
import {foundationRoot} from "./project-code-context";
import {decodeNoiseField} from "./noise-field";
const execute=promisify(execFile);
const state=globalThis as typeof globalThis&{emergentNoiseToolActive?:boolean};
export async function runNoiseTool(project:ProjectSnapshot,asset:string|null,source:string,generate=false){
 if(state.emergentNoiseToolActive)throw new Error("Another noise generation/validation is running. Wait for it to finish.");state.emergentNoiseToolActive=true;
 let directory:string|undefined;
 try{
  directory=await mkdtemp(path.join(os.tmpdir(),"emergent-noise-"));const candidate=path.join(directory,"candidate.noise.toml");await writeFile(candidate,source);
  const root=await foundationRoot(project);
  const result=await execute("cargo",["run","--release","--quiet","--manifest-path",path.join(root,"Cargo.toml"),"--example","noise_filter","--","--project",project.root,...(asset===null?[]:["--asset",asset]),"--candidate",candidate,...(generate?["--generate"]:[])],{cwd:root,shell:false,encoding:"buffer",timeout:120000,maxBuffer:16+2097152*4,windowsHide:true});
  if(generate)decodeNoiseField(result.stdout.buffer.slice(result.stdout.byteOffset,result.stdout.byteOffset+result.stdout.byteLength));
  return result.stdout;
 }catch(error){if(error&&typeof error==="object"&&"stderr"in error)throw new Error(String(error.stderr).slice(-6000)||"Rust noise generation failed.");throw error;}
 finally{if(directory)await rm(directory,{recursive:true,force:true});state.emergentNoiseToolActive=false;}
}
