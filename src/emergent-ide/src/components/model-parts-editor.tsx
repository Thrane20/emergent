"use client";
import Link from "next/link";
import {useRouter} from "next/navigation";
import {useCallback,useEffect,useMemo,useState} from "react";
import * as THREE from "three";
import {GLTFLoader} from "three/addons/loaders/GLTFLoader.js";
import {GLTFExporter} from "three/addons/exporters/GLTFExporter.js";
import {FalBalance} from "./fal-balance";
import {ModelPartsViewport} from "./model-parts-viewport";
import {useEditorNavigationGuard} from "./editor-navigation";
import {center,cut,merge,partsFromScene,recentred,wouldCycle,type Axis,type EditorPart} from "@/lib/model-part-geometry";
import {cutPartsWithBox,type CutBox} from "@/lib/model-box-cut";
import type {GeneratedModel} from "@/lib/model-parts";
import type {SplitJob} from "@/lib/model-split";

type Props={projectId:string;projectName:string;models:GeneratedModel[];scenes:{path:string;name:string}[];configured:boolean;initialModel?:string};
const AXES=["X","Y","Z"] as const;
const active=(job:SplitJob)=>["uploading","submitting","queued","splitting","downloading"].includes(job.state);

async function json<T>(response:Response){const value=await response.json();if(!response.ok)throw new Error(value.error??"Request failed.");return value as T;}

export function ModelPartsEditor({projectId,projectName,models,scenes,configured,initialModel}:Props){
 const router=useRouter(),[descriptor,setDescriptor]=useState(initialModel),model=models.find(item=>item.descriptor===descriptor);
 const [parts,setParts]=useState<EditorPart[]>([]),[selected,setSelected]=useState<Set<string>>(new Set()),[focus,setFocus]=useState<string|null>(null);
 const [cutting,setCutting]=useState<{axis:Axis;offset:number}|null>(null),[dirty,setDirty]=useState(false),[busy,setBusy]=useState(""),[error,setError]=useState(""),[status,setStatus]=useState("");
 const [boxMode,setBoxMode]=useState(false),[cutBox,setCutBox]=useState<CutBox|null>(null);
 const [jobs,setJobs]=useState<SplitJob[]>([]),[endpoint,setEndpoint]=useState("tripo3d/tripo/segment"),[level,setLevel]=useState("medium"),[endpoints,setEndpoints]=useState<{id:string;label:string}[]>([]);
 const [scene,setScene]=useState(scenes[0]?.path??""),[placed,setPlaced]=useState<{scenePath:string;instance:string}|null>(null);
 const asset=useCallback((path:string)=>`/api/project-asset?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}`,[projectId]);
 useEditorNavigationGuard(()=>!busy&&(!dirty||window.confirm("Discard unsaved part changes?")));

 const run=useCallback(async(label:string,work:()=>Promise<void>)=>{setBusy(label);setError("");setStatus("");try{await work();}catch(caught){setError(caught instanceof Error?caught.message:"Something went wrong.");}finally{setBusy("");}},[]);
 const show=useCallback((next:EditorPart[],changed:boolean)=>{setParts(next);setSelected(new Set());setFocus(next[0]?.id??null);setCutting(null);setBoxMode(false);setCutBox(null);setDirty(changed);},[]);
 const loadGlb=useCallback(async(url:string)=>partsFromScene((await new GLTFLoader().loadAsync(url)).scene,()=>crypto.randomUUID()),[]);

 // A saved part set reopens as it was saved; otherwise the whole model is one part.
 const open=useCallback((item:GeneratedModel)=>run("Loading model…",async()=>{
  if(item.saved.length){
   const loaded=await Promise.all(item.saved.map(async saved=>{const [part]=await loadGlb(asset(saved.asset.replace(/\.entity\.toml$/,".glb")));part.geometry.translate(...saved.pivot);return {...part,id:saved.id,name:saved.name,parent:saved.parent,pivot:saved.pivot};}));
   show(loaded,false);
  }else{const loaded=await loadGlb(asset(item.model));show(loaded.length===1?[{...loaded[0],name:item.name.slice(0,64)}]:loaded,false);}
 }),[asset,loadGlb,run,show]);

 const refreshJobs=useCallback(async()=>{if(!model)return;const data=await json<{jobs:SplitJob[];endpoints:{id:string;label:string}[]}>(await fetch(`/api/models?projectId=${encodeURIComponent(projectId)}&model=${encodeURIComponent(model.model)}`,{cache:"no-store"}));setJobs(data.jobs);setEndpoints(data.endpoints);},[model,projectId]);
 // eslint-disable-next-line react-hooks/set-state-in-effect -- Opening the chosen model loads its parts from the project.
 useEffect(()=>{if(model){void open(model);void refreshJobs().catch(()=>undefined);}else setParts([]);},[descriptor]); // eslint-disable-line react-hooks/exhaustive-deps
 // Poll only while a paid split is in flight.
 useEffect(()=>{const pending=jobs.find(active);if(!pending)return;const timer=setTimeout(()=>{void fetch("/api/models",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,action:"refresh",id:pending.id})}).then(()=>refreshJobs()).catch(()=>undefined);},4000);return ()=>clearTimeout(timer);},[jobs,projectId,refreshJobs]);

 const focused=parts.find(part=>part.id===focus);
 const bounds=useMemo(()=>{if(!focused)return null;const box=new THREE.Box3().setFromBufferAttribute(focused.geometry.getAttribute("position") as THREE.BufferAttribute);return box;},[focused]);
 const update=(id:string,change:Partial<EditorPart>)=>{setParts(all=>all.map(part=>part.id===id?{...part,...change}:part));setDirty(true);};
 function pick(id:string|null,add:boolean){if(!id){setSelected(new Set());return;}setFocus(id);setSelected(previous=>{const next=new Set(add?previous:[]);if(add&&next.has(id))next.delete(id);else next.add(id);return next;});}
 function startCut(axis:Axis){if(!bounds)return;setBoxMode(false);setCutBox(null);setCutting({axis,offset:(bounds.min.getComponent(axis)+bounds.max.getComponent(axis))/2});}
 function applyCut(){
  if(!focused||!cutting)return;const halves=cut(focused.geometry,cutting.axis,cutting.offset);
  if(!halves){setError("The plane does not cross this part. Move it inside the part.");return;}
  const [low,high]=halves.map((geometry,i)=>({...focused,id:i?crypto.randomUUID():focused.id,name:i?`${focused.name} ${AXES[cutting.axis]} high`.slice(0,64):focused.name,geometry,pivot:center(geometry),parent:i?focused.id:focused.parent}));
  // The new half hangs under the original, so a rotor cut from a body stays attached to it.
  setParts(all=>all.flatMap(part=>part.id===focused.id?[low,high]:[part]));focused.geometry.dispose();setSelected(new Set([high.id]));setFocus(high.id);setCutting(null);setDirty(true);setStatus(`Cut ${focused.name} in two.`);
 }
 function applyBoxCut(){
  if(!cutBox)return;
  try{
   const next=cutPartsWithBox(parts,cutBox);
   if(!next){setError("The box does not remove any surface. Move or resize it to intersect the model.");return;}
   const retained=new Set(next.map(part=>part.geometry));
   for(const part of parts)if(!retained.has(part.geometry))part.geometry.dispose();
   const ids=new Set(next.map(part=>part.id));
   setParts(next);setSelected(new Set([...selected].filter(id=>ids.has(id))));setFocus(focus&&ids.has(focus)?focus:next[0].id);
   setCutBox(null);setBoxMode(false);setDirty(true);setError("");setStatus("Removed the surfaces inside the box. Save to Object Library to keep the cut.");
  }catch(caught){setError(caught instanceof Error?caught.message:"Box cut failed.");}
 }
 function editBox(axis:number,kind:"centre"|"size",value:number){
  if(!cutBox||!Number.isFinite(value)||(kind==="size"&&value<=0))return;
  const min=[...cutBox.min] as [number,number,number],max=[...cutBox.max] as [number,number,number];
  const centre=(min[axis]+max[axis])/2,half=(max[axis]-min[axis])/2;
  min[axis]=kind==="centre"?value-half:centre-value/2;max[axis]=kind==="centre"?value+half:centre+value/2;
  setCutBox({...cutBox,min,max});
 }
 function applyMerge(){
  const chosen=parts.filter(part=>selected.has(part.id));if(chosen.length<2)return;
  try{
   const geometry=merge(chosen),keep=chosen.find(part=>part.id===focus)??chosen[0],removed=new Set(chosen.map(part=>part.id).filter(id=>id!==keep.id));
   setParts(all=>all.filter(part=>!removed.has(part.id)).map(part=>part.id===keep.id?{...part,geometry,pivot:center(geometry)}:removed.has(part.parent??"")?{...part,parent:keep.id}:part));
   setSelected(new Set([keep.id]));setFocus(keep.id);setDirty(true);setStatus(`Merged ${chosen.length} parts into ${keep.name}.`);
  }catch(caught){setError(caught instanceof Error?caught.message:"Merge failed.");}
 }
 const save=()=>model&&run("Saving to Object Library…",async()=>{
  const form=new FormData(),exporter=new GLTFExporter();
  form.set("projectId",projectId);form.set("descriptor",model.descriptor);form.set("parts",JSON.stringify(parts.map(({id,name,parent,pivot})=>({id,name,parent,pivot}))));
  for(const part of parts){
   const geometry=recentred(part),mesh=new THREE.Mesh(geometry,part.material);mesh.name=part.name;
   const glb=await exporter.parseAsync(mesh,{binary:true}) as ArrayBuffer;geometry.dispose();
   form.set(`part:${part.id}`,new Blob([glb],{type:"model/gltf-binary"}),`${part.id}.glb`);
  }
  await json(await fetch("/api/models/parts",{method:"POST",body:form}));
  setDirty(false);setStatus(`Saved ${parts.length} part${parts.length===1?"":"s"} to Object Library.`);router.refresh();
 });
 const place=()=>model?.parts&&run("Placing in scene…",async()=>{const result=await json<{scenePath:string;instance:string}>(await fetch("/api/models/place",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,parts:model.parts,scenePath:scene})}));setPlaced(result);setStatus("Added to the scene.");});
 const split=()=>model&&run("Submitting split…",async()=>{await json(await fetch("/api/models",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,action:"split",id:crypto.randomUUID(),model:model.model,endpoint,level})}));await refreshJobs();});
 const latestJob=jobs[0];

 return <main className="object-shell model-parts-shell">
  <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Model Parts</b></span><Link className="quiet-button" href="/objects">Object Creator</Link><Link className="quiet-button" href="/">Project map ↗</Link><FalBalance/></header>
  <div className="object-heading"><div><span className="eyebrow">MODELS / PARTS / MOTION</span><h1>Model Parts</h1></div></div>
  <div className="model-parts-workspace">
   <aside className="panel model-parts-models" aria-label="Generated models"><h2>Models</h2>
    {models.length?models.map(item=><button key={item.descriptor} type="button" className={item.descriptor===descriptor?"object-history-item selected":"object-history-item"} disabled={!!busy} onClick={()=>{if(item.descriptor===descriptor||(dirty&&!window.confirm("Discard unsaved part changes?")))return;setPlaced(null);setDescriptor(item.descriptor);}}><span className="texture-history-icon">◇</span><span><b>{item.name}</b><small>{item.parts?"Parts saved":"Single model"}</small></span></button>):<p className="object-muted">No generated models yet. <Link href="/objects">Create one ↗</Link></p>}
   </aside>
   <section className="panel model-parts-stage" aria-label="Model preview"><div className="panel-head"><h2>{model?.name??"No model"}</h2>{dirty&&<span className="kind-badge">UNSAVED</span>}</div><ModelPartsViewport parts={parts} selected={selected} active={focus} cutPlane={cutting} boxMode={boxMode} cutBox={cutBox} onCutBox={setCutBox} onCancelBox={()=>{setBoxMode(false);setCutBox(null);}} onPick={pick}/></section>
   <aside className="panel model-parts-tools" aria-label="Part tools">
    <section aria-labelledby="split-title"><h3 id="split-title">Split with AI</h3>
     <label className="settings-field"><span>Splitter</span><select aria-label="Part splitter" value={endpoint} disabled={!!busy} onChange={event=>setEndpoint(event.target.value)}>{(endpoints.length?endpoints:[{id:"tripo3d/tripo/segment",label:"Tripo Segment · named parts"}]).map(item=><option key={item.id} value={item.id}>{item.label}</option>)}</select></label>
     {endpoint==="hitem3d/hi3d/split"&&<label className="settings-field"><span>Granularity</span><select aria-label="Split granularity" value={level} disabled={!!busy} onChange={event=>setLevel(event.target.value)}><option value="low">Low · few parts</option><option value="medium">Medium</option><option value="high">High · many parts</option></select></label>}
     <button type="button" className="settings-save" disabled={!model||!configured||!!busy||jobs.some(active)} onClick={()=>void split()}>✦ Split into parts</button>
     {!configured&&<Link className="object-link" href="/settings#object-generation">Connect fal to split models →</Link>}
     {latestJob&&<div className="model-split-job" role="status"><span className="kind-badge">{latestJob.state.toUpperCase()}</span>{latestJob.error&&<p className="object-error">{latestJob.error}</p>}{latestJob.state==="ready"&&<button type="button" className="quiet-button" disabled={!!busy} onClick={()=>{if(dirty&&!window.confirm("Replace the current parts with the AI split?"))return;void run("Loading split…",async()=>show(await loadGlb(`/api/models/split?projectId=${encodeURIComponent(projectId)}&id=${encodeURIComponent(latestJob.id)}`),true));}}>Load split parts</button>}</div>}
    </section>
    <section aria-labelledby="parts-title"><h3 id="parts-title">Parts <span className="kind-badge">{parts.length}</span></h3>
     <ul className="model-parts-list">{parts.map(part=><li key={part.id} className={part.id===focus?"focused":undefined}>
      <input type="checkbox" aria-label={`Select ${part.name}`} checked={selected.has(part.id)} onChange={()=>pick(part.id,true)}/>
      <input aria-label={`${part.name} name`} value={part.name} maxLength={64} onFocus={()=>setFocus(part.id)} onChange={event=>update(part.id,{name:event.target.value})}/>
      <select aria-label={`${part.name} parent`} value={part.parent??""} onChange={event=>update(part.id,{parent:event.target.value||null})}><option value="">No parent</option>{parts.filter(other=>other.id!==part.id&&!wouldCycle(parts,part.id,other.id)).map(other=><option key={other.id} value={other.id}>{other.name}</option>)}</select>
     </li>)}</ul>
     <button type="button" className="quiet-button" disabled={selected.size<2||!!busy} onClick={applyMerge}>Merge selected</button>
    </section>
    {focused&&<section aria-labelledby="focus-title"><h3 id="focus-title">{focused.name}</h3>
     <div className="model-part-pivot"><span>Pivot (m)</span>{AXES.map((axis,i)=><label key={axis}>{axis}<input type="number" step="0.01" aria-label={`Pivot ${axis}`} value={Number(focused.pivot[i].toFixed(4))} onChange={event=>{const pivot=[...focused.pivot] as [number,number,number];pivot[i]=Number(event.target.value)||0;update(focused.id,{pivot});}}/></label>)}<button type="button" className="quiet-button" onClick={()=>update(focused.id,{pivot:center(focused.geometry)})}>Centre</button></div>
     <div className="model-part-cut"><span>Cut with a plane</span><div role="group" aria-label="Cut axis">{AXES.map((axis,i)=><button key={axis} type="button" className="quiet-button" aria-pressed={cutting?.axis===i} onClick={()=>startCut(i as Axis)}>{axis}</button>)}</div>
      {cutting&&bounds&&<><input type="range" aria-label="Cut position" min={bounds.min.getComponent(cutting.axis)} max={bounds.max.getComponent(cutting.axis)} step={(bounds.max.getComponent(cutting.axis)-bounds.min.getComponent(cutting.axis))/200||0.001} value={cutting.offset} onChange={event=>setCutting({...cutting,offset:Number(event.target.value)})}/><div className="project-switcher-row"><button type="button" className="settings-save" onClick={applyCut}>Cut part</button><button type="button" className="quiet-button" onClick={()=>setCutting(null)}>Cancel</button></div></>}
     </div>
    </section>}
    <section className="model-box-tools" aria-labelledby="box-cut-title"><h3 id="box-cut-title">Cut with box</h3>
     <button type="button" className="quiet-button" aria-pressed={boxMode} disabled={!parts.length||!!busy} onClick={()=>{setBoxMode(!boxMode);setCutBox(null);setCutting(null);setError("");}}>Cut with box</button>
     {boxMode&&<><p className="object-muted">Drag a rectangle over the model. Depth starts equal to width. Orbit to another view and drag a corner to resize in 3D. This removes surfaces from all intersected parts and leaves open edges.</p>
      {cutBox&&<>{(["centre","size"] as const).map(kind=><div className="model-box-values" key={kind}><span>Box {kind} (local m)</span>{AXES.map((axis,i)=><label key={axis}>{axis}<input type="number" step="0.01" min={kind==="size"?0.000001:undefined} aria-label={`Box ${kind} ${axis}`} value={Number((kind==="centre"?(cutBox.min[i]+cutBox.max[i])/2:cutBox.max[i]-cutBox.min[i]).toFixed(6))} onChange={event=>editBox(i,kind,event.target.valueAsNumber)}/></label>)}</div>)}</>}
      <div className="project-switcher-row">{cutBox&&<button type="button" className="quiet-button" onClick={()=>setCutBox(null)}>Redraw box</button>}<button type="button" className="settings-save" disabled={!cutBox||!!busy} onClick={applyBoxCut}>Cut</button><button type="button" className="quiet-button" onClick={()=>{setBoxMode(false);setCutBox(null);}}>Cancel box cut</button></div>
     </>}
    </section>
    <section aria-labelledby="save-title"><h3 id="save-title">Save and place</h3>
     <p className="object-muted">Save the complete model and its part hierarchy to the project Object Library.</p>
     <Link href="/object-builder" onNavigate={event=>{if(busy||(dirty&&!window.confirm("Discard unsaved part changes?")))event.preventDefault();}}>Open Object Library ↗</Link>
     <button type="button" className="settings-save" disabled={!model||!parts.length||!!busy} onClick={()=>void save()}>{busy==="Saving to Object Library…"?busy:"Save To Object Library"}</button>
     <label className="settings-field"><span>Scene</span><select aria-label="Destination scene" value={scene} disabled={!!busy} onChange={event=>setScene(event.target.value)}>{scenes.map(item=><option key={item.path} value={item.path}>{item.name}</option>)}</select></label>
     <button type="button" className="quiet-button" disabled={!model?.parts||dirty||!scene||!!busy} title={dirty?"Save to Object Library first":undefined} onClick={()=>void place()}>Place in scene</button>
     {placed&&<Link href={`/?scene=${encodeURIComponent(placed.scenePath)}&instance=${encodeURIComponent(placed.instance)}`}>View in scene ↗</Link>}
    </section>
    {busy&&<p role="status">{busy}</p>}{error&&<p role="alert" className="object-error">{error}</p>}{status&&<p role="status">{status}</p>}
   </aside>
  </div>
 </main>;
}
