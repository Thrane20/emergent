"use client";
import Link from "next/link";
import {useCallback,useEffect,useMemo,useRef,useState} from "react";
import {FalBalance} from "@/components/fal-balance";
import {BrainLibrary} from "@/components/brain-library";
import {FieldInput,type FieldOption} from "@/components/field-input";
import {ProjectChat} from "@/components/project-chat";
import {BrainPlaypen,type PlaypenObject} from "@/components/brain-playpen";
import {useEditorNavigationGuard} from "./editor-navigation";
import type {PublicAISettings} from "@/lib/ai-settings";
import {addNode,brainCreationContext,brainProblems,childrenOf,isEmptyBrainDraft,moveNode,newlyCreatedBrainPath,nodeSummary,parseTrace,removeNode,rootOf,setNodeField,shiftNode,type BrainDocument,type BrainNode,type BrainSummary,type BrainTrace,type NodeCatalogue} from "@/lib/brains";

type Open={path?:string;revision?:string;brain:BrainDocument};
type Instance={id:string;name:string};
const blank=():Open=>({brain:{id:crypto.randomUUID(),name:"New brain",description:"",nodes:[]}});
const nodeId=()=>`n-${crypto.randomUUID().slice(0,8)}`;
const TARGET_NODES=["line_of_sight","hide","seek","flee","arrive","pursue","evade","within_distance","in_view","score_distance","follow_target","follow_leader","formation","avoid","orbit","stay_within","outside_distance","attack","flank","encircle","kite"];
const FLOCK_NODES=["flock","separation","alignment","cohesion","neighbours_at_least","score_crowding","regroup"];

/** Groups the preview should populate: who flocks (the agents) and who they react to (the targets). */
function previewGroups(brain:BrainDocument){
 const group=(types:string[])=>brain.nodes.find(node=>types.includes(node.type)&&typeof node.fields.group==="string")?.fields.group as string|undefined;
 const text=(name:unknown)=>typeof name==="string"?name:undefined;
 const agents=group(FLOCK_NODES)??brain.nodes.map(node=>text(node.fields.members)??text(node.fields.allies)).find(Boolean)??"agents";
 const targets=[...brain.nodes.filter(node=>TARGET_NODES.includes(node.type)).map(node=>text(node.fields.group)),...brain.nodes.map(node=>text(node.fields.threats)??text(node.fields.enemies))].find(name=>name&&name!==agents);
 return {agents,targets:targets??"target"};
}

export function BrainEditor({projectId,projectName,scenePath,aiSettings,initialBrains,initial,objects}:{projectId:string;projectName:string;scenePath:string;aiSettings:PublicAISettings;initialBrains:BrainSummary[];initial?:Open;objects:PlaypenObject[]}){
 const [catalogue,setCatalogue]=useState<NodeCatalogue|null>(null),[brains,setBrains]=useState(initialBrains);
 const [open,setOpen]=useState<Open>(initial??blank()),[selected,setSelected]=useState<string|null>(initial?rootOf(initial.brain)?.id??null:null);
 const [dirty,setDirty]=useState(!initial),[busy,setBusy]=useState(false),[error,setError]=useState(""),[status,setStatus]=useState("");
 const [search,setSearch]=useState(""),[view,setView]=useState<"tree"|"preview"|"3d">("tree"),[instances,setInstances]=useState<Instance[]>([]);
 const [chatBusy,setChatBusy]=useState(false);
 useEditorNavigationGuard(useCallback(()=>!busy&&!chatBusy&&(!dirty||window.confirm("Discard unsaved brain changes?")),[busy,chatBusy,dirty]));
 const scratch=isEmptyBrainDraft(open.path,open.brain);
 const brain=open.brain,problems=useMemo(()=>catalogue?brainProblems(brain,catalogue):new Map<string,string>(),[brain,catalogue]);
 const spec=(type:string)=>catalogue?.nodes.find(node=>node.id===type);
 const loadScene=useCallback(async()=>{
  const response=await fetch(`/api/scenes/placement?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(scenePath)}`,{cache:"no-store"});const data=await response.json();
  if(!response.ok)return;
  // Object pickers for target and waypoint fields; the brain itself is a project asset, not part of the scene.
  setInstances((data.instances as {id:string;name?:string}[]).map(item=>({id:item.id,name:item.name??item.id})));
 },[projectId,scenePath]);
 useEffect(()=>{
  let active=true;
  fetch(`/api/brains?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"}).then(async response=>{const data=await response.json();if(!response.ok)throw new Error(data.error??"Node library unavailable.");if(active){setCatalogue(data.catalogue);setBrains(data.brains);}}).catch(failure=>{if(active)setError(failure instanceof Error?failure.message:"Node library unavailable.");});
  const timer=setTimeout(()=>void loadScene(),0);
  return()=>{active=false;clearTimeout(timer);};
 },[projectId,loadScene]);
 function newBrain(){if(busy||chatBusy)return;if(!dirty||window.confirm("Discard unsaved brain changes?")){setOpen(blank());setSelected(null);setDirty(true);setStatus("");window.history.replaceState(null,"",`/brains?scene=${encodeURIComponent(scenePath)}`);}}
 function change(next:BrainDocument){if(busy||chatBusy)return;setOpen(current=>({...current,brain:next}));setDirty(true);setStatus("");setError("");}
 function attempt(edit:()=>BrainDocument){try{change(edit());}catch(failure){setError(failure instanceof Error?failure.message:"That change is not possible.");}}
 function add(type:string,fields:Record<string,string>={}){
  if(!catalogue)return;const id=nodeId();
  const parent=selected&&brain.nodes.some(node=>node.id===selected)?selected:undefined;
  attempt(()=>addNode(brain,catalogue,type,parent,id,fields));setSelected(id);
 }
 async function save(){
  if(busy||chatBusy)return;
  setBusy(true);setError("");
  try{
   const response=await fetch("/api/brains",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path:open.path,revision:open.revision,brain})});
   const data=await response.json();if(!response.ok)throw new Error(data.error??"Brain could not be saved.");
   setOpen({path:data.path,revision:data.revision,brain:data.brain});setDirty(false);setStatus("Saved and validated by the engine.");
   const list=await fetch(`/api/brains?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"}).then(r=>r.json());if(list.brains)setBrains(list.brains);
   window.history.replaceState(null,"",`/brains?asset=${encodeURIComponent(data.path)}&scene=${encodeURIComponent(scenePath)}`);
  }catch(failure){setError(failure instanceof Error?failure.message:"Brain could not be saved.");}finally{setBusy(false);}
 }
 async function load(path:string,afterAI=false){
  if((busy||chatBusy)&&!afterAI)return;
  if(dirty&&!afterAI&&!window.confirm("Discard unsaved brain changes?"))return;
  setBusy(true);setError("");
  try{const response=await fetch(`/api/brains?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}`,{cache:"no-store"});const data=await response.json();if(!response.ok)throw new Error(data.error);setOpen(data);setSelected(rootOf(data.brain)?.id??null);setDirty(false);setStatus("");window.history.replaceState(null,"",`/brains?asset=${encodeURIComponent(path)}&scene=${encodeURIComponent(scenePath)}`);}
  catch(failure){setError(failure instanceof Error?failure.message:"Brain could not be opened.");}finally{setBusy(false);}
 }

 async function afterAI(){
  try{
   const response=await fetch(`/api/brains?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"});
   const data=await response.json();if(!response.ok)throw new Error(data.error??"Brain library could not be refreshed.");
   const fresh=data.brains as BrainSummary[];setBrains(fresh);
   const path=open.path??newlyCreatedBrainPath(brains,fresh);
   if(path)await load(path,true);
   else setStatus("Brain library refreshed. Select the new brain to open it.");
  }catch(failure){setError(failure instanceof Error?failure.message:"AI changes could not be opened.");}
 }

 const query=search.trim().toLowerCase();
 const library=catalogue?catalogue.categories.map(category=>({category,nodes:catalogue.nodes.filter(node=>node.category===category&&node.id!=="brain"&&(!query||`${node.label} ${node.description} ${category}`.toLowerCase().includes(query)))})):[];
 const node=brain.nodes.find(item=>item.id===selected),nodeSpec=node?spec(node.type):undefined;
 const brainOptions:FieldOption[]=brains.filter(item=>item.path!==open.path).map(item=>({id:item.path,name:item.name}));
 return <main className="object-shell brain-shell">
  <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Brain Editor</b></span><Link href={`/?scene=${encodeURIComponent(scenePath)}`} className="quiet-button">Project map ↗</Link><FalBalance/></header>
  <div className="object-heading terrain-library-heading"><div className="terrain-library-title"><span className="eyebrow">BEHAVIOUR / STEERING / EMERGENCE</span><h1>Behaviours</h1><p>Build, preview and save brains for the whole project.</p><span className="kind-badge">{catalogue?`${catalogue.nodes.filter(item=>item.status==="available").length} NODES`:"LOADING NODES"}</span></div>
   <BrainLibrary brains={brains} catalogue={catalogue} selected={open.path} disabled={busy||chatBusy} onSelect={path=>void load(path)} onNew={newBrain}/></div>
  <div className="brain-layout">
   <aside className="panel brain-library" aria-label="Node library">
    <div className="panel-head"><h2>Node library</h2></div>
    <input className="brain-search" type="search" aria-label="Search nodes" placeholder="Search nodes…" value={search} onChange={event=>setSearch(event.target.value)}/>
    <p className="brain-hint">{selected?<>Adds under <b>{node?(node.fields.label as string|undefined)??nodeSpec?.label:"selection"}</b>.</>:brain.nodes.length?"Select a node to add under it.":"The first node becomes the root."}</p>
    <div className="brain-categories">
     {library.map(group=>group.nodes.length>0&&<details key={group.category} open>
      <summary>{group.category}<small>{group.nodes.filter(item=>item.status==="available").length}</small></summary>
      {group.nodes.map(item=><button type="button" key={item.id} className="brain-library-node" data-role={item.role} disabled={item.status!=="available"||busy||chatBusy} draggable={item.status==="available"} title={item.description} onDragStart={event=>event.dataTransfer.setData("application/x-brain-node",item.id)} onClick={()=>add(item.id)}><b>{item.label}</b><small>{item.status==="planned"?"planned":item.role}</small></button>)}
     </details>)}
     <details open><summary>Saved brains<small>{brains.length}</small></summary>
      {brains.map(item=><button type="button" key={item.path} className="brain-library-node" data-role="reference" disabled={busy||chatBusy||item.path===open.path||!!item.error} title={item.error??`Embed ${item.name} as a sub-behaviour`} onClick={()=>add("brain",{brain:item.path})}><b>{item.name}</b><small>{item.error?"invalid":`${item.nodes} nodes`}</small></button>)}
     </details>
    </div>
   </aside>
   <section className="panel brain-workspace">
    <div className="panel-head"><div className="brain-title"><input aria-label="Brain name" value={brain.name} maxLength={64} onChange={event=>change({...brain,name:event.target.value})}/>{dirty&&<span className="kind-badge">UNSAVED</span>}</div>
     <div className="view-switch">{(["tree","preview","3d"] as const).map(item=><button type="button" key={item} className={view===item?"selected":""} onClick={()=>setView(item)}>{item==="tree"?"Tree":item==="preview"?"Preview":"3D preview"}</button>)}</div>
</div>
    {(error||status)&&<p className={error?"brain-error":"brain-status"} role={error?"alert":"status"}>{error||status}</p>}
    {view==="tree"?<div className="brain-canvas" onClick={event=>{if(event.target===event.currentTarget)setSelected(null);}} onDragOver={event=>event.preventDefault()} onDrop={event=>{const type=event.dataTransfer.getData("application/x-brain-node");if(type&&event.target===event.currentTarget){setSelected(null);if(catalogue){const id=nodeId();attempt(()=>addNode(brain,catalogue,type,undefined,id));setSelected(id);}}}}>
     {!brain.nodes.length&&<div className="brain-empty"><b>Start with a root node.</b><span>Composition nodes (Blend, Priority, Selector) hold other behaviours. Click one in the library, or drag it here.</span></div>}
     {rootOf(brain)&&<ul className="brain-tree"><TreeNode node={rootOf(brain)!} depth={0} brain={brain} catalogue={catalogue} selected={selected} problems={problems} onSelect={setSelected} onEdit={attempt} onAdd={(type,parent)=>{if(!catalogue)return;const id=nodeId();attempt(()=>addNode(brain,catalogue,type,parent,id));setSelected(id);}}/></ul>}
    </div>:view==="preview"?<BrainPreview projectId={projectId} path={open.path} brain={brain} disabled={problems.size>0||!brain.nodes.length}/>:<BrainPlaypen projectId={projectId} path={open.path} brain={brain} disabled={problems.size>0||!brain.nodes.length} objects={objects} agentGroup={previewGroups(brain).agents} targetGroup={previewGroups(brain).targets}/>}
    {view==="tree"&&<textarea className="brain-description" aria-label="Brain description" maxLength={500} placeholder="What this brain is for (shown in the library and to the AI)…" value={brain.description} onChange={event=>change({...brain,description:event.target.value})}/>}
    <div className="brain-chat">
     <ProjectChat projectId={projectId} projectName={projectName} scene={{path:scenePath,name:scenePath.replace(/^.*\//,"").replace(/\.scene\.toml$/,"")}} object={{scope:open.path?`file:${open.path}`:`project:${projectId}`,name:brain.name}} requestContext={scratch?brainCreationContext(brain):undefined} aiSettings={aiSettings} defaultExpanded={false} disabled={busy||(!scratch&&dirty)} allowDraftWhileDisabled onBusy={setChatBusy} onApplied={()=>void afterAI()}/>
     {chatBusy?null:scratch?<p className="transform-note">Describe your brain to the AI to create it from scratch.</p>:dirty&&<p className="transform-note">Save the brain to ask the AI to change it.</p>}
    </div>
   </section>
   <aside className="panel brain-inspector" aria-label="Node inspector">
    <div className="panel-head"><h2>{nodeSpec?.label??"Inspector"}</h2>{nodeSpec&&<span className="kind-badge">{nodeSpec.role}</span>}</div>
    {node&&nodeSpec?<div className="brain-node-form">
     <p className="transform-note">{nodeSpec.description}</p>
     {problems.get(node.id)&&<p className="brain-error" role="alert">{problems.get(node.id)}</p>}
     {nodeSpec.fields.map(field=><label className="component-field" key={field.name} title={field.help}><span>{field.label}{field.unit?` (${field.unit})`:""}</span>
      <FieldInput field={field} label={`${nodeSpec.label} ${field.label.toLowerCase()}`} value={node.fields[field.name]} instances={instances} brains={brainOptions} onChange={value=>change(setNodeField(brain,node.id,field.name,typeof value==="number"&&!Number.isFinite(value)?undefined:value))}/>
      <button type="button" className="component-reset" aria-label={`Reset ${field.label}`} disabled={node.fields[field.name]===undefined} onClick={()=>change(setNodeField(brain,node.id,field.name,undefined))}>↺</button></label>)}
     {catalogue&&brain.nodes.length>1&&<label className="settings-field"><span>Move under</span><select aria-label="Move under" value={node.parent??""} disabled={!node.parent} onChange={event=>attempt(()=>moveNode(brain,catalogue,node.id,event.target.value))}>{!node.parent&&<option value="">(root)</option>}{brain.nodes.filter(item=>item.id!==node.id&&["composite","decorator"].includes(spec(item.type)?.role??"")).map(item=><option key={item.id} value={item.id}>{(item.fields.label as string|undefined)??spec(item.type)?.label} · {item.id}</option>)}</select></label>}
     <button type="button" className="quiet-button brain-delete" onClick={()=>{change(removeNode(brain,node.id));setSelected(node.parent??null);}}>Delete {childrenOf(brain,node.id).length?"node and its children":"node"}</button>
    </div>:<p className="transform-note">Select a node to edit its settings. Prefer group names for targets: a brain works in every scene, while a named object only exists in <b>{scenePath.replace(/^.*\//,"")}</b>.</p>}
    <section className="brain-apply" aria-label="Save brain">
     <button type="button" className="settings-save" disabled={busy||chatBusy||!dirty||!brain.nodes.length||problems.size>0||!brain.name.trim()} title={problems.size?"Fix the marked nodes first.":undefined} onClick={()=>void save()}>{busy?"Validating…":"Save brain"}</button>
     <p className="transform-note">{problems.size?"Fix the marked nodes to save. ":""}Saved brains belong to the project: give one to any object with the Brain component in the scene inspector.</p>
    </section>
   </aside>
  </div>
 </main>;
}

function TreeNode({node,depth,brain,catalogue,selected,problems,onSelect,onEdit,onAdd}:{node:BrainNode;depth:number;brain:BrainDocument;catalogue:NodeCatalogue|null;selected:string|null;problems:Map<string,string>;onSelect:(id:string)=>void;onEdit:(edit:()=>BrainDocument)=>void;onAdd:(type:string,parent:string)=>void}){
 const spec=catalogue?.nodes.find(item=>item.id===node.type),children=childrenOf(brain,node.id);
 const label=(node.fields.label as string|undefined)??spec?.label??node.type,weight=node.fields.weight;
 return <li>
  <div role="button" tabIndex={0} className={`brain-node${selected===node.id?" selected":""}${problems.has(node.id)?" problem":""}`} data-role={spec?.role??"steering"} data-node={node.id} aria-label={`${label} node`} aria-pressed={selected===node.id}
   draggable={!!node.parent} onDragStart={event=>event.dataTransfer.setData("application/x-brain-move",node.id)} onDragOver={event=>event.preventDefault()}
   onDrop={event=>{event.preventDefault();event.stopPropagation();const move=event.dataTransfer.getData("application/x-brain-move"),type=event.dataTransfer.getData("application/x-brain-node");if(move&&catalogue&&move!==node.id)onEdit(()=>moveNode(brain,catalogue,move,node.id));else if(type)onAdd(type,node.id);}}
   onClick={event=>{event.stopPropagation();onSelect(node.id);}} onKeyDown={event=>{if(event.key==="Enter"||event.key===" "){event.preventDefault();onSelect(node.id);}}}>
   <span className="brain-node-kind">{spec?.label??node.type}{typeof weight==="number"&&weight!==1?<em> ×{weight}</em>:null}</span>
   {label!==(spec?.label??node.type)&&<b>{label}</b>}
   <small>{nodeSummary(node,spec)||" "}</small>
   {problems.has(node.id)&&<i title={problems.get(node.id)}>!</i>}
   {node.parent&&<span className="brain-node-order"><button type="button" aria-label={`Move ${label} earlier`} onClick={event=>{event.stopPropagation();onEdit(()=>shiftNode(brain,node.id,-1));}}>‹</button><button type="button" aria-label={`Move ${label} later`} onClick={event=>{event.stopPropagation();onEdit(()=>shiftNode(brain,node.id,1));}}>›</button></span>}
  </div>
  {children.length>0&&depth<32&&<ul>{children.map(child=><TreeNode key={child.id} node={child} depth={depth+1} brain={brain} catalogue={catalogue} selected={selected} problems={problems} onSelect={onSelect} onEdit={onEdit} onAdd={onAdd}/>)}</ul>}
 </li>;
}

function BrainPreview({projectId,path,brain,disabled}:{projectId:string;path?:string;brain:BrainDocument;disabled:boolean}){
 const guessed=previewGroups(brain);
 const usesCover=brain.nodes.some(node=>node.type==="hide"||node.type==="line_of_sight"||node.fields.line_of_sight===true);
 const [options,setOptions]=useState({agents:20,targets:1,obstacles:usesCover?8:0,targetSpeed:3,seconds:20,maxSpeed:3,seed:1,agentGroup:guessed.agents,targetGroup:guessed.targets});
 const [trace,setTrace]=useState<BrainTrace|null>(null),[running,setRunning]=useState(false),[error,setError]=useState(""),[playing,setPlaying]=useState(true);
 const canvas=useRef<HTMLCanvasElement>(null),frame=useRef(0);
 async function run(){
  setRunning(true);setError("");
  try{const response=await fetch("/api/brains/preview",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path,brain,options})});const text=await response.text();if(!response.ok)throw new Error(JSON.parse(text).error??"Preview failed.");setTrace(parseTrace(text));frame.current=0;setPlaying(true);}
  catch(failure){setError(failure instanceof Error?failure.message:"Preview failed.");}finally{setRunning(false);}
 }
 useEffect(()=>{
  if(!trace)return;let animation=0,last=performance.now(),carry=0;
  const draw=(now:number)=>{
   const context=canvas.current?.getContext("2d");if(!context)return;
   if(playing){carry+=(now-last)/1000;while(carry>=trace.frameSeconds){carry-=trace.frameSeconds;frame.current=Math.min(frame.current+1,trace.frames.length-1);}}
   last=now;
   const {width,height}=context.canvas,points=trace.frames[frame.current],previous=trace.frames[Math.max(0,frame.current-1)];
   let extent=8;for(let i=0;i<points.length;i+=3)extent=Math.max(extent,Math.abs(points[i])+2,Math.abs(points[i+2])+2);
   const scale=Math.min(width,height)/2/extent,x=(v:number)=>width/2+v*scale,z=(v:number)=>height/2+v*scale;
   context.fillStyle="#0d1510";context.fillRect(0,0,width,height);context.strokeStyle="#1f2e24";context.lineWidth=1;
   for(let g=-Math.floor(extent);g<=extent;g+=2){context.beginPath();context.moveTo(x(g),0);context.lineTo(x(g),height);context.moveTo(0,z(g));context.lineTo(width,z(g));context.stroke();}
   context.fillStyle="#3a4a40";context.strokeStyle="#6f8a78";
   for(const o of trace.obstacles){const half=o.radius/Math.SQRT2*scale;context.fillRect(x(o.x)-half,z(o.z)-half,half*2,half*2);context.strokeRect(x(o.x)-half,z(o.z)-half,half*2,half*2);}
   for(let i=0;i<trace.agents+trace.targets;i++){
    const px=points[i*3],pz=points[i*3+2],dx=px-previous[i*3],dz=pz-previous[i*3+2],angle=Math.atan2(dz,dx);
    context.save();context.translate(x(px),z(pz));
    if(i<trace.agents){context.rotate(angle);context.fillStyle="#9fe0b0";context.beginPath();context.moveTo(7,0);context.lineTo(-5,4);context.lineTo(-5,-4);context.closePath();context.fill();}
    else{context.fillStyle="#f08a6c";context.beginPath();context.arc(0,0,6,0,Math.PI*2);context.fill();}
    context.restore();
   }
   context.fillStyle="#9fb9a8";context.font="11px monospace";context.fillText(`${(frame.current*trace.frameSeconds).toFixed(1)} s · ${trace.agents} ${options.agentGroup} · ${trace.targets} ${options.targetGroup}`,10,18);
   if(playing&&frame.current<trace.frames.length-1)animation=requestAnimationFrame(draw);
  };
  animation=requestAnimationFrame(draw);return()=>cancelAnimationFrame(animation);
 },[trace,playing,options.agentGroup,options.targetGroup]);
 const number=(key:"agents"|"targets"|"obstacles"|"targetSpeed"|"seconds"|"maxSpeed"|"seed",label:string,min:number,max:number,step=1)=><label className="settings-field" key={key}><span>{label}</span><input type="number" aria-label={`Preview ${label.toLowerCase()}`} min={min} max={max} step={step} value={options[key]} onChange={event=>setOptions({...options,[key]:event.target.valueAsNumber})}/></label>;
 return <div className="brain-preview">
  <div className="brain-preview-controls">
   {number("agents","Agents",1,256)}
   <label className="settings-field"><span>Agent group</span><input aria-label="Preview agent group" value={options.agentGroup} maxLength={64} onChange={event=>setOptions({...options,agentGroup:event.target.value})}/></label>
   {number("targets","Targets",0,32)}
   <label className="settings-field"><span>Target group</span><input aria-label="Preview target group" value={options.targetGroup} maxLength={64} onChange={event=>setOptions({...options,targetGroup:event.target.value})}/></label>
   {number("obstacles","Obstacles",0,64)}{number("targetSpeed","Target speed",0,20,0.5)}{number("maxSpeed","Agent speed",0.1,50,0.5)}{number("seconds","Seconds",1,30)}{number("seed","Seed",1,1000000)}
   <button type="button" className="settings-save" disabled={disabled||running} onClick={()=>void run()}>{running?"Simulating…":"Run in engine"}</button>
   {trace&&<button type="button" className="quiet-button" onClick={()=>{frame.current=0;setPlaying(true);setTrace({...trace});}}>Replay</button>}
   {trace&&<button type="button" className="quiet-button" onClick={()=>setPlaying(value=>!value)}>{playing?"Pause":"Play"}</button>}
  </div>
  {error&&<p className="brain-error" role="alert">{error}</p>}
  <canvas ref={canvas} width={900} height={560} className="brain-preview-canvas" aria-label="Brain preview, top-down" data-frames={trace?.frames.length??0}/>
  <p className="transform-note">Top-down view of the Rust runtime: green arrows run this brain (group <b>{options.agentGroup}</b>); red dots are the <b>{options.targetGroup}</b> group circling the arena; grey boxes are obstacles that block sight and give cover. Objects named in the brain are absent here, so nodes that target them stay idle.</p>
 </div>;
}
