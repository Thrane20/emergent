import assert from "node:assert/strict";
import test from "node:test";
import {FalBalanceReader} from "../src/lib/fal-balance";
test("no fal key reports zero without contacting fal",async()=>{
 let calls=0;const reader=new FalBalanceReader(async()=>"",async()=>{calls++;throw new Error("must not call");});const result=await reader.get();assert.equal(result.state,"no-key");assert.equal(result.balance,0);assert.equal(calls,0);
});
test("fal balance uses credits, deduplicates requests, expires, and isolates key changes",async()=>{
 let key="first-secret",calls=0,time=100000;const credentials:string[]=[];
 const reader=new FalBalanceReader(async()=>key,async(input,init)=>{calls++;assert.equal(String(input),"https://api.fal.ai/v1/account/billing?expand=credits");credentials.push(new Headers(init?.headers).get("Authorization")!);return Response.json({username:"private-account",credits:{current_balance:calls+.25,currency:"USD"}});},()=>time);
 const [first,second]=await Promise.all([reader.get(),reader.get()]);assert.deepEqual(first,second);assert.equal(calls,1);assert.equal(first.balance,1.25);assert.ok(!JSON.stringify(first).includes("secret"));assert.ok(!JSON.stringify(first).includes("private-account"));await reader.get();await reader.get(true);assert.equal(calls,1);
 time+=6000;await reader.get(true);assert.equal(calls,2);time+=60000;await reader.get();assert.equal(calls,3);
 key="second-secret";assert.equal((await reader.get()).balance,4.25);assert.deepEqual(credentials,["Key first-secret","Key first-secret","Key first-secret","Key second-secret"]);
 key="";assert.equal((await reader.get()).balance,0);assert.equal(calls,4);
});
test("billing failures remain unavailable rather than a fabricated zero balance",async()=>{
 for(const status of [401,403,429,500]){const reader=new FalBalanceReader(async()=>"secret",async()=>new Response("Sensitive provider error",{status})),result=await reader.get();assert.equal(result.state,"unavailable");assert.equal(result.balance,null);assert.ok(!result.message.includes("Sensitive"));if(status===403)assert.match(result.message,/billing:usage:read/);}
 for(const credits of [undefined,{current_balance:-1,currency:"USD"},{current_balance:"8.76",currency:"USD"},{current_balance:8.76,currency:"INVALID"}])assert.equal((await new FalBalanceReader(async()=>"key",async()=>Response.json({credits})).get()).state,"unavailable");
 assert.equal((await new FalBalanceReader(async()=>"key",async()=>{throw new Error("secret network error");}).get()).balance,null);
});
test("cached-only reads never contact fal",async()=>{
 let calls=0;const reader=new FalBalanceReader(async()=>"key",async()=>{calls++;return Response.json({credits:{current_balance:2,currency:"USD"}});});
 assert.equal((await reader.get(false,true)).state,"unchecked");assert.equal(calls,0);
 await reader.get(true);assert.equal((await reader.get(false,true)).balance,2);assert.equal(calls,1);
});
