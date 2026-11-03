"use client";
import Link from "next/link";
import {useCallback,useEffect,useRef,useState} from "react";
import {AssetCarousel} from "@/components/asset-carousel";
import {FalBalance} from "@/components/fal-balance";
import {FieldInput} from "@/components/field-input";
import {ProjectChat} from "@/components/project-chat";
import {useEditorNavigationGuard} from "./editor-navigation";
import type {PublicAISettings} from "@/lib/ai-settings";
import {changeType,parseLocomotionTrace,type LocomotionCatalogue,type LocomotionFrame,type LocomotionProfile,type LocomotionSummary,type LocomotionTrace,type LocomotionType} from "@/lib/locomotion";

type Open={path?:string;revision?:string;profile:LocomotionProfile};
const blank=():Open=>({profile:{id:crypto.randomUUID(),name:"New locomotion",description:"",type:"slide",fields:{}}});
const REST:LocomotionFrame={x:0,y:0,z:0,yaw:0,pitch:0,bank:0,speed:0,steer:0,throttle:0,brake:0,rotor:0.5,thrust:0,fuel:1,landed:false};
const deg=(radians:number)=>Math.round(radians*180/Math.PI);

/** Schematic of a type's actuators, driven by the activity of one frame. */
export function RigView({rig,frame=REST,size="large"}:{rig:LocomotionType["rig"];frame?:LocomotionFrame;size?:"large"|"thumb"}){
 const stroke="#9fb9a8",body="#2a3a31",accent="#8fd4a3";
 const shape=(()=>{switch(rig){
  case "wheels":return <g>
   <rect x="78" y="22" width="44" height="76" rx="9" fill={body} stroke={stroke}/>
   {[[70,34,true],[130,34,true],[70,86,false],[130,86,false]].map(([x,y,front])=><rect key={`${x}${y}`} x={Number(x)-5} y={Number(y)-10} width="10" height="20" rx="2" fill="#151d18" stroke={accent} transform={front?`rotate(${-deg(frame.steer)} ${x} ${y})`:undefined}/>)}
   <rect x="84" y="94" width="10" height="4" fill={frame.brake>0?"#ff5a4a":"#5a2a26"}/><rect x="106" y="94" width="10" height="4" fill={frame.brake>0?"#ff5a4a":"#5a2a26"}/>
   <path d="M100 30 L94 40 H106 Z" fill={accent}/>
  </g>;
  case "rotors":return <g transform={`translate(100 60) rotate(${-deg(frame.yaw)})`}>
   <path d="M-34 -34 L34 34 M34 -34 L-34 34" stroke={stroke} strokeWidth="5"/>
   {[[-34,-34],[34,-34],[-34,34],[34,34]].map(([x,y])=><circle key={`${x}${y}`} cx={x} cy={y} r="16" fill={accent} fillOpacity={0.15+0.75*frame.rotor} stroke={accent}/>)}
   <rect x="-12" y="-12" width="24" height="24" rx="4" fill={body} stroke={stroke}/>
   <line x1="0" y1="0" x2={Math.sin(frame.bank)*40} y2={Math.sin(frame.pitch)*40} stroke="#e0c27a" strokeWidth="3"/>
  </g>;
  case "wing":return <g transform={`translate(100 62) rotate(${deg(frame.bank)})`}>
   <line x1="-80" y1="0" x2="80" y2="0" stroke={stroke} strokeWidth="6" strokeLinecap="round"/>
   <circle cx="0" cy="0" r="12" fill={body} stroke={stroke}/>
   <line x1="0" y1="-12" x2="0" y2="-30" stroke={stroke} strokeWidth="4"/>
   <line x1="-26" y1="0" x2="26" y2="0" stroke={accent} strokeWidth="3" transform={`translate(0 ${-deg(frame.pitch)/3})`}/>
  </g>;
  case "projectile":return <g transform={`translate(100 62) rotate(${-deg(frame.pitch)})`}>
   {frame.thrust>0&&<path d="M-46 -6 L-74 0 L-46 6 Z" fill="#f2a14b"/>}
   <path d="M-46 -8 H30 L50 0 L30 8 H-46 Z" fill={body} stroke={stroke}/>
   <path d="M-46 -8 L-56 -18 L-40 -8 M-46 8 L-56 18 L-40 8" fill={accent} stroke={accent}/>
  </g>;
  case "legs":return <g><circle cx="100" cy="30" r="12" fill={body} stroke={stroke}/><path d="M100 42 V72 M100 72 L84 104 M100 72 L116 104 M100 52 L80 66 M100 52 L120 66" stroke={stroke} strokeWidth="4" fill="none"/></g>;
  default:return <g transform={`translate(100 60) rotate(${-deg(frame.yaw)})`}><circle r="28" fill={body} stroke={stroke}/><path d="M0 -38 L-10 -20 H10 Z" fill={accent}/></g>;
 }})();
 return <svg className={size==="thumb"?"brain-thumbnail":"loco-rig"} viewBox="0 0 200 120" aria-hidden={size==="thumb"} role={size==="thumb"?undefined:"img"} aria-label={size==="thumb"?undefined:`${rig} rig`}>{shape}</svg>;
}

export function LocomotionEditor({projectId,projectName,scenePath,aiSettings,initialProfiles,initial}:{projectId:string;projectName:string;scenePath:string;aiSettings:PublicAISettings;initialProfiles:LocomotionSummary[];initial?:Open}){
 const [catalogue,setCatalogue]=useState<LocomotionCatalogue|null>(null),[profiles,setProfiles]=useState(initialProfiles);
 const [open,setOpen]=useState<Open>(initial??blank()),[dirty,setDirty]=useState(!initial),[busy,setBusy]=useState(false),[error,setError]=useState(""),[status,setStatus]=useState("");
 const [view,setView]=useState<"rig"|"preview">("rig"),[chatBusy,setChatBusy]=useState(false);
 useEditorNavigationGuard(useCallback(()=>!busy&&(!dirty||window.confirm("Discard unsaved locomotion changes?")),[busy,dirty]));
 const profile=open.profile,type=catalogue?.types.find(item=>item.id===profile.type);
 useEffect(()=>{
  let active=true;
  fetch(`/api/locomotion?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"}).then(async response=>{const data=await response.json();if(!response.ok)throw new Error(data.error??"Locomotion types unavailable.");if(active){setCatalogue(data.catalogue);setProfiles(data.profiles);}}).catch(failure=>{if(active)setError(failure instanceof Error?failure.message:"Locomotion types unavailable.");});
  return()=>{active=false;};
 },[projectId]);
 function change(next:LocomotionProfile){setOpen(current=>({...current,profile:next}));setDirty(true);setStatus("");setError("");}
 async function save(){
  setBusy(true);setError("");
  try{
   const response=await fetch("/api/locomotion",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path:open.path,revision:open.revision,profile})});
   const data=await response.json();if(!response.ok)throw new Error(data.error??"Profile could not be saved.");
   setOpen({path:data.path,revision:data.revision,profile:data.profile});setDirty(false);setStatus("Saved and validated by the engine.");
   const list=await fetch(`/api/locomotion?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"}).then(r=>r.json());if(list.profiles)setProfiles(list.profiles);
   window.history.replaceState(null,"",`/locomotion?asset=${encodeURIComponent(data.path)}&scene=${encodeURIComponent(scenePath)}`);
  }catch(failure){setError(failure instanceof Error?failure.message:"Profile could not be saved.");}finally{setBusy(false);}
 }
 async function load(path:string){
  if(dirty&&!window.confirm("Discard unsaved locomotion changes?"))return;
  setBusy(true);setError("");
  try{const response=await fetch(`/api/locomotion?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}`,{cache:"no-store"});const data=await response.json();if(!response.ok)throw new Error(data.error);setOpen(data);setDirty(false);setStatus("");window.history.replaceState(null,"",`/locomotion?asset=${encodeURIComponent(path)}&scene=${encodeURIComponent(scenePath)}`);}
  catch(failure){setError(failure instanceof Error?failure.message:"Profile could not be opened.");}finally{setBusy(false);}
 }
 function newProfile(){if(!dirty||window.confirm("Discard unsaved locomotion changes?")){setOpen(blank());setDirty(true);setStatus("");window.history.replaceState(null,"",`/locomotion?scene=${encodeURIComponent(scenePath)}`);}}
 const rigOf=(id:string)=>catalogue?.types.find(item=>item.id===id)?.rig??"body";
 return <main className="object-shell brain-shell loco-shell">
  <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Locomotion</b></span><Link href={`/?scene=${encodeURIComponent(scenePath)}`} className="quiet-button">Project map ↗</Link><FalBalance/></header>
  <div className="object-heading terrain-library-heading"><div className="terrain-library-title"><span className="eyebrow">MOVEMENT / VEHICLES / FLIGHT</span><h1>Locomotion</h1><p>How objects move: wheels, rotors, wings and projectiles, for the whole project.</p><span className="kind-badge">{catalogue?`${catalogue.types.filter(item=>item.status==="available").length} TYPES`:"LOADING TYPES"}</span></div>
   <AssetCarousel label="Locomotion library" listLabel="Saved locomotion profiles" newLabel="locomotion" selected={open.path} disabled={busy} onSelect={path=>void load(path)} onNew={newProfile}
    items={profiles.map(item=>({path:item.path,name:item.name,error:item.error,title:`${item.name} · ${catalogue?.types.find(t=>t.id===item.type)?.label??item.type}${item.description?`\n${item.description}`:""}`,thumbnail:<RigView rig={rigOf(item.type)} size="thumb"/>}))}/></div>
  <div className="brain-layout">
   <aside className="panel brain-library" aria-label="Type library">
    <div className="panel-head"><h2>Type library</h2></div>
    <p className="brain-hint">Choose how <b>{profile.name||"this profile"}</b> moves.</p>
    <div className="brain-categories">
     {catalogue?.categories.map(category=><details key={category} open>
      <summary>{category}<small>{catalogue.types.filter(item=>item.category===category&&item.status==="available").length}</small></summary>
      {catalogue.types.filter(item=>item.category===category).map(item=><button type="button" key={item.id} className={`brain-library-node${item.id===profile.type?" selected":""}`} data-role="steering" aria-pressed={item.id===profile.type} disabled={item.status!=="available"||busy} title={item.description} onClick={()=>change(changeType(profile,item))}><b>{item.label}</b><small>{item.status==="boundary"?"boundary only":item.status==="planned"?"planned":item.rig}</small></button>)}
     </details>)}
    </div>
   </aside>
   <section className="panel brain-workspace">
    <div className="panel-head"><div className="brain-title"><input aria-label="Profile name" value={profile.name} maxLength={64} onChange={event=>change({...profile,name:event.target.value})}/>{dirty&&<span className="kind-badge">UNSAVED</span>}</div>
     <div className="view-switch">{(["rig","preview"] as const).map(item=><button type="button" key={item} className={view===item?"selected":""} onClick={()=>setView(item)}>{item==="rig"?"Rig":"Preview"}</button>)}</div></div>
    {(error||status)&&<p className={error?"brain-error":"brain-status"} role={error?"alert":"status"}>{error||status}</p>}
    {view==="rig"?<div className="loco-rig-view">
     <RigView rig={type?.rig??"body"}/>
     <div className="loco-manoeuvres"><b>{type?.label??profile.type}</b><span>{type?.description}</span><div>{type?.manoeuvres.map(item=><span className="field-chip" key={item}>{item}</span>)}</div></div>
    </div>:<LocomotionPreview key={profile.type} projectId={projectId} path={open.path} profile={profile} rig={type?.rig??"body"}/>}
    {view==="rig"&&<textarea className="brain-description" aria-label="Profile description" maxLength={500} placeholder="What moves like this (shown in the library and to the AI)…" value={profile.description} onChange={event=>change({...profile,description:event.target.value})}/>}
    <div className="brain-chat">
     <ProjectChat projectId={projectId} projectName={projectName} scene={{path:scenePath,name:scenePath.replace(/^.*\//,"").replace(/\.scene\.toml$/,"")}} object={open.path?{scope:`file:${open.path}`,name:profile.name}:undefined} aiSettings={aiSettings} disabled={dirty||busy} onBusy={setChatBusy} onApplied={()=>{if(open.path)void load(open.path);}}/>
     {chatBusy?null:dirty&&<p className="transform-note">Save the profile to ask the AI to change it.</p>}
    </div>
   </section>
   <aside className="panel brain-inspector" aria-label="Locomotion settings">
    <div className="panel-head"><h2>{type?.label??"Settings"}</h2>{type&&<span className="kind-badge">{type.category}</span>}</div>
    <div className="brain-node-form">
     {type?type.fields.map(field=><label className="component-field" key={field.name} title={field.help}><span>{field.label}{field.unit?` (${field.unit})`:""}</span>
      <FieldInput field={field} label={`${type.label} ${field.label.toLowerCase()}`} value={profile.fields[field.name]} instances={[]} onChange={value=>{const fields={...profile.fields};if(value===""||typeof value==="number"&&!Number.isFinite(value))delete fields[field.name];else fields[field.name]=value;change({...profile,fields});}}/>
      <button type="button" className="component-reset" aria-label={`Reset ${field.label}`} disabled={profile.fields[field.name]===undefined} onClick={()=>{const fields={...profile.fields};delete fields[field.name];change({...profile,fields});}}>↺</button></label>)
     :<p className="transform-note">Loading types…</p>}
    </div>
    <section className="brain-apply" aria-label="Save locomotion profile">
     <button type="button" className="settings-save" disabled={busy||!dirty||!profile.name.trim()||!type} onClick={()=>void save()}>{busy?"Validating…":"Save profile"}</button>
     <p className="transform-note">Profiles belong to the project: give one to any object with the Locomotion component in the scene inspector. Its brain says where to go; the profile decides how.</p>
    </section>
   </aside>
  </div>
 </main>;
}

const COURSES:{id:string;label:string}[]=[{id:"waypoints",label:"Waypoint loop"},{id:"orbit",label:"Orbit a point"},{id:"stop",label:"Go and stop / hover"},{id:"launch",label:"Launch at a target"},{id:"circuit",label:"Circuit and landing (physical aircraft)"}];

function LocomotionPreview({projectId,path,profile,rig}:{projectId:string;path?:string;profile:LocomotionProfile;rig:LocomotionType["rig"]}){
 const projectile=profile.type==="ballistic"||profile.type==="rocket";
 const aircraft=profile.type==="fixed_wing_physics";
 const [course,setCourse]=useState(projectile?"launch":aircraft?"circuit":"waypoints"),[seconds,setSeconds]=useState(aircraft?240:20);
 const [trace,setTrace]=useState<LocomotionTrace|null>(null),[running,setRunning]=useState(false),[error,setError]=useState(""),[frame,setFrame]=useState(0),[playing,setPlaying]=useState(true);
 const top=useRef<HTMLCanvasElement>(null),side=useRef<HTMLCanvasElement>(null);
 // A new trace can arrive while the frame counter still points into a longer one.
 const index=trace?.frames.length?Math.min(frame,trace.frames.length-1):-1;
 async function run(){
  setRunning(true);setError("");
  try{const response=await fetch("/api/locomotion/preview",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path,profile,options:{course,seconds}})});const text=await response.text();if(!response.ok)throw new Error(JSON.parse(text).error??"Preview failed.");setTrace(parseLocomotionTrace(text));setFrame(0);setPlaying(true);}
  catch(failure){setError(failure instanceof Error?failure.message:"Preview failed.");}finally{setRunning(false);}
 }
 useEffect(()=>{
  if(!trace||!playing)return;
  let animation=0,last=performance.now(),carry=0;
  const tick=(now:number)=>{carry+=(now-last)/1000;last=now;const steps=Math.floor(carry/trace.frameSeconds);if(steps){carry-=steps*trace.frameSeconds;setFrame(current=>Math.min(current+steps,trace.frames.length-1));}animation=requestAnimationFrame(tick);};
  animation=requestAnimationFrame(tick);return()=>cancelAnimationFrame(animation);
 },[trace,playing]);
 useEffect(()=>{
  if(!trace||index<0)return;
  const draw=(canvas:HTMLCanvasElement|null,map:(f:LocomotionFrame,i:number)=>[number,number],points:[number,number][])=>{
   const context=canvas?.getContext("2d");if(!context||!canvas)return;
   const all=[...trace.frames.map(map),...points];
   const xs=all.map(p=>p[0]),ys=all.map(p=>p[1]);
   const [minX,maxX,minY,maxY]=[Math.min(...xs),Math.max(...xs),Math.min(...ys),Math.max(...ys)];
   const span=Math.max(maxX-minX,maxY-minY,1)*1.1,cx=(minX+maxX)/2,cy=(minY+maxY)/2,{width,height}=canvas,scale=Math.min(width,height)/span;
   const px=(x:number)=>width/2+(x-cx)*scale,py=(y:number)=>height/2-(y-cy)*scale;
   context.fillStyle="#0d1510";context.fillRect(0,0,width,height);
   context.strokeStyle="#2b3d32";context.lineWidth=1;context.beginPath();trace.frames.forEach((f,i)=>{const [x,y]=map(f,i);if(i)context.lineTo(px(x),py(y));else context.moveTo(px(x),py(y));});context.stroke();
   context.strokeStyle="#8fd4a3";context.lineWidth=2;context.beginPath();trace.frames.slice(0,index+1).forEach((f,i)=>{const [x,y]=map(f,i);if(i)context.lineTo(px(x),py(y));else context.moveTo(px(x),py(y));});context.stroke();
   context.fillStyle="#f08a6c";for(const [x,y] of points){context.beginPath();context.arc(px(x),py(y),4,0,Math.PI*2);context.fill();}
   const [x,y]=map(trace.frames[index],index);context.fillStyle="#fff1aa";context.beginPath();context.arc(px(x),py(y),5,0,Math.PI*2);context.fill();
  };
  // Top-down: x right, z down the screen (as in the scene's top view); height over time beside it.
  draw(top.current,f=>[f.x,-f.z],trace.course.map(p=>[p.x,-p.z]));
  draw(side.current,(f,i)=>[i*trace.frameSeconds,f.y],[]);
 },[trace,index]);
 const now=index>=0?trace?.frames[index]:undefined;
 const readout=now?[["Time",`${(index*(trace?.frameSeconds??0)).toFixed(1)} s`],["Speed",`${now.speed.toFixed(1)} m/s`],["Height",`${now.y.toFixed(1)} m`],["Steer",`${deg(now.steer)}°`],["Throttle",`${Math.round(now.throttle*100)}%`],["Brake",now.brake>0?"on":"off"],["Bank",`${deg(now.bank)}°`],["Pitch",`${deg(now.pitch)}°`],["Rotors",`${Math.round(now.rotor*100)}%`],["Thrust",now.thrust>0?"burning":"off"],["Fuel",`${Math.round(now.fuel*100)}%`],["Landed",now.landed?"yes":"no"],...(now.phase?[["Phase",now.phase]]:[])]:[];
 return <div className="brain-preview loco-preview">
  <div className="brain-preview-controls">
   <label className="settings-field loco-course"><span>Course</span><select aria-label="Preview course" value={course} onChange={event=>setCourse(event.target.value)}>{COURSES.map(item=><option key={item.id} value={item.id}>{item.label}</option>)}</select></label>
   <label className="settings-field"><span>Seconds</span><input type="number" aria-label="Preview seconds" min={1} max={aircraft?300:60} value={seconds} onChange={event=>setSeconds(event.target.valueAsNumber)}/></label>
   <button type="button" className="settings-save" disabled={running} onClick={()=>void run()}>{running?"Simulating…":"Run in engine"}</button>
   {trace&&<button type="button" className="quiet-button" onClick={()=>{setFrame(0);setPlaying(true);}}>Replay</button>}
   {trace&&<button type="button" className="quiet-button" onClick={()=>setPlaying(value=>!value)}>{playing?"Pause":"Play"}</button>}
  </div>
  {error&&<p className="brain-error" role="alert">{error}</p>}
  <div className="loco-preview-views">
   <canvas ref={top} width={560} height={420} className="loco-canvas" aria-label="Locomotion preview, top-down" data-frames={trace?.frames.length??0}/>
   <div className="loco-preview-side">
    <RigView rig={rig} frame={now}/>
    <canvas ref={side} width={320} height={150} className="loco-canvas" aria-label="Height over time"/>
    <dl className="loco-readout">{readout.map(([label,value])=><div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
   </div>
  </div>
  <p className="transform-note">The Rust runtime drives this profile along the course (red dots): top-down track on the left, height over time and the rig&apos;s live activity on the right.</p>
 </div>;
}
