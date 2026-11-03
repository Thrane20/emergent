import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {mkdtemp,rm} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {expect} from "@playwright/test";
async function main(){
 const data=await mkdtemp(path.join(os.tmpdir(),"emergent-fal-balance-")),base="http://[::1]:3001";
 let server:ReturnType<typeof spawn>|undefined,logs="";
 const env={...process.env,FAL_KEY:"",EMERGENT_IDE_DATA_DIR:data,EMERGENT_PROJECT_PATH:path.resolve("../../templates/standard"),EMERGENT_NEXT_DIST_DIR:".next-balance-build",EMERGENT_NAV_TEST_URL:base};
 try{
  server=spawn(process.execPath,[path.resolve("node_modules/next/dist/bin/next"),"start","--hostname","::1","--port","3001"],{env,stdio:["ignore","pipe","pipe"]});server.stdout?.on("data",c=>logs+=c);server.stderr?.on("data",c=>logs+=c);
  await expect.poll(async()=>{if(server?.exitCode!==null)throw new Error(logs);try{return (await fetch(`${base}/api/fal-balance`,{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:30000}).toBe(true);
  const response=await fetch(`${base}/api/fal-balance`),balance=await response.json();assert.equal(balance.state,"no-key");assert.equal(balance.balance,0);assert.match(response.headers.get("cache-control")??"",/no-store/);
  const child=spawn(path.resolve("node_modules/.bin/tsx"),["scripts/editor-navigation-smoke.ts"],{env,stdio:"inherit"});const code=await new Promise<number|null>((resolve,reject)=>{child.once("exit",resolve);child.once("error",reject);});assert.equal(code,0);
  console.log("PASS balance API no-key zero and private response; shared balance in all headers; ready/no-key/unavailable states and settings event. No paid calls.");
 }catch(error){console.error(logs);throw error;}finally{if(server){server.kill("SIGTERM");await new Promise<void>(resolve=>{if(server!.exitCode!==null)resolve();else server!.once("exit",()=>resolve());});}await rm(data,{recursive:true,force:true});}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
