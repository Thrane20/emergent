import "server-only";
import {createHash} from "node:crypto";
import {objectApiKey} from "./object-settings";
import {boundedBody} from "./object-files";
import type {FalBalance} from "./fal-balance-types";
type Cache={fingerprint:string;result?:FalBalance;at:number;pending?:Promise<FalBalance>};
export class FalBalanceReader{
 private cache?:Cache;
 constructor(private credentials=objectApiKey,private transport:typeof fetch=fetch,private now=Date.now){}
 async get(force=false,cachedOnly=false):Promise<FalBalance>{
  const key=await this.credentials();
  if(!key){this.cache=undefined;return {state:"no-key",balance:0,currency:"USD",message:"No fal API key configured. Add one in Settings.",checkedAt:null};}
  const fingerprint=createHash("sha256").update(key).digest("hex");
  if(this.cache?.fingerprint!==fingerprint)this.cache={fingerprint,at:0};
  const cache=this.cache;
  if(cachedOnly)return cache.result??{state:"unchecked",balance:null,currency:"USD",message:"fal balance not checked yet. Press refresh.",checkedAt:null};
  if(cache.pending)return cache.pending;
  const ttl=cache.result?.state==="ready"?60000:15000;
  if(cache.result&&this.now()-cache.at<(force?5000:ttl))return cache.result;
  cache.pending=this.load(key).then(result=>{cache.result=result;cache.at=this.now();return result;}).finally(()=>{cache.pending=undefined;});
  return cache.pending;
 }
 private async load(key:string):Promise<FalBalance>{
  const unavailable=(message:string):FalBalance=>({state:"unavailable",balance:null,currency:"USD",message,checkedAt:null});
  try{
   const response=await this.transport("https://api.fal.ai/v1/account/billing?expand=credits",{cache:"no-store",headers:{Authorization:`Key ${key}`},redirect:"error",signal:AbortSignal.timeout(10000)});
   if(response.status===403)return unavailable("This fal key cannot read billing. It needs billing:usage:read permission.");
   if(response.status===401)return unavailable("fal rejected this API key. Check the saved key in Settings.");
   if(!response.ok)return unavailable("fal balance is temporarily unavailable. Click to retry.");
   const data=JSON.parse((await boundedBody(response,32768)).toString("utf8")),credits=data.credits;
   if(!credits||typeof credits.current_balance!=="number"||!Number.isFinite(credits.current_balance)||credits.current_balance<0||typeof credits.currency!=="string"||!/^[A-Z]{3}$/.test(credits.currency))return unavailable("fal returned an invalid balance. Click to retry.");
   return {state:"ready",balance:credits.current_balance,currency:credits.currency,message:"Available fal account credit. Click to refresh.",checkedAt:new Date(this.now()).toISOString()};
  }catch{return unavailable("Unable to reach fal billing. Click to retry.");}
 }
}
export const falBalance=new FalBalanceReader();
