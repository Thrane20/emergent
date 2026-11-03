import test from "node:test";
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {parse} from "smol-toml";
import {shaderGraph,shaderTemplate,shaderValues,shaderGLSL} from "../src/lib/shader-graph";
import {materialSettings} from "../src/lib/material-settings";
test("shader templates match native fixtures and compile bounded expressions",async()=>{
 for(const kind of ["force-field","water","dissolve"] as const){
  const g=shaderTemplate(kind);assert.ok(shaderGLSL(g).includes("surfaceGraph"));assert.equal(shaderValues(g).length,16);
  const source=await readFile(`../../templates/standard/assets/shaders/${kind}.shader.toml`,"utf8");assert.deepEqual(shaderGraph(parse(source)),g);
 }
});
test("shader graph rejects cycles, invalid bindings, view-based displacement and invalid parameters",()=>{
 const original=shaderTemplate("water");
 for(const edit of [(g:typeof original)=>g.nodes[0].inputs.push(g.nodes.at(-1)!.id),(g:typeof original)=>g.nodes[0].value=Infinity,(g:typeof original)=>g.outputs.opacity="missing",(g:typeof original)=>g.parameters[0].default=100001]){const g=structuredClone(original);edit(g);assert.throws(()=>shaderGraph(g));}
 const force=shaderTemplate("force-field");force.outputs.displacement="rim";assert.throws(()=>shaderGraph(force),/Displacement/);
 assert.throws(()=>shaderValues(original,{unknown:1}));assert.throws(()=>shaderValues(original,{height:NaN}));assert.throws(()=>shaderValues(original,{height:2}));
 assert.throws(()=>shaderGraph({...original,source:"arbitrary code"}));
});
test("shader materials require explicit shadow policy and preserve overrides",()=>{
 const material=materialSettings({shader:"assets/a.shader.toml",cast_shadows:false,shader_parameters:{speed:2}});assert.equal(material.shader_parameters?.speed,2);
 assert.throws(()=>materialSettings({shader:"assets/a.shader.toml"}));assert.throws(()=>materialSettings({shader:"assets/a.wgsl",cast_shadows:false}));assert.throws(()=>materialSettings({shader_parameters:{speed:2}}));
});

test("shader parameters resolve authored IDs without consulting object prototypes",()=>{
 const g=shaderTemplate("water");g.parameters[0].id="constructor";g.nodes.find(n=>n.op==="parameter"&&n.parameter==="red")!.parameter="constructor";
 shaderGraph(g);assert.equal(shaderValues(g)[0],g.parameters[0].default);
});

test("force field pulse drives opacity as well as emission",()=>{
 const graph=shaderTemplate("force-field");
 const opacity=graph.nodes.find(n=>n.id===graph.outputs.opacity)!;
 assert.equal(opacity.op,"multiply");assert.ok(opacity.inputs.includes("pulse-bias"));
 const amplitude=graph.nodes.find(n=>n.id==="pulse-amplitude")!.value;
 const bias=graph.nodes.find(n=>n.id==="pulse-offset")!.value;
 assert.equal(bias-amplitude,0.25);assert.equal(bias+amplitude,1);
});

test("node editor helpers: positions are optional layout, wiring is re-ordered, loops and gaps are reported",async()=>{
 const {layoutShaderGraph,orderShaderNodes,shaderGraphGaps,shaderNodeInfo,shaderOps,shaderArity}=await import("../src/lib/shader-graph");
 for(const op of shaderOps)assert.equal(shaderNodeInfo[op].inputs.length,shaderArity(op),op);
 const g=layoutShaderGraph(shaderTemplate("water"));
 assert.ok(g.nodes.every(n=>n.position)&&g.outputs.position);
 assert.equal(shaderGLSL(shaderGraph(g)),shaderGLSL(shaderTemplate("water")),"layout never changes the compiled shader");
 assert.throws(()=>shaderGraph({...g,outputs:{...g.outputs,position:[NaN,0]}}));
 // Inputs from the left sit in earlier columns; the output card is right of everything.
 const x=(id:string)=>g.nodes.find(n=>n.id===id)!.position![0];
 assert.ok(x("time")<x("phase")&&x("phase")<x("displace")&&x("displace")<g.outputs.position![0]);
 // Wiring a later node into an earlier one reorders so the file contract (inputs name earlier nodes) holds.
 const nodes=structuredClone(g.nodes),zero=nodes.find(n=>n.id==="x")!;zero.op="sine";zero.inputs=["time"];
 const moved=nodes.splice(nodes.findIndex(n=>n.id==="time"),1)[0];nodes.push(moved);
 const ordered=orderShaderNodes(nodes);assert.ok(ordered.findIndex(n=>n.id==="time")<ordered.findIndex(n=>n.id==="x"));
 shaderGraph({...g,nodes:ordered});
 // A loop is refused.
 const loop=structuredClone(g.nodes);loop.find(n=>n.id==="phase")!.inputs[0]="displace";
 assert.throws(()=>orderShaderNodes(loop),/loop/);
 const open=structuredClone(g);open.nodes.find(n=>n.id==="waves")!.inputs[1]="";open.outputs.emission="";
 assert.deepEqual(shaderGraphGaps(open),["Connect B on waves.","Connect Emission on Surface output."]);
});
