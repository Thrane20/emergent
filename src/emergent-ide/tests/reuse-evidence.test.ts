import assert from "node:assert/strict";
import test from "node:test";
import { implementationResponse } from "../src/lib/statement-ai";
import { parseImplementation, reuseEvidenceGuide } from "../src/lib/statement-proposal";

const context={root:"/unused",search:[],files:new Map([["assets/entities/cube.entity.toml",'schema_version=1\nid="cube-asset"\n[geometry]\nprimitive="cube"'],["foundation/src/world.rs","pub fn set_local_transform() {}"]])};
const proposal=(path="assets/entities/cube.entity.toml",symbol="cube.entity.toml")=>({summary:"Reuse the cube",reuse:[{path,symbol,reason:"Reuse inspected code or asset"}],files:[]});

test("reuse accepts inspected asset references and Rust call notation, while rejecting invented evidence",()=>{
 for(const symbol of ["cube.entity.toml","assets/entities/cube.entity.toml","cube-asset","primitive"]){assert.equal(parseImplementation(JSON.stringify(proposal(undefined,symbol)),context).reuse[0].symbol,symbol);}
 assert.equal(parseImplementation(JSON.stringify(proposal("./assets/entities/cube.entity.toml")),context).reuse[0].path,"assets/entities/cube.entity.toml");
 assert.doesNotThrow(()=>parseImplementation(JSON.stringify(proposal("foundation/src/world.rs","`World::set_local_transform()`")),context));
 assert.throws(()=>parseImplementation(JSON.stringify(proposal(undefined,"Cube primitive asset")),context),/Reuse symbol.*Cube primitive asset.*No files were applied/);
 assert.throws(()=>parseImplementation(JSON.stringify(proposal("assets/entities/missing.entity.toml")),context),/was not inspected/);
 assert.throws(()=>parseImplementation(JSON.stringify(proposal("foundation/src/world.rs","World::InventedSpin()")),context),/does not occur/);
 assert.ok(reuseEvidenceGuide(context).includes('"symbols":["cube.entity.toml"]'));
});

test("a rejected citation gets one guided correction, and a second rejection stays rejected",async()=>{
 let calls=0;
 const result=await implementationResponse("Add a cube",context,async prompt=>{
  calls++;
  assert.ok(prompt.includes("REUSE CITATION EXAMPLES"));
  if(calls===1)return JSON.stringify(proposal(undefined,"Cube primitive asset"));
  assert.ok(prompt.includes("REUSE CORRECTION"));assert.ok(prompt.includes("does not occur"));
  return JSON.stringify(proposal());
 });
 assert.equal(calls,2);assert.equal(result.reuse[0].symbol,"cube.entity.toml");
 calls=0;
 await assert.rejects(implementationResponse("Add a cube",context,async()=>{calls++;return JSON.stringify(proposal("missing.rs","nope"));}),/was not inspected/);
 assert.equal(calls,2);
 calls=0;
 await assert.rejects(implementationResponse("Add a cube",context,async()=>{calls++;return "invalid JSON";}),SyntaxError);
 assert.equal(calls,1);
});
