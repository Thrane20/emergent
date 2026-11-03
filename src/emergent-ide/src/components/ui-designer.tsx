"use client";
import Link from "next/link";
import {useCallback,useEffect,useMemo,useRef,useState,type PointerEvent as ReactPointerEvent} from "react";
import {AssetCarousel} from "@/components/asset-carousel";
import {FalBalance} from "@/components/fal-balance";
import {FieldInput} from "@/components/field-input";
import {isEmptyUiDraft,layoutMode,newlyCreatedUiPath,uiCreationContext} from "@/lib/ui";
import {ProjectChat} from "@/components/project-chat";
import {useEditorNavigationGuard} from "./editor-navigation";
import type {PublicAISettings} from "@/lib/ai-settings";
import {childrenOf,removeNode,rootOf,setNodeField,shiftNode} from "@/lib/brains";
import type {ComponentValue} from "@/lib/components";
import {linearToHex} from "@/lib/material-settings";
import {addWidget,moveWidget,parseLength,uiActions,uiLayout,uiProblems,type Rect,type UiDocument,type UiNode,type UiSummary,type WidgetCatalogue} from "@/lib/ui";

type Open={path?:string;revision?:string;ui:UiDocument};
const blank=():Open=>({ui:{id:crypto.randomUUID(),name:"New UI",description:"",canvas:[640,480],nodes:[{id:"root",type:"panel",fields:{padding:10}}]}});
const nodeId=(type:string)=>`${type}_${crypto.randomUUID()}`;
const colour=(value:ComponentValue|undefined,fallback:number[])=>linearToHex(Array.isArray(value)&&value.length===3?value as number[]:fallback);
const num=(value:ComponentValue|undefined,fallback:number)=>typeof value==="number"?value:fallback;
const text=(value:ComponentValue|undefined)=>typeof value==="string"?value:"";

/** Saved UIs as the carousel strip the other editors use; thumbnails are the laid-out boxes. */
function UiThumbnail({ui}:{ui:UiSummary}){
 if(!ui.canvas||!ui.boxes?.length)return <span className="particle-swatch-unavailable">{ui.error?"Invalid UI":"Empty"}</span>;
 return <svg className="brain-thumbnail" viewBox={`0 0 ${ui.canvas[0]} ${ui.canvas[1]}`} preserveAspectRatio="xMidYMid meet" aria-hidden="true">
  {ui.boxes.map((box,i)=><rect key={i} x={box.rect.x} y={box.rect.y} width={box.rect.w} height={box.rect.h} fill={box.type==="panel"?"#0b0f14":"none"} stroke={box.type==="panel"?"#55606a":"#9aa6ae"} strokeWidth={ui.canvas![0]/160}/>)}
 </svg>;
}

export function UiDesigner({projectId,projectName,scenePath,aiSettings,initialUis,initial}:{projectId:string;projectName:string;scenePath:string;aiSettings:PublicAISettings;initialUis:UiSummary[];initial?:Open}){
 const [catalogue,setCatalogue]=useState<WidgetCatalogue|null>(null),[uis,setUis]=useState(initialUis);
 const [open,setOpen]=useState<Open>(initial??blank()),[selected,setSelected]=useState<string|null>(initial?rootOf(initial.ui)?.id??null:"root");
 const [dirty,setDirty]=useState(!initial),[busy,setBusy]=useState(false),[error,setError]=useState(""),[status,setStatus]=useState("");
 const [world,setWorld]=useState(false),[chatBusy,setChatBusy]=useState(false);
 const external=useRef({path:open.path,dirty,busy:busy||chatBusy});
 useEffect(()=>{external.current={path:open.path,dirty,busy:busy||chatBusy};},[open.path,dirty,busy,chatBusy]);
 useEffect(()=>{
  const events=new EventSource(`/api/project-events?projectId=${encodeURIComponent(projectId)}`);
  let active=true;
  events.addEventListener("changed",async message=>{
   try{
    const change=JSON.parse((message as MessageEvent<string>).data) as {paths:string[]};
    if(!change.paths.some(path=>path.endsWith(".ui.toml")))return;
    const list=await fetch(`/api/ui?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"}).then(response=>response.json());
    if(!active)return;if(list.uis)setUis(list.uis);
    const current=external.current;
    if(!current.path||!change.paths.includes(current.path)||current.busy)return;
    if(current.dirty){setError("This UI changed externally. Your unsaved edits are kept; reopen it before saving.");return;}
    const response=await fetch(`/api/ui?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(current.path)}`,{cache:"no-store"}),data=await response.json();
    if(!active||external.current.path!==current.path||external.current.dirty||external.current.busy)return;
    if(!response.ok)throw new Error(data.error??"UI changed externally. Reopen a UI from the library.");
    setOpen(data);setSelected(id=>data.ui.nodes.some((node:UiNode)=>node.id===id)?id:rootOf(data.ui)?.id??null);setStatus("UI refreshed after an external change.");setError("");
   }catch(failure){if(active)setError(failure instanceof Error?failure.message:"UI could not be refreshed.");}
  });
  return()=>{active=false;events.close();};
 },[projectId]);
 const canLeave=useCallback(()=>!busy&&!chatBusy&&(!dirty||window.confirm("Discard unsaved UI changes?")),[busy,chatBusy,dirty]);
 useEditorNavigationGuard(canLeave);
 useEffect(()=>{if(!dirty)return;const warn=(event:BeforeUnloadEvent)=>event.preventDefault();window.addEventListener("beforeunload",warn);return()=>window.removeEventListener("beforeunload",warn);},[dirty]);
 const ui=open.ui,rects=useMemo(()=>{try{return uiLayout(ui);}catch{return new Map<string,Rect>();}},[ui]),problems=useMemo(()=>catalogue?uiProblems(ui,catalogue):new Map<string,string>(),[ui,catalogue]);
 const scratch=isEmptyUiDraft(open.path,ui);
 const spec=(type:string)=>catalogue?.widgets.find(widget=>widget.id===type);
 useEffect(()=>{
  let active=true;
  fetch(`/api/ui?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"}).then(async response=>{const data=await response.json();if(!response.ok)throw new Error(data.error??"Widget catalogue unavailable.");if(active){setCatalogue(data.catalogue);setUis(data.uis);}}).catch(failure=>{if(active)setError(failure instanceof Error?failure.message:"Widget catalogue unavailable.");});
  return()=>{active=false;};
 },[projectId]);
 const url=(path?:string)=>`/ui?${path?`asset=${encodeURIComponent(path)}&`:""}scene=${encodeURIComponent(scenePath)}`;
 function change(next:UiDocument){if(busy||chatBusy)return;setOpen(current=>({...current,ui:next}));setDirty(true);setStatus("");setError("");}
 function attempt(edit:()=>UiDocument){try{change(edit());}catch(failure){setError(failure instanceof Error?failure.message:"That change is not possible.");}}
 function add(type:string){if(!catalogue)return;const id=nodeId(type);attempt(()=>addWidget(ui,catalogue,type,selected??undefined,id));setSelected(id);}
 function newUi(){if(canLeave()){setOpen(blank());setSelected("root");setDirty(true);setStatus("");setError("");window.history.replaceState(null,"",url());}}
 async function save(){
  setBusy(true);setError("");
  try{
   const response=await fetch("/api/ui",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path:open.path,revision:open.revision,ui})});
   const data=await response.json();if(!response.ok)throw new Error(data.error??"UI could not be saved.");
   setOpen({path:data.path,revision:data.revision,ui:data.ui});setDirty(false);setStatus("Saved and validated by the engine.");
   const list=await fetch(`/api/ui?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"}).then(r=>r.json());if(list.uis)setUis(list.uis);
   window.history.replaceState(null,"",url(data.path));
  }catch(failure){setError(failure instanceof Error?failure.message:"UI could not be saved.");}finally{setBusy(false);}
 }
 async function place(space:"screen"|"world"){
  if(!open.path||dirty||busy||chatBusy)return;setBusy(true);setError("");
  try{const response=await fetch("/api/ui",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path:open.path,revision:open.revision,scenePath,space})});const data=await response.json();if(!response.ok)throw new Error(data.error??"UI placement failed.");setStatus(`Added ${space==="screen"?"screen overlay":"world panel"} to the scene. Open the scene to move the panel or Run to render it.`);}catch(failure){setError(failure instanceof Error?failure.message:"UI placement failed.");}finally{setBusy(false);}
 }
 async function load(path:string,applied=false){
  if(!applied&&!canLeave())return;
  setBusy(true);setError("");
  try{const response=await fetch(`/api/ui?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}`,{cache:"no-store"});const data=await response.json();if(!response.ok)throw new Error(data.error);setOpen(data);setSelected(rootOf(data.ui)?.id??null);setDirty(false);setStatus("");window.history.replaceState(null,"",url(path));}
  catch(failure){setError(failure instanceof Error?failure.message:"UI could not be opened.");}finally{setBusy(false);}
 }

 async function afterAI(){
  try{
   const response=await fetch(`/api/ui?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"});
   const data=await response.json();if(!response.ok)throw new Error(data.error??"UI library could not be refreshed.");
   const fresh=data.uis as UiSummary[];setUis(fresh);
   const path=open.path??newlyCreatedUiPath(uis,fresh);
   if(path)await load(path,true);
   else setStatus("UI library refreshed. Select the new UI to open it.");
  }catch(failure){setError(failure instanceof Error?failure.message:"AI changes could not be opened.");}
 }

 const node=ui.nodes.find(item=>item.id===selected),nodeSpec=node?spec(node.type):undefined,actions=uiActions(ui);
 const parentNode=ui.nodes.find(item=>item.id===node?.parent),parentMode=parentNode?layoutMode(parentNode):"absolute";
 const sizeDefaults={w:parentMode==="horizontal"?"24px":"100%",h:parentMode==="vertical"||parentMode==="absolute"&&parentNode?.fields.stack===true?"24px":"100%"};
 const panels=ui.nodes.filter(item=>spec(item.type)?.container&&node&&item.id!==node.id);
 return <main className="object-shell brain-shell ui-shell">
  <header className="settings-topbar"><Link className="brand" href="/" onNavigate={event=>{if(!canLeave())event.preventDefault();}}><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>UI Designer</b></span><Link href={`/?scene=${encodeURIComponent(scenePath)}`} className="quiet-button" onNavigate={event=>{if(!canLeave())event.preventDefault();}}>Project map ↗</Link><FalBalance/></header>
  <div className="object-heading terrain-library-heading"><div className="terrain-library-title"><span className="eyebrow">HUD / MENUS / CONSOLE SCREENS</span><h1>UI Designer</h1><p>Lay out screen- and world-space UIs, label their actions and save them as project assets.</p><span className="kind-badge">{catalogue?`${catalogue.widgets.length} WIDGETS`:"LOADING WIDGETS"}</span></div>
   <AssetCarousel label="UI library" newLabel="UI" selected={open.path} disabled={busy||chatBusy} onSelect={path=>void load(path)} onNew={newUi}
    items={uis.map(item=>({path:item.path,name:item.name,error:item.error,title:`${item.name}${item.description?`\n${item.description}`:""}\n${item.path}`,thumbnail:<UiThumbnail ui={item}/>}))}/></div>
  <div className="brain-layout">
   <aside className="panel brain-library" aria-label="Widgets">
    <div className="panel-head"><h2>Widgets</h2></div>
    <p className="brain-hint">{node?<>Adds into <b>{spec(node.type)?.container?node.id:node.parent??node.id}</b>.</>:"Select a panel to add into."}</p>
    <div className="brain-categories">
     {catalogue?.categories.map(category=><details key={category} open><summary>{category}</summary>
      {catalogue.widgets.filter(widget=>widget.category===category).map(widget=><button type="button" key={widget.id} className="brain-library-node" data-role={widget.container?"composite":"steering"} disabled={busy} title={widget.description} onClick={()=>add(widget.id)}><b>{widget.label}</b><small>{widget.container?"container":"widget"}</small></button>)}
     </details>)}
     <details open><summary>Outline<small>{ui.nodes.length}</small></summary>
      {rootOf(ui)&&<Outline node={rootOf(ui)!} ui={ui} depth={0} selected={selected} problems={problems} onSelect={setSelected} onShift={(id,by)=>change(shiftNode(ui,id,by))}/>}
     </details>
    </div>
   </aside>
   <section className="panel brain-workspace">
    <div className="panel-head"><div className="brain-title"><input aria-label="UI name" value={ui.name} maxLength={64} onChange={event=>change({...ui,name:event.target.value})}/>{dirty&&<span className="kind-badge">UNSAVED</span>}</div>
     <label className="ui-canvas-size">Canvas<input type="number" aria-label="Canvas width" min={16} max={4096} value={ui.canvas[0]} onChange={event=>change({...ui,canvas:[event.target.valueAsNumber||1,ui.canvas[1]]})}/>×<input type="number" aria-label="Canvas height" min={16} max={4096} value={ui.canvas[1]} onChange={event=>change({...ui,canvas:[ui.canvas[0],event.target.valueAsNumber||1]})}/></label>
     <div className="view-switch">{([false,true] as const).map(item=><button type="button" key={String(item)} className={world===item?"selected":""} onClick={()=>setWorld(item)}>{item?"World":"Screen"}</button>)}</div>
    </div>
    {(error||status)&&<p className={error?"brain-error":"brain-status"} role={error?"alert":"status"}>{error||status}</p>}
    <div className={`ui-stage${world?" world":""}`}>
     <UiCanvas ui={ui} rects={rects} selected={world?null:selected} interactive={!world} onSelect={setSelected} onChange={change}/>
    </div>
    <p className="transform-note ui-stage-note">{world?"World space: the same UI on a panel in perspective (preview only; select Screen to edit).":"Drag to move, drag the corner to resize. Moving converts that offset to pixels; layout containers manage positions; resize or reorder children in the tree."}</p>
    <textarea className="brain-description" aria-label="UI description" maxLength={500} placeholder="What this UI is for (shown in the library and to the AI)…" value={ui.description} onChange={event=>change({...ui,description:event.target.value})}/>
    <div className="brain-chat">
     {chatBusy?null:dirty&&!scratch&&<p className="transform-note">Save your widget edits to enable sending; your request draft will be kept.</p>}
     <ProjectChat projectId={projectId} projectName={projectName} scene={{path:scenePath,name:scenePath.replace(/^.*\//,"").replace(/\.scene\.toml$/,"")}} object={{scope:open.path?`file:${open.path}`:`project:${projectId}`,name:ui.name,draftKey:ui.id}} requestContext={scratch?uiCreationContext(ui):undefined} aiSettings={aiSettings} disabled={busy||(!scratch&&dirty)} allowDraftWhileDisabled onBusy={setChatBusy} onApplied={()=>void afterAI()}/>
    </div>
   </section>
   <aside className="panel brain-inspector" aria-label="Widget inspector">
    <div className="panel-head"><h2>{nodeSpec?.label??"Inspector"}</h2>{node&&<span className="kind-badge">{node.id}</span>}</div>
    {node&&nodeSpec?<div className="brain-node-form">
     <p className="transform-note">{nodeSpec.description}</p>
     {problems.get(node.id)&&<p className="brain-error" role="alert">{problems.get(node.id)}</p>}
     {nodeSpec.fields.map(field=><label className="component-field" key={field.name} title={field.help}><span>{field.label}</span>
      <FieldInput field={field} label={`${nodeSpec.label} ${field.label.toLowerCase()}`} value={node.fields[field.name]??(field.name==="w"||field.name==="h"?sizeDefaults[field.name]:field.default)} instances={[]} onChange={value=>change(setNodeField(ui,node.id,field.name,value===""||typeof value==="number"&&!Number.isFinite(value)?undefined:value))}/>
      <button type="button" className="component-reset" aria-label={`Reset ${field.label}`} disabled={node.fields[field.name]===undefined} onClick={()=>change(setNodeField(ui,node.id,field.name,undefined))}>↺</button></label>)}
     {node.parent&&catalogue&&<label className="settings-field"><span>Move into</span><select aria-label="Move into" value={node.parent} onChange={event=>attempt(()=>moveWidget(ui,catalogue,node.id,event.target.value))}>{panels.map(item=><option key={item.id} value={item.id}>{item.id}</option>)}</select></label>}
     {node.parent&&<button type="button" className="quiet-button brain-delete" onClick={()=>{change(removeNode(ui,node.id));setSelected(node.parent??null);}}>Delete {childrenOf(ui,node.id).length?"widget and its contents":"widget"}</button>}
    </div>:<p className="transform-note">Select a widget on the canvas or in the outline.</p>}
    <section className="ui-actions" aria-label="Actions in this UI">
     <h3>Actions <small>{actions.length}</small></h3>
     {actions.length?<ul>{actions.map(action=><li key={action}><button type="button" onClick={()=>setSelected(ui.nodes.find(item=>item.fields.action===action)?.id??null)}>{action}</button></li>)}</ul>:<p className="transform-note">Give buttons and controls an action label; the game binds each label to a function.</p>}
    </section>
    <section className="brain-apply" aria-label="Save UI">
     <button type="button" className="settings-save" disabled={!catalogue||busy||chatBusy||!dirty||!ui.nodes.length||problems.size>0||!ui.name.trim()} title={problems.size?"Fix the marked widgets first.":undefined} onClick={()=>void save()}>{busy?"Validating…":"Save UI"}</button>
     <button type="button" className="settings-save" disabled={!open.path||dirty||busy||chatBusy} onClick={()=>void place("screen")}>Add screen overlay</button>
     <button type="button" className="settings-save" disabled={!open.path||dirty||busy||chatBusy} onClick={()=>void place("world")}>Add world panel</button>
     <p className="transform-note">{problems.size?"Fix the marked widgets to save. ":""}Save the UI, then add a screen overlay or world panel. World panels follow their scene entity; overlays fill the viewport by default.</p>
    </section>
   </aside>
  </div>
 </main>;
}

function Outline({node,ui,depth,selected,problems,onSelect,onShift}:{node:UiNode;ui:UiDocument;depth:number;selected:string|null;problems:Map<string,string>;onSelect:(id:string)=>void;onShift:(id:string,by:-1|1)=>void}){
 const kids=depth<16?childrenOf(ui,node.id):[];
 return <>
  <div className={`ui-outline-row${selected===node.id?" selected":""}${problems.has(node.id)?" problem":""}`} style={{paddingLeft:6+depth*12}}>
   <button type="button" aria-pressed={selected===node.id} title={problems.get(node.id)??node.id} onClick={()=>onSelect(node.id)}><b>{node.id}</b><small>{node.type}{typeof node.fields.text==="string"?` · ${node.fields.text}`:""}</small></button>
   {node.parent&&<span><button type="button" aria-label={`Move ${node.id} earlier`} onClick={()=>onShift(node.id,-1)}>‹</button><button type="button" aria-label={`Move ${node.id} later`} onClick={()=>onShift(node.id,1)}>›</button></span>}
  </div>
  {kids.map(kid=><Outline key={kid.id} node={kid} ui={ui} depth={depth+1} selected={selected} problems={problems} onSelect={onSelect} onShift={onShift}/>)}
 </>;
}

type Drag={id:string;mode:"move"|"size";start:{x:number;y:number};base:UiDocument;rect:Rect;content:Rect;stack:boolean};

/** SVG preview at canvas resolution: what the engine will draw (layout is exact; glyphs are approximate until the bitmap font lands). */
function UiCanvas({ui,rects,selected,interactive,onSelect,onChange}:{ui:UiDocument;rects:Map<string,Rect>;selected:string|null;interactive:boolean;onSelect:(id:string)=>void;onChange:(ui:UiDocument)=>void}){
 const svg=useRef<SVGSVGElement>(null),drag=useRef<Drag|null>(null);
 const point=(event:ReactPointerEvent)=>{const matrix=svg.current?.getScreenCTM()?.inverse();if(!matrix)return{x:0,y:0};const p=new DOMPoint(event.clientX,event.clientY).matrixTransform(matrix);return{x:p.x,y:p.y};};
 const order:UiNode[]=[];const root=rootOf(ui);
 if(root){const queue=[root];for(let i=0;i<queue.length&&i<4096;i++){order.push(queue[i]);queue.push(...childrenOf(ui,queue[i].id));}}
 function begin(event:ReactPointerEvent,id:string,mode:Drag["mode"]){
  event.stopPropagation();onSelect(id);
  const node=ui.nodes.find(item=>item.id===id),parent=ui.nodes.find(item=>item.id===node?.parent),rect=rects.get(id),box=parent&&rects.get(parent.id);
  if(!interactive||!node||!parent||!rect||!box)return;
  if(mode==="move"&&layoutMode(parent)!=="absolute")return;
  const pad=num(parent.fields.padding,0);
  drag.current={id,mode,start:point(event),base:ui,rect,content:{x:box.x+pad,y:box.y+pad,w:Math.max(0,box.w-2*pad),h:Math.max(0,box.h-2*pad)},stack:parent.fields.stack===true};
  (event.target as Element).setPointerCapture(event.pointerId);
 }
 function move(event:ReactPointerEvent){
  const d=drag.current;if(!d)return;
  const p=point(event),dx=Math.round(p.x-d.start.x),dy=Math.round(p.y-d.start.y),node=d.base.nodes.find(item=>item.id===d.id)!;
  const offset=(name:"x"|"y",size:number)=>{const length=parseLength(text(node.fields[name])||"0px")??{percent:0,px:0};return size*length.percent/100+length.px;};
  let next=d.base;
  if(d.mode==="move"){
   if(dx)next=setNodeField(next,d.id,"x",`${Math.round(offset("x",d.content.w)+dx)}px`);
   if(dy&&!d.stack)next=setNodeField(next,d.id,"y",`${Math.round(offset("y",d.content.h)+dy)}px`);
  }else{
   next=setNodeField(next,d.id,"w",`${Math.max(4,Math.round(d.rect.w+dx))}px`);
   next=setNodeField(next,d.id,"h",`${Math.max(4,Math.round(d.rect.h+dy))}px`);
  }
  onChange(next);
 }
 const sel=selected?rects.get(selected):undefined;
 return <svg ref={svg} className="ui-canvas" viewBox={`0 0 ${ui.canvas[0]} ${ui.canvas[1]}`} role="img" aria-label={`${ui.name} preview`} onPointerMove={move} onPointerUp={()=>{drag.current=null;}} onPointerCancel={()=>{if(drag.current)onChange(drag.current.base);drag.current=null;}}>
  {order.map(node=>{const rect=rects.get(node.id);return rect?<g key={node.id} data-node={node.id} onPointerDown={event=>begin(event,node.id,"move")}><Widget node={node} rect={rect}/></g>:null;})}
  {sel&&interactive&&<g className="ui-selection" pointerEvents="none"><rect x={sel.x} y={sel.y} width={sel.w} height={sel.h}/></g>}
  {sel&&interactive&&selected!==root?.id&&<rect className="ui-resize" x={sel.x+sel.w-5} y={sel.y+sel.h-5} width={10} height={10} aria-label="Resize" onPointerDown={event=>begin(event,selected!,"size")}/>}
 </svg>;
}

function Widget({node,rect}:{node:UiNode;rect:Rect}){
 const f=node.fields,size=num(f.size,2)*8,ink=colour(f.colour,[0.62,0.68,0.72]),cy=rect.y+rect.h/2;
 const label=(value:string,x:number,anchor:"start"|"middle"|"end"="start",fill=ink)=><text x={x} y={cy} fill={fill} fontSize={size} textAnchor={anchor} dominantBaseline="central" className="ui-glyphs">{value}</text>;
 const split=rect.x+rect.w*0.55,right=rect.x+rect.w;
 switch(node.type){
  case "horizontal": case "vertical": case "grid": return <rect x={rect.x} y={rect.y} width={rect.w} height={rect.h} fill="none" stroke="#55606a" strokeDasharray="3 3"/>;
  case "panel":{const border=num(f.border_width,0);return <rect x={rect.x+border/2} y={rect.y+border/2} width={Math.max(0,rect.w-border)} height={Math.max(0,rect.h-border)} fill={colour(f.fill,[0.012,0.016,0.024])} stroke={border?colour(f.border,[0.25,0.3,0.33]):"none"} strokeWidth={border}/>;}
  case "divider":{const t=num(f.thickness,2);return <rect x={rect.x} y={cy-t/2} width={rect.w} height={t} fill={colour(f.colour,[0.25,0.3,0.33])}/>;}
  case "label":{const align=text(f.align)||"left";return <>{f.fill&&<rect x={rect.x} y={rect.y} width={rect.w} height={rect.h} fill={colour(f.fill,[0,0,0])}/>}{label(text(f.text),align==="center"?rect.x+rect.w/2:align==="right"?right-6:rect.x+6,align==="center"?"middle":align==="right"?"end":"start")}</>;}
  case "button":{const on=f.selected===true,fill=colour(f.fill,[0.05,0.06,0.07]);return <><rect x={rect.x+1} y={rect.y+1} width={Math.max(0,rect.w-2)} height={Math.max(0,rect.h-2)} fill={on?ink:fill} stroke={ink} strokeOpacity={0.35}/>{label(text(f.text),rect.x+8,"start",on?fill:ink)}{on&&label("▸",right-8,"end",fill)}</>;}
  case "textbox":return <><rect x={rect.x} y={rect.y} width={rect.w} height={rect.h} fill={colour(f.fill,[0.02,0.025,0.035])} stroke={ink} strokeOpacity={0.5}/>{label(text(f.text),rect.x+6,"start",ink)}</>;
  case "toggle":{const s=Math.min(rect.h-4,size*1.4),x=split,y=cy-s/2;return <>{label(text(f.text),rect.x+6)}<rect x={x} y={y} width={s} height={s} fill="none" stroke={ink} strokeWidth={2}/>{f.checked===true&&<path d={`M${x+3} ${y+3}L${x+s-3} ${y+s-3}M${x+s-3} ${y+3}L${x+3} ${y+s-3}`} stroke={ink} strokeWidth={2}/>}</>;}
  case "slider":{const min=num(f.min,0),max=num(f.max,1),t=max>min?Math.min(1,Math.max(0,(num(f.default,0.5)-min)/(max-min))):0,h=Math.min(rect.h-6,size*1.2),w=right-split-4;return <>{label(text(f.text),rect.x+6)}<rect x={split} y={cy-h/2} width={w} height={h} fill="none" stroke={ink} strokeOpacity={0.6}/><rect x={split+2} y={cy-h/2+2} width={Math.max(0,(w-4)*t)} height={Math.max(0,h-4)} fill={colour(f.fill,[0.45,0.5,0.52])}/></>;}
  case "stepper":{const options=text(f.options).split(",").map(item=>item.trim()).filter(Boolean),choice=options[Math.min(options.length-1,Math.max(0,Math.round(num(f.index,0))))]??"—";return <>{label(text(f.text),rect.x+6)}{label("◂",split,"start")}{label(choice,(split+right)/2,"middle")}{label("▸",right-6,"end")}</>;}
  default:return <rect x={rect.x} y={rect.y} width={rect.w} height={rect.h} fill="none" stroke="#e0a294" strokeDasharray="4 3"/>;
 }
}
