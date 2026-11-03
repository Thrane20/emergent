export const shaderOps=["constant","parameter","time","position_x","position_y","position_z","uv_x","uv_y","fresnel","noise","add","multiply","sine","one_minus","smoothstep"] as const;
export type ShaderOp=typeof shaderOps[number];
export type ShaderNode={id:string;op:ShaderOp;inputs:string[];value:number;parameter?:string;position?:[number,number]};
export type ShaderParameter={id:string;name:string;default:number;min:number;max:number};
export type ShaderGraph={schema_version:1;id:string;name:string;transparent:boolean;parameters:ShaderParameter[];nodes:ShaderNode[];outputs:{color:string[];emission:string;opacity:string;displacement:string;position?:[number,number]}};
export const shaderArity=(op:ShaderOp)=>op==="add"||op==="multiply"?2:op==="smoothstep"?3:["noise","sine","one_minus"].includes(op)?1:0;
const id=(v:unknown):v is string=>typeof v==="string"&&/^[a-z0-9_-]{1,64}$/i.test(v);
const name=(v:unknown)=>typeof v==="string"&&v.trim()&&v.length<=64&&!/[\p{Cc}\p{Cf}]/u.test(v);
const finite=(v:unknown):v is number=>typeof v==="number"&&Number.isFinite(v)&&Math.abs(v)<=10000;
/** Editor-only card position; compilation ignores it. */
const layout=(v:unknown)=>v===undefined||(Array.isArray(v)&&v.length===2&&v.every(n=>typeof n==="number"&&Number.isFinite(n)&&Math.abs(n)<=100000));
export function shaderGraph(value:unknown):ShaderGraph {
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Expected a shader graph.");
 const g=structuredClone(value) as ShaderGraph;
 if(Object.keys(g).some(k=>!["schema_version","id","name","transparent","parameters","nodes","outputs"].includes(k))||g.schema_version!==1||!id(g.id)||!name(g.name)||typeof g.transparent!=="boolean"||!Array.isArray(g.nodes)||!g.nodes.length||g.nodes.length>32||!Array.isArray(g.parameters)||g.parameters.length>16)throw new Error("Use schema 1, an ID/name, 1–32 nodes and at most 16 parameters.");
 const params=new Set<string>();
 for(const p of g.parameters){if(!p||Object.keys(p).some(k=>!["id","name","default","min","max"].includes(k))||!id(p.id)||!name(p.name)||params.has(p.id)||![p.default,p.min,p.max].every(finite)||p.min>p.max||p.default<p.min||p.default>p.max)throw new Error("Invalid or duplicate shader parameter.");params.add(p.id);}
 const nodes=new Map<string,boolean>();
 for(const n of g.nodes){
  if(!n||typeof n!=="object"||Array.isArray(n))throw new Error("Invalid shader node.");
  if(n.inputs===undefined)n.inputs=[];if(n.value===undefined)n.value=0;
  if(!n||Object.keys(n).some(k=>!["id","op","inputs","value","parameter","position"].includes(k))||!layout(n.position)||!id(n.id)||nodes.has(n.id)||!shaderOps.includes(n.op)||!finite(n.value)||!Array.isArray(n.inputs)||n.inputs.length!==shaderArity(n.op)||n.inputs.some(i=>!nodes.has(i))||(n.op==="parameter"?!params.has(n.parameter??""):n.parameter!==undefined))throw new Error(`Node ${n?.id??"?"}: check unique ID, operation, parameter and earlier inputs.`);
  nodes.set(n.id,n.op==="fresnel"||n.inputs.some(i=>nodes.get(i)));
 }
 const o=g.outputs;
 if(!o||Object.keys(o).some(k=>!["color","emission","opacity","displacement","position"].includes(k))||!layout(o.position)||!Array.isArray(o.color)||o.color.length!==3||[...o.color,o.emission,o.opacity,o.displacement].some(i=>!nodes.has(i)))throw new Error("Surface outputs must reference existing nodes.");
 if(nodes.get(o.displacement))throw new Error("Displacement cannot depend on Fresnel/view direction.");
 return g;
}
export function shaderValues(g:ShaderGraph,overrides:Record<string,number>={}) {
 if(Object.keys(overrides).some(id=>!g.parameters.some(p=>p.id===id)))throw new Error("Unknown shader parameter override.");
 return Array.from({length:16},(_,i)=>{const p=g.parameters[i];if(!p)return 0;const n=Object.hasOwn(overrides,p.id)?overrides[p.id]:p.default;if(!Number.isFinite(n)||n<p.min||n>p.max)throw new Error(`Parameter ${p.name} is outside ${p.min}..${p.max}.`);return n;});
}
export function shaderGLSL(value:ShaderGraph) {
 const g=shaderGraph(value),indices=new Map(g.nodes.map((n,i)=>[n.id,`n${i}`]));
 const code=g.nodes.map((n,i)=>{
  const a=n.inputs.map(id=>indices.get(id)!);let e:string;
  switch(n.op){
   case "constant":e=n.value.toFixed(8);break;
   case "parameter":e=`shaderParameters[${g.parameters.findIndex(p=>p.id===n.parameter)}]`;break;
   case "time":e="shaderTime";break;
   case "position_x":case "position_y":case "position_z":e=`p.${n.op.at(-1)}`;break;
   case "uv_x":e="tex.x";break;case "uv_y":e="tex.y";break;
   case "fresnel":e="clamp(1.0-abs(facing),0.0,1.0)";break;
   case "noise":e=`surfaceNoise(${a[0]})`;break;
   case "add":e=`(${a[0]}+${a[1]})`;break;case "multiply":e=`(${a[0]}*${a[1]})`;break;
   case "sine":e=`sin(${a[0]})`;break;case "one_minus":e=`(1.0-${a[0]})`;break;
   case "smoothstep":e=`surfaceSmooth(${a.join(",")})`;break;
  }
  return `float n${i}=clamp(${e},-10000.0,10000.0);`;
 }).join("\n");
 const o=g.outputs,ref=(id:string)=>indices.get(id)!;
 return `uniform float shaderTime;uniform float shaderParameters[16];
 struct SurfaceGraph { vec3 color;float emission;float opacity;float displacement; };
 float surfaceNoise(float x){float i=floor(x),f=fract(x),u=f*f*(3.0-2.0*f);return mix(fract(sin(i*127.1)*43758.5453),fract(sin((i+1.0)*127.1)*43758.5453),u);}
 float surfaceSmooth(float a,float b,float x){float t=clamp((x-a)/max(b-a,0.0001),0.0,1.0);return t*t*(3.0-2.0*t);}
 SurfaceGraph surfaceGraph(vec3 p,vec2 tex,float facing){${code}\nreturn SurfaceGraph(clamp(vec3(${o.color.map(ref).join(",")}),0.0,1.0),clamp(${ref(o.emission)},0.0,100.0),clamp(${ref(o.opacity)},0.0,1.0),clamp(${ref(o.displacement)},-100.0,100.0));}`;
}
export function shaderTemplate(kind:"force-field"|"water"|"dissolve"):ShaderGraph {
 const parameters:ShaderParameter[]=[],nodes:ShaderNode[]=[];
 const add=(id:string,op:ShaderOp,inputs:string[]=[],value=0,parameter?:string)=>{nodes.push({id,op,inputs,value,...(parameter?{parameter}:{})});return id;};
 const constant=(id:string,v:number)=>add(id,"constant",[],v);
 const param=(id:string,name:string,d:number,min:number,max:number)=>{parameters.push({id,name,default:d,min,max});return add(id,"parameter",[],0,id);};
 const zero=constant("zero",0),one=constant("one",1),time=add("time","time");
 const color=[param("red","Red",kind==="dissolve"?1:0.05,0,1),param("green","Green",kind==="dissolve"?0.25:0.6,0,1),param("blue","Blue",kind==="dissolve"?0.04:1,0,1)];
 let opacity=one,emission=zero,displacement=zero;
 if(kind==="force-field"){
  const rim=add("rim","fresnel"),strength=param("strength","Glow strength",1,0,10),speed=param("speed","Pulse speed",2,0,10),base=param("opacity","Base opacity",0.15,0,1);
  const phase=add("phase","multiply",[time,speed]),wave=add("pulse","sine",[phase]),amplitude=constant("pulse-amplitude",0.375),pulse=add("pulse-scaled","multiply",[wave,amplitude]),bias=constant("pulse-offset",0.625),pulseBias=add("pulse-bias","add",[pulse,bias]);
  emission=add("glow","multiply",[rim,strength]);emission=add("glow-pulse","multiply",[emission,pulseBias]);const alpha=add("alpha","add",[base,rim]);opacity=add("alpha-pulse","multiply",[alpha,pulseBias]);
 }else if(kind==="water"){
  const x=add("x","position_x"),z=add("z","position_z"),frequency=param("frequency","Wave frequency",3,0.1,20),speed=param("speed","Wave speed",1.5,0,10),height=param("height","Wave height (m)",0.12,0,1);
  const phase=add("phase","multiply",[time,speed]),a=add("x-frequency","multiply",[x,frequency]),b=add("z-frequency","multiply",[z,frequency]),ax=add("x-phase","add",[a,phase]),bz=add("z-phase","add",[b,phase]),sx=add("wave-x","sine",[ax]),sz=add("wave-z","sine",[bz]),sum=add("waves","add",[sx,sz]);displacement=add("displace","multiply",[sum,height]);opacity=param("opacity","Opacity",0.75,0,1);
 }else{
  const x=add("x","position_x"),y=add("y","position_y"),scale=param("scale","Noise scale",8,0.1,50),progress=param("progress","Dissolve amount",0.45,0,1),edge=param("edge","Edge width",0.08,0.001,0.3),strength=param("strength","Edge glow",4,0,10);
  const coord=add("coordinate","add",[x,y]),scaled=add("scaled","multiply",[coord,scale]),noise=add("pattern","noise",[scaled]),upper=add("upper","add",[progress,edge]);opacity=add("mask","smoothstep",[progress,upper,noise]);const inverse=add("edge-inverse","one_minus",[opacity]);emission=add("edge-glow","multiply",[inverse,strength]);
 }
 return shaderGraph({schema_version:1,id:kind,name:kind==="force-field"?"Force field":kind==="water"?"Stylised water":"Dissolve",transparent:true,parameters,nodes,outputs:{color,emission,opacity,displacement}});
}

/** What each operation is for and what its inputs mean; the node editor and MCP catalogue both read this. */
export const shaderNodeInfo:Record<ShaderOp,{label:string;group:"Input"|"Math"|"Pattern";inputs:string[];description:string}>={
 constant:{label:"Constant",group:"Input",inputs:[],description:"A fixed number (value)."},
 parameter:{label:"Parameter",group:"Input",inputs:[],description:"An exposed parameter that materials can override."},
 time:{label:"Time",group:"Input",inputs:[],description:"Seconds since the scene started."},
 position_x:{label:"Position X",group:"Input",inputs:[],description:"Object-space X in metres."},
 position_y:{label:"Position Y",group:"Input",inputs:[],description:"Object-space Y in metres."},
 position_z:{label:"Position Z",group:"Input",inputs:[],description:"Object-space Z in metres."},
 uv_x:{label:"UV X",group:"Input",inputs:[],description:"Texture coordinate U, 0..1."},
 uv_y:{label:"UV Y",group:"Input",inputs:[],description:"Texture coordinate V, 0..1."},
 fresnel:{label:"Fresnel",group:"Input",inputs:[],description:"0 facing the camera, 1 at grazing edges. Not usable for displacement."},
 add:{label:"Add",group:"Math",inputs:["A","B"],description:"A + B."},
 multiply:{label:"Multiply",group:"Math",inputs:["A","B"],description:"A × B."},
 sine:{label:"Sine",group:"Math",inputs:["Radians"],description:"sin(x)."},
 one_minus:{label:"One minus",group:"Math",inputs:["X"],description:"1 − x."},
 smoothstep:{label:"Smoothstep",group:"Math",inputs:["Low","High","X"],description:"Smooth 0→1 as X goes from Low to High."},
 noise:{label:"Noise",group:"Pattern",inputs:["X"],description:"Smooth 1D value noise, 0..1."},
};
export const shaderOutputSockets=[["Red","color",0],["Green","color",1],["Blue","color",2],["Emission","emission"],["Opacity","opacity"],["Displacement","displacement"]] as const;

/** Orders nodes so every input names an earlier node (the file contract), keeping the existing order where it already works. Throws on a loop. Unconnected ("") inputs are ignored. */
export function orderShaderNodes(nodes:ShaderNode[]):ShaderNode[]{
 const byId=new Map(nodes.map(n=>[n.id,n])),state=new Map<string,1|2>(),ordered:ShaderNode[]=[];
 const visit=(n:ShaderNode)=>{
  if(state.get(n.id)===2)return;if(state.get(n.id)===1)throw new Error("That connection would make a loop.");
  state.set(n.id,1);for(const input of n.inputs){const source=byId.get(input);if(source)visit(source);}state.set(n.id,2);ordered.push(n);
 };
 nodes.forEach(visit);return ordered;
}

/** Gives nodes without a position a place: columns by distance from the surface output (sources left), each node
 * level with the nodes it feeds. Positioned nodes stay put. Heights roughly match the editor's cards. */
export function layoutShaderGraph(g:ShaderGraph,column=230,gap=22):ShaderGraph{
 const height=(n:ShaderNode)=>44+shaderArity(n.op)*26+(n.op==="constant"||n.op==="parameter"?38:0);
 const consumers=new Map<string,{id:string;slot:number}[]>(g.nodes.map(n=>[n.id,[]]));
 for(const n of g.nodes)n.inputs.forEach((input,slot)=>consumers.get(input)?.push({id:n.id,slot}));
 const outputs=[...g.outputs.color,g.outputs.emission,g.outputs.opacity,g.outputs.displacement];
 outputs.forEach((id,slot)=>consumers.get(id)?.push({id:"@output",slot}));
 const distance=new Map<string,number>();
 for(const n of [...g.nodes].reverse())distance.set(n.id,Math.max(0,...consumers.get(n.id)!.map(c=>c.id==="@output"?0:distance.get(c.id)!+1)));
 const last=Math.max(0,...distance.values()),placed=new Map<string,[number,number]>(g.nodes.flatMap(n=>n.position?[[n.id,n.position] as const]:[]));
 const output:[number,number]=g.outputs.position??[(last+1)*column,0];
 const anchor=(id:string,slot:number)=>id==="@output"?output[1]+44+slot*26:(placed.get(id)?.[1]??0)+44+slot*26;
 for(let d=0;d<=last;d++){
  const x=(last-d)*column,col=g.nodes.filter(n=>!n.position&&distance.get(n.id)===d);
  const want=new Map(col.map(n=>{const c=consumers.get(n.id)!;return [n.id,c.length?c.reduce((sum,c)=>sum+anchor(c.id,c.slot),0)/c.length:Infinity] as const;}));
  let y=-Infinity;
  for(const n of [...col].sort((a,b)=>want.get(a.id)!-want.get(b.id)!)){
   const top=Math.max(y,Number.isFinite(want.get(n.id)!)?want.get(n.id)!-22:y===-Infinity?0:y);
   placed.set(n.id,[x,Math.round(top)]);y=top+height(n)+gap;
  }
 }
 return {...g,nodes:g.nodes.map(n=>n.position?n:{...n,position:placed.get(n.id)!}),outputs:{...g.outputs,position:output}};
}

/** Plain-language problems that stop the draft compiling (unconnected sockets), before the full contract check. */
export function shaderGraphGaps(g:ShaderGraph):string[]{
 const ids=new Set(g.nodes.map(n=>n.id)),gaps:string[]=[];
 for(const n of g.nodes)n.inputs.forEach((input,i)=>{if(!ids.has(input))gaps.push(`Connect ${shaderNodeInfo[n.op].inputs[i]??`input ${i+1}`} on ${n.id}.`);});
 for(const [label,key,i] of shaderOutputSockets){const id=key==="color"?g.outputs.color[i!]:g.outputs[key];if(!ids.has(id))gaps.push(`Connect ${label} on Surface output.`);}
 return gaps;
}
