"use client";
import {useEffect,useRef,useState,type PointerEvent as ReactPointerEvent} from "react";
import {orderShaderNodes,shaderArity,shaderGraphGaps,shaderNodeInfo,shaderOps,shaderOutputSockets,type ShaderGraph,type ShaderNode,type ShaderOp} from "@/lib/shader-graph";

// Fixed card geometry, so socket positions come from data rather than DOM measurement.
const WIDTH=176,HEADER=32,ROW=26,FIELD=38;
type View={x:number;y:number;k:number};
type Output=(typeof shaderOutputSockets)[number];
/** A socket a wire can end on: a node input, or a surface output. */
type Target={node:string;input:number}|{output:number};
type Wire={from?:string;to?:Target;x:number;y:number};

const hasField=(op:ShaderOp)=>op==="constant"||op==="parameter";
const nodeHeight=(n:ShaderNode)=>HEADER+shaderArity(n.op)*ROW+(hasField(n.op)?FIELD:0)+6;
const outputHeight=HEADER+shaderOutputSockets.length*ROW+6;
const outputSource=(g:ShaderGraph,[,key,i]:Output)=>key==="color"?g.outputs.color[i]:g.outputs[key];
function withOutput(g:ShaderGraph,[,key,i]:Output,id:string):ShaderGraph{
 return {...g,outputs:key==="color"?{...g.outputs,color:g.outputs.color.map((v,j)=>j===i?id:v)}:{...g.outputs,[key]:id}};
}
const wirePath=(x1:number,y1:number,x2:number,y2:number)=>{const d=Math.max(40,Math.abs(x2-x1)/2);return `M${x1} ${y1}C${x1+d} ${y1} ${x2-d} ${y2} ${x2} ${y2}`;};
const newId=(op:ShaderOp,g:ShaderGraph)=>{for(let i=1;;i++){const id=`${op.replace("_","-")}-${i}`;if(!g.nodes.some(n=>n.id===id))return id;}};

/** Node-based view of a shader graph: inputs on the left of each card, its value on the right, wires between. */
export function ShaderNodeGraph({graph,fitKey,onChange,onRefuse}:{graph:ShaderGraph;fitKey:number;onChange:(next:ShaderGraph)=>void;onRefuse:(message:string)=>void}){
 const surface=useRef<HTMLDivElement>(null),[view,setView]=useState<View>({x:40,y:40,k:1}),[selected,setSelected]=useState<string>(),[drag,setDrag]=useState<{id:string;dx:number;dy:number;x:number;y:number}>(),[wire,setWire]=useState<Wire>(),pan=useRef<{x:number;y:number;vx:number;vy:number}>(undefined);
 const position=(n:ShaderNode)=>drag?.id===n.id?[drag.x,drag.y] as const:n.position??[0,0];
 const outputPosition=drag?.id==="@output"?[drag.x,drag.y] as const:graph.outputs.position??[0,0];
 const local=(clientX:number,clientY:number)=>{const r=surface.current!.getBoundingClientRect();return [(clientX-r.left-view.x)/view.k,(clientY-r.top-view.y)/view.k] as const;};
 const byId=new Map(graph.nodes.map(n=>[n.id,n]));
 const sourcePoint=(id:string)=>{const n=byId.get(id);if(!n)return undefined;const [x,y]=position(n);return [x+WIDTH,y+HEADER/2] as const;};
 const targetPoint=(t:Target)=>{
  if("output" in t){const [x,y]=outputPosition;return [x,y+HEADER+t.output*ROW+ROW/2] as const;}
  const n=byId.get(t.node)!,[x,y]=position(n);return [x,y+HEADER+t.input*ROW+ROW/2] as const;
 };
 const gaps=new Set(shaderGraphGaps(graph));

 function fitAll(){
  const element=surface.current;if(!element)return;
  const boxes=[...graph.nodes.map(n=>[...(n.position??[0,0]),nodeHeight(n)] as const),[...(graph.outputs.position??[0,0]),outputHeight] as const];
  const x0=Math.min(...boxes.map(b=>b[0])),y0=Math.min(...boxes.map(b=>b[1])),x1=Math.max(...boxes.map(b=>b[0]+WIDTH)),y1=Math.max(...boxes.map(b=>b[1]+b[2]));
  const {width,height}=element.getBoundingClientRect();if(width<=60||height<=100)return;
  const k=Math.min(1.2,(width-60)/(x1-x0||1),(height-100)/(y1-y0||1));
  setView({k,x:(width-(x1-x0)*k)/2-x0*k,y:(height-(y1-y0)*k)/2-y0*k});
 }
 useEffect(()=>{
  // Opening a shader refits it; edits keep the current view.
  fitAll();
  // eslint-disable-next-line react-hooks/exhaustive-deps -- Only refit on open, not on every edit.
 },[fitKey]);
 function zoom(factor:number){
  const element=surface.current;if(!element)return;
  const {width,height}=element.getBoundingClientRect(),mx=width/2,my=height/2;
  setView(before=>{const k=Math.min(2,Math.max(Math.min(.02,before.k),before.k*factor));return {k,x:mx-(mx-before.x)*k/before.k,y:my-(my-before.y)*k/before.k};});
 }

 function connect(from:string,to:Target){
  if("output" in to)return onChange(withOutput(graph,shaderOutputSockets[to.output],from));
  if(from===to.node)return onRefuse("A node can't feed itself.");
  const nodes=graph.nodes.map(n=>n.id===to.node?{...n,inputs:n.inputs.map((v,i)=>i===to.input?from:v)}:n);
  try{onChange({...graph,nodes:orderShaderNodes(nodes)});}catch(e){onRefuse(e instanceof Error?e.message:"Connection refused.");}
 }
 function disconnect(t:Target){
  if("output" in t)return onChange(withOutput(graph,shaderOutputSockets[t.output],""));
  onChange({...graph,nodes:graph.nodes.map(n=>n.id===t.node?{...n,inputs:n.inputs.map((v,i)=>i===t.input?"":v)}:n)});
 }
 function remove(id:string){
  const clear=(v:string)=>v===id?"":v;
  onChange({...graph,nodes:graph.nodes.filter(n=>n.id!==id).map(n=>({...n,inputs:n.inputs.map(clear)})),outputs:{...graph.outputs,color:graph.outputs.color.map(clear),emission:clear(graph.outputs.emission),opacity:clear(graph.outputs.opacity),displacement:clear(graph.outputs.displacement)}});
  setSelected(undefined);
 }
 function add(op:ShaderOp){
  if(graph.nodes.length>=32)return onRefuse("Shaders hold at most 32 nodes.");
  const r=surface.current!.getBoundingClientRect(),[cx,cy]=local(r.left+r.width/2,r.top+r.height/2),id=newId(op,graph);
  let next=graph,parameter:string|undefined;
  if(op==="parameter"){
   parameter=graph.parameters[0]?.id;
   if(!parameter){if(graph.parameters.length>=16)return onRefuse("Shaders expose at most 16 parameters.");parameter=`p-${crypto.randomUUID().slice(0,8)}`;next={...graph,parameters:[...graph.parameters,{id:parameter,name:"New parameter",default:1,min:0,max:10}]};}
  }
  const node:ShaderNode={id,op,inputs:Array(shaderArity(op)).fill(""),value:op==="constant"?1:0,...(parameter?{parameter}:{}),position:[Math.round(cx-WIDTH/2),Math.round(cy-40)]};
  onChange({...next,nodes:[...next.nodes,node]});setSelected(id);
 }
 const update=(id:string,patch:Partial<ShaderNode>)=>onChange({...graph,nodes:graph.nodes.map(n=>n.id===id?{...n,...patch}:n)});

 // Pointer handling: pan on the background, move cards by their header, wires from sockets.
 function backgroundDown(e:ReactPointerEvent){if(e.target!==e.currentTarget&&!(e.target as Element).closest(".shader-graph-wires"))return;setSelected(undefined);pan.current={x:e.clientX,y:e.clientY,vx:view.x,vy:view.y};e.currentTarget.setPointerCapture(e.pointerId);}
 function cardDown(e:ReactPointerEvent,id:string,at:readonly [number,number]){if(e.button!==0)return;e.stopPropagation();const [x,y]=local(e.clientX,e.clientY);setSelected(id==="@output"?undefined:id);setDrag({id,dx:x-at[0],dy:y-at[1],x:at[0],y:at[1]});(e.currentTarget as Element).setPointerCapture(e.pointerId);}
 function socketDown(e:ReactPointerEvent,start:Wire){e.stopPropagation();e.preventDefault();surface.current!.setPointerCapture(e.pointerId);setWire(start);}
 function move(e:ReactPointerEvent){
  if(pan.current){const p=pan.current;setView({...view,x:p.vx+e.clientX-p.x,y:p.vy+e.clientY-p.y});return;}
  const [x,y]=local(e.clientX,e.clientY);
  if(drag)setDrag({...drag,x:Math.round(x-drag.dx),y:Math.round(y-drag.dy)});
  if(wire)setWire({...wire,x,y});
 }
 function up(e:ReactPointerEvent){
  pan.current=undefined;
  if(drag){
   const moved=drag.id==="@output"?graph.outputs.position?.[0]!==drag.x||graph.outputs.position?.[1]!==drag.y:byId.get(drag.id)?.position?.[0]!==drag.x||byId.get(drag.id)?.position?.[1]!==drag.y;
   if(moved)onChange(drag.id==="@output"?{...graph,outputs:{...graph.outputs,position:[drag.x,drag.y]}}:{...graph,nodes:graph.nodes.map(n=>n.id===drag.id?{...n,position:[drag.x,drag.y]}:n)});
   setDrag(undefined);
  }
  if(wire){
   const socket=document.elementFromPoint(e.clientX,e.clientY)?.closest<HTMLElement>("[data-socket]")?.dataset.socket;
   const [kind,a,b]=socket?.split("|")??[];
   if(wire.from&&kind==="in")connect(wire.from,{node:a,input:Number(b)});
   else if(wire.from&&kind==="out")connect(wire.from,{output:Number(a)});
   else if(wire.to&&kind==="value")connect(a,wire.to);
   setWire(undefined);
  }
 }
 function wheel(e:React.WheelEvent){
  const r=surface.current!.getBoundingClientRect(),k=Math.min(2,Math.max(Math.min(.02,view.k),view.k*Math.exp(-e.deltaY*.0015))),mx=e.clientX-r.left,my=e.clientY-r.top;
  setView({k,x:mx-(mx-view.x)*k/view.k,y:my-(my-view.y)*k/view.k});
 }
 // Picking up a connected input's wire carries it from its source; dropping it on empty space disconnects.
 function inputDown(e:ReactPointerEvent,t:Target,source:string|undefined){
  const [x,y]=local(e.clientX,e.clientY);
  if(source&&byId.has(source)){disconnect(t);socketDown(e,{from:source,x,y});}else socketDown(e,{to:t,x,y});
 }
 useEffect(()=>{
  // Delete removes the selected node when the canvas (not a field) has focus.
  const element=surface.current;if(!element)return;
  const key=(e:KeyboardEvent)=>{if(selected&&(e.key==="Delete"||e.key==="Backspace")&&!(e.target as Element).closest("input,select,textarea")){e.preventDefault();remove(selected);}};
  const preventScroll=(e:WheelEvent)=>e.preventDefault();
  element.addEventListener("wheel",preventScroll,{passive:false});
  element.addEventListener("keydown",key);return()=>{element.removeEventListener("keydown",key);element.removeEventListener("wheel",preventScroll);};
 });

 const wires:{key:string;d:string}[]=[];
 for(const n of graph.nodes)n.inputs.forEach((source,i)=>{const a=sourcePoint(source);if(a){const b=targetPoint({node:n.id,input:i});wires.push({key:`${n.id}:${i}`,d:wirePath(a[0],a[1],b[0],b[1])});}});
 shaderOutputSockets.forEach((socket,i)=>{const a=sourcePoint(outputSource(graph,socket));if(a){const b=targetPoint({output:i});wires.push({key:`out:${i}`,d:wirePath(a[0],a[1],b[0],b[1])});}});
 const pending=wire&&(wire.from?(()=>{const a=sourcePoint(wire.from!)!;return wirePath(a[0],a[1],wire.x,wire.y);})():(()=>{const b=targetPoint(wire.to!);return wirePath(wire.x,wire.y,b[0],b[1]);})());
 const chosen=selected?byId.get(selected):undefined;
 const groups=(["Input","Math","Pattern"] as const).map(group=>[group,shaderOps.filter(op=>shaderNodeInfo[op].group===group)] as const);

 return <div className="shader-graph">
  <div className="shader-graph-palette" role="toolbar" aria-label="Add shader node">{groups.map(([group,ops])=><div key={group}><span>{group}</span>{ops.map(op=><button key={op} title={shaderNodeInfo[op].description} disabled={graph.nodes.length>=32} onClick={()=>add(op)}>{shaderNodeInfo[op].label}</button>)}</div>)}<span className="shader-graph-count">{graph.nodes.length}/32 nodes</span></div>
  <div ref={surface} className={`shader-graph-surface${wire?" wiring":""}`} tabIndex={0} aria-label="Shader node graph" onPointerDown={backgroundDown} onPointerMove={move} onPointerUp={up} onPointerCancel={up} onWheel={wheel} onDoubleClick={e=>{if(e.target===e.currentTarget)fitAll();}}>
   <div className="shader-graph-view-controls" role="toolbar" aria-label="Graph view controls" onPointerDown={e=>e.stopPropagation()}>
    <button type="button" aria-label="Zoom out graph" title="Zoom out" onClick={()=>zoom(1/1.25)}>−</button>
    <output aria-label="Graph zoom">{Math.round(view.k*100)}%</output>
    <button type="button" aria-label="Zoom in graph" title="Zoom in" onClick={()=>zoom(1.25)}>+</button>
    <button type="button" aria-label="Fit all and center graph" title="Fit all and center" onClick={fitAll}>Fit all &amp; center</button>
   </div>
   <div className="shader-graph-layer" style={{transform:`translate(${view.x}px,${view.y}px) scale(${view.k})`}}>
    <svg className="shader-graph-wires" width="1" height="1" aria-hidden="true">{wires.map(w=><path key={w.key} d={w.d}/>)}{pending&&<path className="pending" d={pending}/>}</svg>
    {graph.nodes.map(n=>{const [x,y]=position(n),info=shaderNodeInfo[n.op];return <div key={n.id} className={`shader-card group-${info.group.toLowerCase()}${selected===n.id?" selected":""}`} style={{left:x,top:y,width:WIDTH,height:nodeHeight(n)}} data-node={n.id} onPointerDown={e=>{if(!(e.target as Element).closest("input,select,button,[data-socket]"))cardDown(e,n.id,[x,y]);}}>
     <header title={info.description}><b>{info.label}</b><small>{n.id}</small><span className="socket value" data-socket={`value|${n.id}`} aria-label={`${n.id} output`} onPointerDown={e=>{const [px,py]=local(e.clientX,e.clientY);socketDown(e,{from:n.id,x:px,y:py});}}/></header>
     {n.inputs.map((source,i)=>{const label=info.inputs[i]??`Input ${i+1}`,open=!byId.has(source);return <div key={i} className="shader-card-row"><span className={`socket${open?" open":""}${gaps.size&&open?" missing":""}`} data-socket={`in|${n.id}|${i}`} aria-label={`${n.id} ${label} input`} onPointerDown={e=>inputDown(e,{node:n.id,input:i},source)}/>{label}</div>;})}
     {n.op==="constant"&&<input className="shader-card-field" type="number" step="any" aria-label={`Node ${n.id} value`} value={Number.isFinite(n.value)?n.value:""} onChange={e=>update(n.id,{value:e.target.valueAsNumber})}/>}
     {n.op==="parameter"&&<select className="shader-card-field" aria-label={`Node ${n.id} parameter`} value={n.parameter??""} onChange={e=>update(n.id,{parameter:e.target.value})}><option value="">Choose parameter</option>{graph.parameters.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select>}
    </div>;})}
    <div className="shader-card surface" style={{left:outputPosition[0],top:outputPosition[1],width:WIDTH,height:outputHeight}} onPointerDown={e=>{if(!(e.target as Element).closest("[data-socket]"))cardDown(e,"@output",outputPosition);}}>
     <header><b>Surface output</b></header>
     {shaderOutputSockets.map((socket,i)=>{const source=outputSource(graph,socket),open=!byId.has(source);return <div key={socket[0]} className="shader-card-row"><span className={`socket${open?" open missing":""}`} data-socket={`out|${i}`} aria-label={`Surface ${socket[0]} input`} onPointerDown={e=>inputDown(e,{output:i},source)}/>{socket[0]}</div>;})}
    </div>
   </div>
   <p className="shader-graph-hint">Drag from a node’s right socket to an input. Drag a connected input away to unplug it. Drag the background to pan, scroll to zoom.</p>
  </div>
  {chosen?<section className="shader-node-inspector" aria-label={`Node ${chosen.id}`}>
   <b>{chosen.id}</b>
   <label>Operation<select aria-label={`Node ${chosen.id} operation`} value={chosen.op} onChange={e=>{const op=e.target.value as ShaderOp,arity=shaderArity(op);update(chosen.id,{op,inputs:Array.from({length:arity},(_,i)=>chosen.inputs[i]??""),parameter:op==="parameter"?chosen.parameter??graph.parameters[0]?.id:undefined});}}>{shaderOps.map(op=><option key={op} value={op}>{shaderNodeInfo[op].label}</option>)}</select></label>
   {chosen.inputs.map((source,i)=><label key={i}>{shaderNodeInfo[chosen.op].inputs[i]??`Input ${i+1}`}<select aria-label={`Node ${chosen.id} input ${i+1}`} value={byId.has(source)?source:""} onChange={e=>e.target.value?connect(e.target.value,{node:chosen.id,input:i}):disconnect({node:chosen.id,input:i})}><option value="">Not connected</option>{graph.nodes.filter(n=>n.id!==chosen.id).map(n=><option key={n.id} value={n.id}>{n.id}</option>)}</select></label>)}
   <p>{shaderNodeInfo[chosen.op].description}</p>
   <button onClick={()=>remove(chosen.id)}>Delete node</button>
  </section>:<section className="shader-node-inspector" aria-label="Surface outputs">
   <b>Surface output</b>
   {shaderOutputSockets.map((socket,i)=>{const source=outputSource(graph,socket);return <label key={socket[0]}>{socket[0]}<select aria-label={`Output ${socket[0]}`} value={byId.has(source)?source:""} onChange={e=>e.target.value?connect(e.target.value,{output:i}):disconnect({output:i})}><option value="">Not connected</option>{graph.nodes.map(n=><option key={n.id} value={n.id}>{n.id}</option>)}</select></label>;})}
   <p>Select a node to change its operation or inputs from the keyboard.</p>
  </section>}
 </div>;
}
