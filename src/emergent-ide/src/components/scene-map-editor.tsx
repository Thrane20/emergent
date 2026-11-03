"use client";
import Link from "next/link";
import {Fragment,useCallback,useEffect,useMemo,useRef,useState,type PointerEvent as ReactPointerEvent} from "react";
import {FieldInput} from "@/components/field-input";
import type {ComponentType,ComponentValue} from "@/lib/components";
import {MAP_TOOLS,MIN_DRAG_METRES,type GestureResult,type MapTool} from "@/lib/map-tools";
import {fitView,metadataOf,pixelsPerMetre,toScreen,toWorld,waypointLinks,zoneShape,zoomAt,type MapInstance,type MapView,type MapWind,type SceneMapModel} from "@/lib/scene-map-types";

const MAP_TYPES=["emergent.zone","emergent.waypoint","emergent.metadata"];
const isMapItem=(instance:MapInstance)=>instance.components.some(component=>component.type==="emergent.zone"||component.type==="emergent.waypoint");
const at=(instance:MapInstance):[number,number]=>[instance.matrix[12],instance.matrix[14]];
type Draft=
 |{kind:"pan";sx:number;sy:number;view:MapView}
 |{kind:"move";id:string;sx:number;sy:number;dx:number;dz:number;moved:boolean}
 |{kind:"circle";x:number;z:number;radius:number}
 |{kind:"rect";x0:number;z0:number;x1:number;z1:number}
 |{kind:"polyline";points:[number,number][]};

async function request<T>(url:string,init?:RequestInit):Promise<T>{const response=await fetch(url,init),data=await response.json();if(!response.ok)throw new Error(data.error??"Request failed.");return data as T;}

/** Fill for a zone by its cost: blocked hatched red, costly orange, cheap green, neutral blue. */
function zoneStyle(cost:number,blocked:boolean){
 if(blocked)return {fill:"url(#map-blocked)",stroke:"#e0645a"};
 if(cost>1)return {fill:`rgba(232,150,70,${Math.min(0.12+Math.log2(cost)*0.08,0.45)})`,stroke:"#e8a050"};
 if(cost<1)return {fill:`rgba(110,200,120,${Math.min(0.12+Math.log2(1/cost)*0.08,0.45)})`,stroke:"#7ccf88"};
 return {fill:"rgba(120,170,230,.12)",stroke:"#8ab4e8"};
}

export function SceneMapEditor({projectId,projectName,scenes,initialScene}:{projectId:string;projectName:string;scenes:string[];initialScene:string}){
 const [scene,setScene]=useState(initialScene),[map,setMap]=useState<SceneMapModel|null>(null),[types,setTypes]=useState<ComponentType[]>([]);
 const [view,setView]=useState<MapView>({x:0,z:0,height:60}),[size,setSize]=useState({width:800,height:600}),fitted=useRef("");
 const [toolId,setToolId]=useState("select"),[selected,setSelected]=useState<string|null>(null),[parent,setParent]=useState(""),[linkFrom,setLinkFrom]=useState<string|null>(null);
 const [overlay,setOverlay]=useState(true),[draft,setDraft]=useState<Draft|null>(null),[hover,setHover]=useState<[number,number]|null>(null),[busy,setBusy]=useState(""),[error,setError]=useState("");
 const surface=useRef<HTMLDivElement>(null),tool=MAP_TOOLS.find(item=>item.id===toolId)!;

 const sizeRef=useRef(size);
 const fit=useCallback((next:SceneMapModel)=>setView(fitView(next.instances.flatMap(instance=>instance.outline.length?instance.outline:[at(instance)]),sizeRef.current)),[]);
 const load=useCallback(async(path:string)=>{
  try{
   const next=await request<SceneMapModel>(`/api/map?projectId=${encodeURIComponent(projectId)}&scene=${encodeURIComponent(path)}`);setMap(next);setError("");
   // Frame each scene once, when it first loads.
   if(fitted.current!==path){fitted.current=path;fit(next);}
  }
  catch(e){setError(e instanceof Error?e.message:"The map could not be loaded.");}
 },[fit,projectId]);
 useEffect(()=>{
  let active=true;
  request<SceneMapModel>(`/api/map?projectId=${encodeURIComponent(projectId)}&scene=${encodeURIComponent(scene)}`).then(next=>{
   if(!active)return;setMap(next);setError("");
   if(fitted.current!==scene){fitted.current=scene;fit(next);}
  }).catch(e=>{if(active)setError(e instanceof Error?e.message:"The map could not be loaded.");});
  return()=>{active=false;};
 },[fit,projectId,scene]);
 useEffect(()=>{let active=true;void request<{types:ComponentType[]}>(`/api/components?projectId=${encodeURIComponent(projectId)}`).then(data=>{if(active)setTypes(data.types);}).catch(()=>{});return()=>{active=false;};},[projectId]);
 // Other editors may change the scene; pick that up when the page comes back into focus.
 useEffect(()=>{const refresh=()=>{if(!busy)void load(scene);};window.addEventListener("focus",refresh);return()=>window.removeEventListener("focus",refresh);},[busy,load,scene]);
 useEffect(()=>{const element=surface.current;if(!element)return;const observer=new ResizeObserver(([entry])=>{const next={width:entry.contentRect.width,height:entry.contentRect.height};sizeRef.current=next;setSize(next);});observer.observe(element);return()=>observer.disconnect();},[]);

 async function act(body:Record<string,unknown>,label:string){
  if(!map)return null;setBusy(label);setError("");
  try{const result=await request<{map:SceneMapModel;added?:string[];group?:string|null}>("/api/map",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,scene,revision:map.revision,...body})});setMap(result.map);return result;}
  catch(e){setError(e instanceof Error?e.message:"The map edit failed.");void load(scene);return null;}
  finally{setBusy("");}
 }
 async function create(active:MapTool,result:GestureResult){
  const built=active.build?.(result);if(!built?.items.length)return;
  const added=await act({action:"add",items:built.items,path:built.path,name:built.name,parent:parent||null},built.path?"Adding the path…":`Adding ${built.items[0].kind==="zone"?"the zone":"the waypoint"}…`);
  if(added?.added?.length)setSelected(added.group??added.added.at(-1)!);
 }

 const instances=useMemo(()=>map?.instances??[],[map]);
 const byId=useMemo(()=>new Map(instances.map(instance=>[instance.id,instance])),[instances]);
 const selectedItem=selected?byId.get(selected):undefined;
 const screen=(x:number,z:number)=>toScreen(view,size,x,z),scale=pixelsPerMetre(view,size.height);
 function world(event:{clientX:number;clientY:number}){const box=surface.current!.getBoundingClientRect();return toWorld(view,size,event.clientX-box.left,event.clientY-box.top) as [number,number];}
 /** The map item under the pointer: waypoints first (they're small), then zones, topmost last-drawn. */
 function hit(x:number,z:number,onlyWaypoints=false){
  const tolerance=9/scale;
  for(const instance of [...instances].reverse())if(waypointLinks(instance)&&Math.hypot(at(instance)[0]-x,at(instance)[1]-z)<=tolerance)return instance;
  if(onlyWaypoints)return undefined;
  for(const instance of [...instances].reverse()){
   const shape=zoneShape(instance);if(!shape)continue;
   const [dx,dz]=[x-shape.x,z-shape.z];
   if(shape.kind==="circle"?Math.hypot(dx,dz)<=shape.radius:Math.abs(dx*Math.cos(shape.yaw)-dz*Math.sin(shape.yaw))<=shape.width/2&&Math.abs(dx*Math.sin(shape.yaw)+dz*Math.cos(shape.yaw))<=shape.depth/2)return instance;
  }
  return undefined;
 }

 function down(event:ReactPointerEvent<HTMLDivElement>){
  if(!map||busy)return;
  const [x,z]=world(event);surface.current?.setPointerCapture(event.pointerId);
  if(event.button===1||event.button===2||event.shiftKey&&tool.gesture==="select"){setDraft({kind:"pan",sx:event.clientX,sy:event.clientY,view});return;}
  if(event.button!==0)return;
  switch(tool.gesture){
   case "select":{const item=hit(x,z);if(item){setSelected(item.id);setDraft({kind:"move",id:item.id,sx:event.clientX,sy:event.clientY,dx:0,dz:0,moved:false});}else{setSelected(null);setDraft({kind:"pan",sx:event.clientX,sy:event.clientY,view});}break;}
   case "point":void create(tool,{gesture:"point",x,z});break;
   case "circle":setDraft({kind:"circle",x,z,radius:0});break;
   case "rect":setDraft({kind:"rect",x0:x,z0:z,x1:x,z1:z});break;
   case "polyline":setDraft(current=>current?.kind==="polyline"?{kind:"polyline",points:[...current.points,[x,z]]}:{kind:"polyline",points:[[x,z]]});break;
   case "link":{
    const item=hit(x,z,true);if(!item){setLinkFrom(null);break;}
    if(!linkFrom){setLinkFrom(item.id);setSelected(item.id);break;}
    if(linkFrom!==item.id)void act({action:"link",from:linkFrom,to:item.id},"Linking…");
    setLinkFrom(null);break;
   }
  }
 }
 function move(event:ReactPointerEvent<HTMLDivElement>){
  const [x,z]=world(event);setHover([x,z]);
  if(!draft)return;
  if(draft.kind==="pan"){const [dx,dy]=[event.clientX-draft.sx,event.clientY-draft.sy];setView({...draft.view,x:draft.view.x-dx/scale,z:draft.view.z-dy/scale});}
  else if(draft.kind==="move"){const [dx,dz]=[(event.clientX-draft.sx)/scale,(event.clientY-draft.sy)/scale];setDraft({...draft,dx,dz,moved:draft.moved||Math.hypot(event.clientX-draft.sx,event.clientY-draft.sy)>3});}
  else if(draft.kind==="circle")setDraft({...draft,radius:Math.hypot(x-draft.x,z-draft.z)});
  else if(draft.kind==="rect")setDraft({...draft,x1:x,z1:z});
 }
 function up(){
  const finished=draft;if(!finished||finished.kind==="polyline")return;setDraft(null);
  if(finished.kind==="move"&&finished.moved){const item=byId.get(finished.id);if(item){const [x,z]=at(item);void act({action:"move",id:finished.id,x:x+finished.dx,z:z+finished.dz},"Moving…");}}
  if(finished.kind==="circle")void create(tool,{gesture:"circle",x:finished.x,z:finished.z,radius:finished.radius});
  if(finished.kind==="rect")void create(tool,{gesture:"rect",...finished});
 }
 const finishPath=useCallback(()=>{if(draft?.kind==="polyline"&&draft.points.length){const points=draft.points;setDraft(null);void create(tool,{gesture:"polyline",points});}},[draft,tool]);// eslint-disable-line react-hooks/exhaustive-deps
 useEffect(()=>{
  const key=(event:KeyboardEvent)=>{
   if(event.target instanceof HTMLElement&&event.target.closest("input,textarea,select"))return;
   if(event.key==="Escape"){setDraft(null);setLinkFrom(null);}
   if(event.key==="Enter")finishPath();
   if((event.key==="Delete"||event.key==="Backspace")&&selected&&!busy){const item=byId.get(selected);if(item&&window.confirm(`Delete ${item.name}${instances.some(other=>other.parent===item.id)?" and everything under it":""}?`)){setSelected(null);void act({action:"delete",id:selected},"Deleting…");}}
  };
  window.addEventListener("keydown",key);return()=>window.removeEventListener("keydown",key);
 });
 function wheel(event:React.WheelEvent<HTMLDivElement>){const box=surface.current!.getBoundingClientRect();setView(current=>zoomAt(current,size,event.clientX-box.left,event.clientY-box.top,Math.pow(1.0015,event.deltaY)));}
 function chooseTool(id:string){setToolId(id);setDraft(null);setLinkFrom(null);}

 // A grid step that stays 40-400 px apart at any height.
 const grid=useMemo(()=>{const metres=Math.pow(10,Math.ceil(Math.log10(40/scale)));const step=metres*scale;const [x0,z0]=toWorld(view,size,0,0),[x1,z1]=toWorld(view,size,size.width,size.height);const lines:{v:boolean;p:number;major:boolean}[]=[];for(let x=Math.floor(x0/metres)*metres;x<=x1;x+=metres)lines.push({v:true,p:x,major:Math.round(x/metres)%10===0});for(let z=Math.floor(z0/metres)*metres;z<=z1;z+=metres)lines.push({v:false,p:z,major:Math.round(z/metres)%10===0});return {metres,step,lines};},[scale,view,size]);
 const moving=draft?.kind==="move"?draft:null;
 const position=(instance:MapInstance):[number,number]=>{const [x,z]=at(instance);return moving&&(moving.id===instance.id||ancestor(instance,moving.id))?[x+moving.dx,z+moving.dz]:[x,z];};
 function ancestor(instance:MapInstance,id:string){let parentId=instance.parent;while(parentId){if(parentId===id)return true;parentId=byId.get(parentId)?.parent??null;}return false;}
 const waypointOptions=instances.filter(instance=>waypointLinks(instance)).map(instance=>({id:instance.id,name:instance.name}));

 return <main className="object-shell brain-shell map-shell">
  <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Scene map</b></span><Link href={`/?scene=${encodeURIComponent(scene)}`} className="quiet-button">Open scene ↗</Link></header>
  <div className="brain-layout map-layout">
   <aside className="panel brain-library map-tools" aria-label="Map tools">
    <div className="panel-head"><h2>Tools</h2></div>
    <div className="map-tool-list" role="toolbar" aria-label="Map tools">
     {MAP_TOOLS.map((item,index)=><Fragment key={item.id}>{item.group&&item.group!==MAP_TOOLS[index-1]?.group&&<span className="map-tool-group">{item.group}</span>}<button type="button" className={`map-tool${item.id===toolId?" selected":""}`} aria-pressed={item.id===toolId} onClick={()=>chooseTool(item.id)} title={item.hint}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d={item.icon}/></svg><span>{item.label}</span></button></Fragment>)}
    </div>
    <p className="brain-hint map-hint">{tool.hint}{tool.id==="link"&&linkFrom?` From: ${byId.get(linkFrom)?.name}.`:""}</p>
    <label className="map-field"><span>Scene</span><select value={scene} onChange={event=>{setScene(event.target.value);setSelected(null);setDraft(null);}}>{scenes.map(path=><option key={path} value={path}>{path.replace(/^.*\//,"").replace(/\.scene\.toml$/,"")}</option>)}</select></label>
    <label className="map-field"><span>Place under</span><select value={parent} onChange={event=>setParent(event.target.value)}><option value="">Scene root</option>{instances.map(instance=><option key={instance.id} value={instance.id}>{instance.name}</option>)}</select></label>
    <label className="map-field map-check"><input type="checkbox" checked={overlay} onChange={event=>setOverlay(event.target.checked)}/><span>Airport overlay</span></label>
    {map&&<WindControls key={`${scene}:${map.wind?.speed??0}:${map.wind?.from??0}`} wind={map.wind} busy={Boolean(busy)} onApply={wind=>void act({action:"wind",wind},"Saving the wind…")}/>}
    <div className="map-legend" aria-label="Zone colours"><span><i className="cost-low"/>preferred</span><span><i className="cost-neutral"/>normal</span><span><i className="cost-high"/>avoided</span><span><i className="cost-blocked"/>blocked</span></div>
   </aside>
   <section className="panel brain-workspace map-workspace">
    <div className="panel-head"><div className="brain-title"><b>{scene.replace(/^.*\//,"").replace(/\.scene\.toml$/,"")}</b><span className="kind-badge">{instances.filter(isMapItem).length} MAP ITEMS</span></div>
     <div className="view-switch"><span className="map-readout">{hover?`x ${hover[0].toFixed(1)}  z ${hover[1].toFixed(1)} · `:""}height {view.height.toFixed(0)} m</span>
      <button type="button" onClick={()=>setView(current=>({...current,height:Math.max(current.height/1.5,2)}))} aria-label="Zoom in">＋</button><button type="button" onClick={()=>setView(current=>({...current,height:Math.min(current.height*1.5,20000)}))} aria-label="Zoom out">－</button>
      <button type="button" disabled={!map} onClick={()=>{if(map)fit(map);}}>Fit</button>
      <button type="button" disabled={Boolean(busy)} onClick={()=>void act({action:"undo"},"Undoing…")} title="Undo the most recent scene change">Undo</button></div></div>
    {error&&<p className="brain-error" role="alert">{error}</p>}
    {busy&&<p className="brain-status" role="status">{busy}</p>}
    <div ref={surface} className={`map-surface tool-${tool.gesture}${busy?" busy":""}`} onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerLeave={()=>setHover(null)} onDoubleClick={finishPath} onWheel={wheel} onContextMenu={event=>event.preventDefault()} aria-label="Top-down scene map">
     <svg width={size.width} height={size.height} role="img" aria-label="Scene seen from above">
      <defs><pattern id="map-blocked" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="8" height="8" fill="rgba(224,100,90,.18)"/><line x1="0" y1="0" x2="0" y2="8" stroke="rgba(224,100,90,.6)" strokeWidth="3"/></pattern>
       <marker id="map-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0 10 5 0 10z" fill="#d9d591"/></marker>
       <marker id="map-circuit-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path d="M0 0 10 5 0 10z" fill="#e6cf93"/></marker>
       <marker id="map-landing-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto"><path d="M0 0 10 5 0 10z" fill="#93d6c0"/></marker></defs>
      <g className="map-grid">{grid.lines.map((line,i)=>{const [sx,sy]=screen(line.v?line.p:0,line.v?0:line.p);return line.v?<line key={i} x1={sx} x2={sx} y1={0} y2={size.height} className={line.major?"major":""}/>:<line key={i} y1={sy} y2={sy} x1={0} x2={size.width} className={line.major?"major":""}/>;})}</g>
      <g className="map-backdrop">{instances.filter(instance=>!isMapItem(instance)).map(instance=>instance.outline.length>=3
       ?<polygon key={instance.id} points={instance.outline.map(([x,z])=>screen(x,z).join(",")).join(" ")}><title>{instance.name}</title></polygon>
       :<circle key={instance.id} className="map-object-point" cx={screen(...at(instance))[0]} cy={screen(...at(instance))[1]} r={3}><title>{instance.name}</title></circle>)}</g>
      <g className="map-zones">{instances.map(instance=>{const shape=zoneShape(instance);if(!shape)return null;const [x,z]=position(instance),[sx,sy]=screen(x,z),style=zoneStyle(shape.cost,shape.blocked),chosen=instance.id===selected;
       const label=metadataOf(instance).type??instance.name;
       return <g key={instance.id} className={chosen?"selected":""}>
        {shape.kind==="circle"?<circle cx={sx} cy={sy} r={shape.radius*scale} fill={style.fill} stroke={style.stroke}/>:<rect x={sx-shape.width*scale/2} y={sy-shape.depth*scale/2} width={shape.width*scale} height={shape.depth*scale} fill={style.fill} stroke={style.stroke} transform={`rotate(${-shape.yaw*180/Math.PI} ${sx} ${sy})`}/>}
        <text x={sx} y={sy} className="map-label">{String(label)}{shape.layers?` · ${shape.layers}`:""}</text></g>;})}</g>
      <g className="map-links">{instances.flatMap(instance=>{const links=waypointLinks(instance);if(!links)return [];const [x,z]=position(instance),[ax,ay]=screen(x,z);
       return links.links.map(target=>{const other=byId.get(target);if(!other)return null;const [bx,by]=screen(...position(other)),length=Math.hypot(bx-ax,by-ay)||1,trim=8/length;
        return <line key={`${instance.id}>${target}`} x1={ax+(bx-ax)*trim} y1={ay+(by-ay)*trim} x2={bx-(bx-ax)*trim} y2={by-(by-ay)*trim} markerEnd={links.twoWay?undefined:"url(#map-arrow)"}/>;});})}</g>
      {overlay&&<g className="map-airports" aria-label="Airport overlay">{(map?.airports??[]).map((airport,a)=><g key={a}>
       {airport.runways.map(runway=>{
        const [tx,ty]=screen(...runway.threshold),[ex,ey]=screen(...runway.farEnd);
        const arrow=<line className="map-landing" x1={tx} y1={ty} x2={tx+(ex-tx)*0.45} y2={ty+(ey-ty)*0.45} markerEnd="url(#map-landing-arrow)"/>;
        if(!runway.active||runway.circuit.length<6)return <g key={runway.id} className="inactive">{arrow}</g>;
        // Circuit points: final turn, threshold, far end, join, base turn, final turn.
        const legs=[["Final",0,1],["Base",4,5],["Downwind",3,4]] as const,points=runway.circuit.map(([x,z])=>screen(x,z));
        const leg=(from:number,to:number)=>[points[from],points[to]] as const;
        return <g key={runway.id}>
         {arrow}<text x={tx} y={ty} dy={18} className="map-airport-label">{`Landing ${Math.round(runway.heading)}°${runway.rightCircuit?" · right circuit":""}`}</text>
         <line className="map-circuit join" x1={points[2][0]} y1={points[2][1]} x2={points[3][0]} y2={points[3][1]}/>
         {[[3,4],[4,5],[0,1]].map(([from,to])=>{const [[ax,ay],[bx,by]]=leg(from,to);return <line key={from} className="map-circuit" x1={ax} y1={ay} x2={bx} y2={by} markerEnd="url(#map-circuit-arrow)"/>;})}
         {legs.map(([label,from,to])=>{const [[ax,ay],[bx,by]]=leg(from,to);return <text key={label} x={(ax+bx)/2} y={(ay+by)/2} className="map-airport-label">{label}</text>;})}
         <circle className="map-circuit-point" cx={points[3][0]} cy={points[3][1]} r={5}/><text x={points[3][0]} y={points[3][1]} dy={-9} className="map-airport-label">{`Join · ${runway.patternAltitude?.toFixed(0)??""} m`}</text>
        </g>;
       })}
       {airport.holdingFix&&(()=>{const [hx,hy]=screen(...airport.holdingFix);return <g><circle className="map-hold" cx={hx} cy={hy} r={Math.max(10,250*scale)}/><text x={hx} y={hy} className="map-airport-label">{airport.towered?"Holding fix":"Holding fix (no tower)"}</text></g>;})()}
      </g>)}</g>}
      <g className="map-waypoints">{instances.filter(instance=>waypointLinks(instance)).map(instance=>{const [sx,sy]=screen(...position(instance));return <g key={instance.id} className={`${instance.id===selected?"selected":""}${instance.id===linkFrom?" linking":""}`}><circle cx={sx} cy={sy} r={6}/><text x={sx+9} y={sy-8} className="map-label">{instance.name}</text></g>;})}</g>
      {draft?.kind==="circle"&&draft.radius>=MIN_DRAG_METRES&&<circle className="map-draft" cx={screen(draft.x,draft.z)[0]} cy={screen(draft.x,draft.z)[1]} r={draft.radius*scale}/>}
      {draft?.kind==="rect"&&(()=>{const [ax,ay]=screen(draft.x0,draft.z0),[bx,by]=screen(draft.x1,draft.z1);return <rect className="map-draft" x={Math.min(ax,bx)} y={Math.min(ay,by)} width={Math.abs(bx-ax)} height={Math.abs(by-ay)}/>;})()}
      {draft?.kind==="polyline"&&<polyline className="map-draft" points={[...draft.points,...(hover?[hover]:[])].map(([x,z])=>screen(x,z).join(",")).join(" ")}/>}
      {draft?.kind==="polyline"&&draft.points.map(([x,z],i)=><circle key={i} className="map-draft-point" cx={screen(x,z)[0]} cy={screen(x,z)[1]} r={5}/>)}
     </svg>
     {map?.wind&&map.wind.speed>0&&<div className="map-wind" aria-label={`Wind ${map.wind.speed} m/s from ${Math.round(map.wind.from)}°`}><svg viewBox="-20 -20 40 40" aria-hidden="true"><circle r="18"/><path d="M0 14 L0 -12 M-6 -5 L0 -13 L6 -5" transform={`rotate(${map.wind.from+180})`}/></svg><span>{map.wind.speed} m/s<br/>from {Math.round(map.wind.from)}°</span></div>}
     <div className="map-scale" aria-hidden="true"><i style={{width:grid.step}}/>{grid.metres>=1000?`${grid.metres/1000} km`:`${grid.metres} m`}</div>
     {!map&&!error&&<p className="map-loading">Loading the scene…</p>}
    </div>
   </section>
   <aside className="panel brain-inspector" aria-label="Map item">
    <div className="panel-head"><h2>{selectedItem?.name??"Map item"}</h2>{selectedItem&&<span className="kind-badge">{zoneShape(selectedItem)?"ZONE":waypointLinks(selectedItem)?"WAYPOINT":"OBJECT"}</span>}</div>
    {selectedItem?<MapInspector key={`${selectedItem.id}:${map?.revision}`} item={selectedItem} types={types} waypoints={waypointOptions.filter(option=>option.id!==selectedItem.id)} parentName={selectedItem.parent?byId.get(selectedItem.parent)?.name:undefined} busy={Boolean(busy)}
     onRename={name=>void act({action:"rename",id:selectedItem.id,name},"Renaming…")}
     onFields={(type,fields)=>void act({action:"component",id:selectedItem.id,type,fields},"Saving…")}
     onDelete={()=>{if(window.confirm(`Delete ${selectedItem.name}?`)){setSelected(null);void act({action:"delete",id:selectedItem.id},"Deleting…");}}}/>
     :<div className="brain-node-form"><p className="transform-note">Pick a tool on the left and click the map to add waypoints, paths and zones. They go straight into the scene (undo with Undo). Select one to edit its shape, cost, links and metadata.</p><p className="transform-note">Drag empty space (or with the right mouse button) to pan; scroll to change height.</p></div>}
   </aside>
  </div>
 </main>;
}

/** Fields of the selected item's map components, built from the engine's registry. Changes save after a short pause. */
function MapInspector({item,types,waypoints,parentName,busy,onRename,onFields,onDelete}:{item:MapInstance;types:ComponentType[];waypoints:{id:string;name:string}[];parentName?:string;busy:boolean;onRename:(name:string)=>void;onFields:(type:string,fields:Record<string,ComponentValue|null>|null)=>void;onDelete:()=>void}){
 const [name,setName]=useState(item.name),pending=useRef<{type:string;fields:Record<string,ComponentValue|null>}|null>(null),timer=useRef<ReturnType<typeof setTimeout>|null>(null);
 const flush=useCallback(()=>{if(timer.current)clearTimeout(timer.current);timer.current=null;const next=pending.current;pending.current=null;if(next)onFields(next.type,next.fields);},[onFields]);
 // Save anything still waiting when the selection changes or the map reloads.
 const flushRef=useRef(flush);useEffect(()=>{flushRef.current=flush;},[flush]);
 useEffect(()=>()=>flushRef.current(),[]);
 function change(type:string,field:string,value:ComponentValue|null){
  if(pending.current&&pending.current.type!==type)flush();
  pending.current={type,fields:{...pending.current?.fields,[field]:value}};
  if(timer.current)clearTimeout(timer.current);timer.current=setTimeout(flush,700);
 }
 const [x,,z]=[item.matrix[12],item.matrix[13],item.matrix[14]];
 const present=MAP_TYPES.filter(type=>item.components.some(component=>component.type===type));
 return <div className="brain-node-form">
  <label className="component-field"><span>Name</span><input type="text" maxLength={64} value={name} disabled={busy} onChange={event=>setName(event.target.value)} onBlur={()=>{if(name.trim()&&name!==item.name)onRename(name.trim());}} onKeyDown={event=>{if(event.key==="Enter")(event.target as HTMLInputElement).blur();}}/><span/></label>
  <p className="transform-note">At x {x.toFixed(2)}, z {z.toFixed(2)}{parentName?` · under ${parentName}`:""}</p>
  {present.map(type=>{const schema=types.find(entry=>entry.id===type),current=item.components.find(component=>component.type===type)!;if(!schema)return null;
   return <section key={type} className="map-component" aria-label={schema.label}><h3>{schema.label}</h3>
    {schema.fields.map(field=>{const value=current.fields[field.name]??field.default;
     return <label key={field.name} className="component-field" title={field.help}><span>{field.label}{field.unit?` (${field.unit})`:""}</span>
      <FieldInput field={field} label={field.label} value={value} instances={waypoints} onChange={next=>change(type,field.name,field.kind==="entities"&&Array.isArray(next)&&!next.length?null:next)}/><span/></label>;})}
   </section>;})}
  {!present.includes("emergent.metadata")&&<button type="button" className="quiet-button" disabled={busy} onClick={()=>onFields("emergent.metadata",{})}>Add metadata</button>}
  <section className="brain-apply"><button type="button" className="quiet-button" disabled={busy} onClick={onDelete}>Delete</button></section>
 </div>;
}

/** Scene wind: speed (m/s, 0 removes it) and the compass bearing it blows from. */
function WindControls({wind,busy,onApply}:{wind:MapWind|null;busy:boolean;onApply:(wind:MapWind|null)=>void}){
 const [speed,setSpeed]=useState(wind?.speed??0),[from,setFrom]=useState(wind?.from??0);
 const changed=speed!==(wind?.speed??0)||from!==(wind?.from??0),valid=Number.isFinite(speed)&&speed>=0&&speed<=100&&Number.isFinite(from)&&from>=0&&from<=360;
 return <fieldset className="map-wind-fields" disabled={busy}><legend>Wind</legend>
  <label className="map-field"><span>Speed (m/s)</span><input type="number" aria-label="Wind speed" min={0} max={100} step={0.5} value={speed} onChange={event=>setSpeed(event.target.valueAsNumber)}/></label>
  <label className="map-field"><span>From (°)</span><input type="number" aria-label="Wind from" min={0} max={360} step={5} value={from} onChange={event=>setFrom(event.target.valueAsNumber)}/></label>
  <button type="button" className="quiet-button" disabled={!changed||!valid} onClick={()=>onApply(speed>0?{speed,from}:null)}>Set wind</button>
 </fieldset>;
}
