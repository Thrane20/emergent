import assert from "node:assert/strict";
import test from "node:test";
import {mkdir,mkdtemp,readFile,rm,writeFile} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {POST} from "../src/app/api/structures/ai/route";
import {updateAISettings} from "../src/lib/ai-settings";

test("Object AI accepts empty drafts and follow-ups, validates provider edits and never saves",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-structure-ai-")),previousRoot=process.env.EMERGENT_PROJECT_PATH,previousData=process.env.EMERGENT_IDE_DATA_DIR;
 process.env.EMERGENT_PROJECT_PATH=root;process.env.EMERGENT_IDE_DATA_DIR=path.join(root,"data");
 t.after(async()=>{if(previousRoot===undefined)delete process.env.EMERGENT_PROJECT_PATH;else process.env.EMERGENT_PROJECT_PATH=previousRoot;if(previousData===undefined)delete process.env.EMERGENT_IDE_DATA_DIR;else process.env.EMERGENT_IDE_DATA_DIR=previousData;await rm(root,{recursive:true,force:true});});
 await mkdir(path.join(root,"assets"));await mkdir(path.join(root,"src"));await writeFile(path.join(root,"emergent.toml"),'id="object-ai"\nname="AI test"\nasset_roots=["assets"]\nentry_scene="assets/main.scene.toml"');
 const scene='schema_version=1\nid="main"\nentities=[]\n';await writeFile(path.join(root,"assets/main.scene.toml"),scene);
 await updateAISettings({provider:"openai-codex",baseUrl:"http://localhost:9999/v1",model:"test-model",apiKey:"test-only"});
 let output:unknown[]=[{type:"function_call",call_id:"proposal",name:"replace_object",arguments:JSON.stringify({name:"Drone",parts:[{id:"body",name:"Body",primitive:"Cube",parent:null}]})}],requests=0,lastPrompt="";
 t.mock.method(globalThis,"fetch",async(_url:unknown,options:RequestInit)=>{requests++;lastPrompt=String(options.body);return Response.json({status:"completed",output});});
 const draft={schema_version:1,id:"draft",name:"New object",parts:[]};
 const send=(body:unknown,origin?:string)=>POST(new Request("http://localhost:3001/api/structures/ai",{method:"POST",headers:{"Content-Type":"application/json",...(origin?{origin}:{})},body:JSON.stringify(body)}));
 let response=await send({projectId:"object-ai",document:draft,message:"Make a drone"});assert.equal(response.status,200);const made=await response.json();assert.equal(made.document.parts.length,1);assert.equal(made.document.id,"draft");
 output=[{type:"message",content:[{type:"output_text",text:"Yes, we can add four rotors."}]}];
 response=await send({projectId:"object-ai",document:made.document,message:"Can it have rotors?",history:[{role:"user",text:"Make a drone"}]});assert.equal(response.status,200);assert.equal((await response.json()).document,undefined);assert.match(lastPrompt,/Make a drone/);
 output=[{type:"function_call",call_id:"bad",name:"replace_object",arguments:JSON.stringify({parts:[{id:"a",name:"A",primitive:"Cube",parent:"missing"}]})}];
 assert.equal((await send({projectId:"object-ai",document:draft,message:"Broken"})).status,400);
 const before=requests;assert.equal((await send({projectId:"other",document:draft,message:"No"})).status,400);assert.equal((await send({projectId:"object-ai",document:draft,message:"No"},"https://example.invalid")).status,400);assert.equal(requests,before);
 assert.equal(await readFile(path.join(root,"assets/main.scene.toml"),"utf8"),scene);
});
