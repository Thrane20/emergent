import "server-only";
import {randomUUID} from "node:crypto";
import {execFile} from "node:child_process";
import path from "node:path";
import {stringify} from "smol-toml";
import {readProjectBinding,type ProjectSnapshot} from "./project";
import {foundationRoot} from "./project-code-context";
import {applyChanges} from "./statement-files";
import {tweenAsset,tweenSettings} from "./tween";

export function prepareTweenSave(project:ProjectSnapshot,input:{name:unknown;settings:unknown}) {
  if(typeof input.name!=="string"||!input.name.trim()||Array.from(input.name.trim()).length>64||/[\p{Cc}\p{Cf}]/u.test(input.name))throw new Error("Use a tween name of 1–64 printable characters.");
  const root=project.assetRoots[0];if(!root)throw new Error("Project has no asset root.");
  const id=randomUUID(),relative=`${root}/tweens/${id}.tween.toml`,name=input.name.trim();
  const content=stringify({schema_version:1,id,name,kind:"tween",tween:tweenSettings(input.settings)});
  const asset=tweenAsset(relative,content);
  return {change:{path:relative,before:null,content},asset};
}

export async function runTweenTool(project:ProjectSnapshot,source:string):Promise<string> {
  if(Buffer.byteLength(source)>8192)throw new Error("Tween asset exceeds 8 KiB.");
  const root=await foundationRoot(project);
  return new Promise((resolve,reject)=>{
    const child=execFile("cargo",["run","--quiet","--manifest-path",path.join(root,"Cargo.toml"),"--example","tween_sample"],{cwd:root,shell:false,timeout:120000,maxBuffer:32768,windowsHide:true},(error,stdout,stderr)=>{
      if(error)reject(new Error(stderr.slice(-3000)||"Rust tween validation failed."));else resolve(stdout);
    });
    child.stdin?.on("error",()=>{/* Process exit is reported by the callback. */});child.stdin?.end(source);
  });
}
const state=globalThis as typeof globalThis&{emergentTweenSaveActive?:boolean};
export async function saveTween(project:ProjectSnapshot,input:Parameters<typeof prepareTweenSave>[1]) {
  if(state.emergentTweenSaveActive)throw new Error("Another tween is being saved. Wait for it to finish.");state.emergentTweenSaveActive=true;
  try{
    const {change,asset}=prepareTweenSave(project,input);
    await runTweenTool(project,change.content);
    const current=await readProjectBinding();if(current.id!==project.id||current.root!==project.root)throw new Error("Linked project changed during validation. No tween was saved.");
    await applyChanges(project,[change]);return asset;
  }finally{state.emergentTweenSaveActive=false;}
}
