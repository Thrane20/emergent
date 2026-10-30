"use client";

import {useCallback,useEffect,useRef,useState} from "react";
import type {ToolResult} from "@/lib/inspection-protocol";
import type {TransformSpace,TransformValues} from "@/lib/scene-placement";
import {imageRect,viewportPoint} from "@/lib/viewport-coordinates";
import {focusView,type OrbitView} from "@/lib/inspection-navigation";

type Projection="perspective"|"isometric";
type View=OrbitView;
type Point={x:number;y:number};
type Drag={token:string;id:string;revision:string;ready:Promise<void>;latest?:TransformValues;changed:boolean;ending:boolean;failure?:Error};
const initialView=():View=>({yaw:Math.atan2(7,9),pitch:Math.asin(5/Math.sqrt(155)),distance:Math.sqrt(155),target:[0,0,0],projection:"perspective",fieldOfView:45,orthographicSize:12});
const colors=["#ff7979","#8ee7a5","#80b5ff"];

export function InspectionViewport({projectId,sessionId,sceneName,frame,readOnly=false,terrainPreview=false,selectedId,revision,space,disabled,onSelect,onCommit,onBusy,onReady,onFailure}:{
 terrainPreview?:boolean;readOnly?:boolean;projectId:string;sessionId:string;sceneName:string;frame?:{target:number[];distance:number};selectedId:string|null;revision:string;space:TransformSpace;disabled:boolean;
 onSelect:(id:string|null)=>void;onCommit:(id:string,revision:string,transform:TransformValues)=>Promise<void>;onBusy:(busy:boolean)=>void;onReady:(id:string)=>void;onFailure:()=>void;
}){
 const canvas=useRef<HTMLCanvasElement>(null),container=useRef<HTMLDivElement>(null);
 const camera=useRef(initialView()),orbit=useRef<{x:number;y:number;startX:number;startY:number;moved:boolean}|null>(null);
 const drag=useRef<Drag|null>(null),desiredDrag=useRef<Point|null>(null),dragSending=useRef<Promise<void>|null>(null);
 const desired=useRef<View|null>(null),sending=useRef(false),disposed=useRef(false);
 const callbacks=useRef({onSelect,onCommit,onBusy});useEffect(()=>{callbacks.current={onSelect,onCommit,onBusy};},[onSelect,onCommit,onBusy]);
 const [error,setError]=useState(""),[busy,setBusy]=useState(false),[tool,setTool]=useState<"translate"|"rotate"|"scale">("translate");
 const [projection,setProjection]=useState<Projection>("perspective"),[fieldOfView,setFieldOfView]=useState(45);
 const [overlay,setOverlay]=useState<ToolResult|null>(null),[rect,setRect]=useState(imageRect(960,540));
 const [overlayCurrent,setOverlayCurrent]=useState(false);
 const overlaySequence=useRef(0);
 const shortcut=useRef(false);
 const lastCommand=useRef(0),pendingOverlay=useRef<{seq:number;data:ToolResult}|null>(null);
 const [stats,setStats]=useState({sequence:0,command:0,worker:0,render:0,encode:0,fps:0});
 const [previewQuality,setPreviewQuality]=useState("auto"),[stepHeatmap,setStepHeatmap]=useState(false);
 const command=useCallback(async(value:object)=>{
  const response=await fetch("/api/inspection",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,id:sessionId,command:value})});
  const data=await response.json();if(!response.ok)throw new Error(data.error||"Inspection command failed.");
  if(data.result&&!disposed.current&&data.commandSequence>=(pendingOverlay.current?.seq??0)){pendingOverlay.current={seq:data.commandSequence,data:data.result};if(lastCommand.current===data.commandSequence){setOverlay(data.result);overlaySequence.current=data.commandSequence;setOverlayCurrent(true);}}
  return data.result as ToolResult;
 },[projectId,sessionId]);
 useEffect(()=>{
  const element=container.current;if(!element)return;
  const observer=new ResizeObserver(([entry])=>setRect(imageRect(entry.contentRect.width,entry.contentRect.height)));observer.observe(element);return()=>observer.disconnect();
 },[]);
 useEffect(()=>{
  disposed.current=false;const abort=new AbortController();let sequence=0,sampleAt=performance.now(),sampleFrames=0,measuredFps=0;
  async function frames(){try{while(!abort.signal.aborted){
   const response=await fetch(`/api/inspection/frame?projectId=${encodeURIComponent(projectId)}&id=${encodeURIComponent(sessionId)}&after=${sequence}`,{signal:abort.signal,cache:"no-store"});
   if(response.status===204)continue;if(!response.ok){const data=await response.json();throw new Error(data.error||"Inspection frame failed.");}
   const bitmap=await createImageBitmap(await response.blob());
   try{if(abort.signal.aborted)return;const context=canvas.current?.getContext("2d");if(!context)throw new Error("Inspection canvas unavailable.");context.drawImage(bitmap,0,0,960,540);}finally{bitmap.close();}
   lastCommand.current=Number(response.headers.get("X-Inspection-Command"));
   if(pendingOverlay.current&&lastCommand.current===pendingOverlay.current.seq){setOverlay(pendingOverlay.current.data);overlaySequence.current=lastCommand.current;}
   setOverlayCurrent(overlaySequence.current===lastCommand.current);
   if(sequence===0)onReady(sessionId);sequence=Number(response.headers.get("X-Inspection-Frame"));
   const now=performance.now();sampleFrames++;if(now-sampleAt>=1000){measuredFps=sampleFrames*1000/(now-sampleAt);sampleFrames=0;sampleAt=now;}
   setStats({sequence,command:lastCommand.current,worker:Number(response.headers.get("X-Inspection-Worker")),render:Number(response.headers.get("X-Inspection-Render-Us"))/1000,encode:Number(response.headers.get("X-Inspection-Encode-Us"))/1000,fps:measuredFps});
  }}catch(error){if(!abort.signal.aborted){setError(error instanceof Error?error.message:"Inspection disconnected.");onFailure();}}}
  void frames();return()=>{disposed.current=true;abort.abort();if(drag.current){const active=drag.current;void active.ready.then(()=>command({op:"cancel",token:active.token})).catch(()=>{});}callbacks.current.onBusy(false);};
 },[projectId,sessionId,onReady,onFailure,command]);
 const running=stats.worker>0;
 useEffect(()=>{if(!terrainPreview||!running)return;let active=true;void command({op:"preview",quality:previewQuality,stepHeatmap}).catch(error=>{if(active)setError(error.message);});return()=>{active=false;};},[terrainPreview,running,command,previewQuality,stepHeatmap]);
 useEffect(()=>{if(!running||busy)return;let active=true;void command({op:"select",id:selectedId,space,tool}).catch(error=>{if(active)setError(error.message);});return()=>{active=false;};},[command,selectedId,revision,space,tool,running,busy]);
 const sendView=useCallback(async()=>{
  if(sending.current)return;sending.current=true;
  try{while(desired.current&&!disposed.current){const view=desired.current;desired.current=null;await command({op:"view",...view});if(!disposed.current)setError("");}}
  catch(error){if(!disposed.current)setError(error instanceof Error?error.message:"Camera command failed.");}finally{sending.current=false;}
 },[command]);
 const framed=useRef(false);
 useEffect(()=>{if(!running||!frame||framed.current)return;framed.current=true;const view={...initialView(),...frame};camera.current=view;desired.current=view;void sendView();},[running,frame,sendView]);
 const update=useCallback((view:View)=>{view={...view,target:view.target.map(n=>Math.max(-10000,Math.min(10000,n)))};camera.current=view;desired.current={...view,target:[...view.target]};void sendView();},[sendView]);
 useEffect(()=>{
  const key=(event:KeyboardEvent)=>{
   if(event.key.toLowerCase()!=="f"||event.repeat||event.ctrlKey||event.metaKey||event.altKey||!selectedId||!revision||!running||disabled||busy||shortcut.current||drag.current||orbit.current||sending.current||desired.current)return;
   if(event.target instanceof Element&&event.target.closest('input,textarea,select,[contenteditable]:not([contenteditable="false"]),[role="textbox"],dialog,[role="dialog"]'))return;
   if(document.querySelector('[aria-modal="true"]'))return;
   event.preventDefault();shortcut.current=true;setBusy(true);callbacks.current.onBusy(true);setError("");
   const id=selectedId,stamp=revision,align=event.shiftKey;
   void (async()=>{
    let saved=false;
    try{
     const result=await command({op:align?"align":"focus",id});
     if(disposed.current)return;
     if(align){
      if(!result.placement)throw new Error("Reload the scene to enable alignment.");
      await callbacks.current.onCommit(id,stamp,result.placement);saved=true;
      await command({op:"transform",id,...result.placement});
     }else{const view=focusView(camera.current,result);setProjection(view.projection);setFieldOfView(view.fieldOfView);update(view);}
    }catch(error){if(saved)onFailure();if(!disposed.current)setError(`${error instanceof Error?error.message:"Viewport shortcut failed."}${saved?" Placement saved; restart inspection.":""}`);}
    finally{shortcut.current=false;if(!disposed.current){setBusy(false);callbacks.current.onBusy(false);}}
   })();
  };
  window.addEventListener("keydown",key);return()=>window.removeEventListener("keydown",key);
 },[selectedId,revision,running,disabled,busy,command,update,onFailure]);
 useEffect(()=>{const element=canvas.current;const wheel=(event:WheelEvent)=>{event.preventDefault();if(drag.current||disabled)return;const view=camera.current,factor=Math.exp(event.deltaY*0.001);update(view.projection==="perspective"?{...view,distance:Math.max(0.25,Math.min(30000,view.distance*factor))}:{...view,orthographicSize:Math.max(0.001,Math.min(1000000,view.orthographicSize*factor))});};element?.addEventListener("wheel",wheel,{passive:false});return()=>element?.removeEventListener("wheel",wheel);},[update,disabled]);
 function point(event:{clientX:number;clientY:number},clamp=false){const box=canvas.current?.getBoundingClientRect();return box?viewportPoint(box,event.clientX,event.clientY,clamp):null;}
 function begin(axis:number,start:Point){
  if(disabled||busy||!overlayCurrent||!selectedId||!revision||sending.current)return null;
  setError("");setBusy(true);callbacks.current.onBusy(true);
  const active:Drag={token:crypto.randomUUID(),id:selectedId,revision,changed:false,ending:false,ready:Promise.resolve()};drag.current=active;
  active.ready=command({op:"begin",axis,...start,id:selectedId,space,tool,token:active.token}).then(result=>{active.latest=result.transform;}).catch(error=>{active.failure=error;});return active;
 }
 function flushDrag(){
  if(dragSending.current)return dragSending.current;
  const active=drag.current;
  const task=(async()=>{if(!active)return;await active.ready;while(desiredDrag.current&&drag.current===active&&!active.failure){const position=desiredDrag.current;desiredDrag.current=null;try{const result=await command({op:"drag",...position,token:active.token});active.latest=result.transform;}catch(error){active.failure=error instanceof Error?error:new Error("Drag failed.");}}})();
  dragSending.current=task.finally(()=>{dragSending.current=null;});return dragSending.current;
 }
 async function finish(cancel=false){
  const active=drag.current;if(!active||active.ending)return;active.ending=true;
  let saved=false;
  try{await active.ready;if(cancel){desiredDrag.current=null;await dragSending.current;await command({op:"cancel",token:active.token});}
   else{await flushDrag();if(active.failure)throw active.failure;if(active.changed&&active.latest){await callbacks.current.onCommit(active.id,active.revision,active.latest);saved=true;}await command({op:"finish",token:active.token});}setError("");
  }catch(error){if(!saved)await command({op:"cancel",token:active.token}).catch(()=>{});else onFailure();setError(`${error instanceof Error?error.message:"Gizmo edit failed."}${saved?" Placement saved; restart inspection.":" Drag canceled; source unchanged."}`);}
  finally{drag.current=null;desiredDrag.current=null;if(!disposed.current){setBusy(false);callbacks.current.onBusy(false);}}
 }
 useEffect(()=>{const key=(event:KeyboardEvent)=>{if(event.key==="Escape"&&drag.current){event.preventDefault();void finish(true);}};window.addEventListener("keydown",key);return()=>window.removeEventListener("keydown",key);});
 function moveHandle(event:React.PointerEvent){const active=drag.current;if(!active||active.ending)return;const position=point(event,true);if(position){active.changed=true;desiredDrag.current=position;void flushDrag();}}
 const handleKey=(event:React.KeyboardEvent,axis:number,p:number[])=>{if(!["ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(event.key))return;event.preventDefault();const active=begin(axis,{x:p[0],y:p[1]});if(!active)return;active.changed=true;desiredDrag.current={x:Math.max(0,Math.min(1,p[0]+(event.key==="ArrowLeft"?-5:event.key==="ArrowRight"?5:0)/960)),y:Math.max(0,Math.min(1,p[1]+(event.key==="ArrowUp"?-5:event.key==="ArrowDown"?5:0)/540))};void finish();};
 return <div className={`inspection-viewport${terrainPreview?" terrain-preview-viewport":""}`} ref={container}>
  <canvas ref={canvas} width={960} height={540} className="inspection-frame" aria-label="Live Rust inspection viewport" data-session={sessionId} data-frame={stats.sequence} data-worker={stats.worker} data-command={stats.command} data-tick="0" data-fps={stats.fps.toFixed(1)}
   onPointerDown={event=>{if(event.button!==0||busy||disabled||!point(event))return;event.currentTarget.setPointerCapture(event.pointerId);orbit.current={x:event.clientX,y:event.clientY,startX:event.clientX,startY:event.clientY,moved:false};}}
   onPointerUp={event=>{const start=orbit.current;orbit.current=null;if(start&&!start.moved&&!disabled&&!readOnly){const p=point(event);if(p)void command({op:"pick",...p}).then(result=>callbacks.current.onSelect(result.selected||null)).catch(error=>setError(error.message));}}}
   onPointerCancel={()=>{orbit.current=null;}} onLostPointerCapture={()=>{orbit.current=null;}}
   onPointerMove={event=>{const start=orbit.current;if(!start)return;const dx=event.clientX-start.x,dy=event.clientY-start.y;if(!start.moved&&Math.hypot(event.clientX-start.startX,event.clientY-start.startY)<4)return;start.moved=true;start.x=event.clientX;start.y=event.clientY;const view=camera.current;if(event.shiftKey){const speed=(view.projection==="perspective"?view.distance:view.orthographicSize)*0.002;update({...view,target:[view.target[0]-dx*speed*Math.cos(view.yaw),view.target[1]+dy*speed,view.target[2]+dx*speed*Math.sin(view.yaw)]});}else update({...view,yaw:Math.max(-10000,Math.min(10000,view.yaw-dx*0.008)),pitch:Math.max(-1.45,Math.min(1.45,view.pitch+dy*0.008))});}}
  />
  <svg className="inspection-gizmos" viewBox="0 0 960 540" style={{left:rect.left,top:rect.top,width:rect.width,height:rect.height,opacity:overlayCurrent?1:0}} aria-label="Scene transform gizmos">
   {overlay?.frustum&&<g className="camera-frustum" data-camera-frustum={overlay.frustum.id} aria-label="Selected camera frustum" stroke="#efcf85" strokeWidth="1.5" fill="none">{overlay.frustum.segments.map((segment,index)=><line key={index} x1={segment[0][0]*960} y1={segment[0][1]*540} x2={segment[1][0]*960} y2={segment[1][1]*540}/>)}</g>}
   {overlay?.markers.map(marker=><g key={marker.id} transform={`translate(${marker.point[0]*960} ${marker.point[1]*540})`}><path d="M 0 -7 L 7 0 L 0 7 L -7 0 Z" fill="#19251d" stroke="#d9d591"/><text x="10" y="3" fill="#eee9b1" fontSize="10">{marker.id}</text></g>)}
   {overlay?.selected===selectedId&&overlay.tool===tool&&overlay.space===space&&overlay.origin&&<>
    <circle cx={overlay.origin[0]*960} cy={overlay.origin[1]*540} r="5" fill="none" stroke="#fff1aa" strokeWidth="2"/>
    {overlay.handles.map(handle=>{const points=handle.points.map(p=>`${p[0]*960},${p[1]*540}`).join(" "),tip=handle.points.at(-1)!;return <g key={handle.axis}>
     <polyline points={points} fill="none" stroke="#0c161f" strokeWidth="5"/><polyline points={points} fill="none" stroke={colors[handle.axis]} strokeWidth="2.5"/>
     {tool!=="rotate"&&<><circle cx={tip[0]*960} cy={tip[1]*540} r={tool==="scale"?5:4} fill={colors[handle.axis]}/><text x={tip[0]*960+8} y={tip[1]*540+4} fill={colors[handle.axis]} fontSize="12">{["X","Y","Z"][handle.axis]}</text></>}
     <polyline role="button" tabIndex={0} aria-label={`${tool} ${["X","Y","Z"][handle.axis]} handle`} aria-disabled={disabled||busy} points={points} fill="none" stroke="transparent" strokeWidth="16" className="gizmo-hit" style={{pointerEvents:overlayCurrent?"stroke":"none"}}
      onPointerDown={event=>{if(event.button!==0)return;event.preventDefault();event.stopPropagation();const p=point(event);if(p&&begin(handle.axis,p))event.currentTarget.setPointerCapture(event.pointerId);}}
      onPointerMove={moveHandle} onPointerUp={()=>void finish()} onPointerCancel={()=>void finish(true)} onLostPointerCapture={()=>{if(drag.current&&!drag.current.ending)void finish(true);}}
      onKeyDown={event=>handleKey(event,handle.axis,tool==="rotate"?handle.points[Math.floor(handle.points.length/8)]:tip)}/>
    </g>;})}
   </>}
  </svg>
  <div className="inspection-tools" role="group" aria-label="Viewport tool">{!readOnly&&(["translate","rotate","scale"] as const).map(mode=><button key={mode} aria-pressed={mode===tool} disabled={!running||disabled||busy} onClick={()=>{setTool(mode);setError("");}}>{mode[0].toUpperCase()+mode.slice(1)}</button>)}<span>{busy?"Release to save · Esc to cancel":selectedId?`${selectedId} · ${space}`:readOnly?"Drag to orbit · Shift-drag to pan · Scroll to zoom":"Click an object to select"}</span><div className="inspection-camera-controls"><label>Camera<select aria-label="Camera projection" value={projection} disabled={!running||disabled||busy} onChange={event=>{const value=event.target.value as Projection;setProjection(value);update({...camera.current,projection:value});}}><option value="perspective">Perspective</option><option value="isometric">Orthographic</option></select></label>{projection==="perspective"&&<label>FOV<input aria-label="Vertical field of view" type="number" min="1" max="178" step="1" value={fieldOfView} disabled={!running||disabled||busy} onChange={event=>{const value=Math.max(1,Math.min(178,Number(event.target.value)||45));setFieldOfView(value);update({...camera.current,fieldOfView:value});}} /></label>}</div></div>
  {!stats.sequence&&!error&&<p className="inspection-message">Starting native inspection session…</p>}
  {error&&<p className="inspection-message inspection-error" role="alert">{error}</p>}
  {selectedId&&overlay?.selected===selectedId&&overlay.tool===tool&&overlay.space===space&&overlay.origin&&!overlay.handles.length&&!busy&&<p className="inspection-message">No usable axes in this view/space. Change the view or use Local mode.</p>}
  {terrainPreview&&<div className="inspection-tools terrain-preview-options" role="group" aria-label="Terrain preview diagnostics"><label>Preview quality<select aria-label="Terrain preview quality" value={previewQuality} disabled={!running} onChange={event=>setPreviewQuality(event.target.value)}><option value="auto">Auto · half while moving</option><option value="full">Full · 960 × 540</option><option value="half">Half · 480 × 270</option></select></label><label><input type="checkbox" checked={stepHeatmap} disabled={!running} onChange={event=>setStepHeatmap(event.target.checked)}/>Ray-step heatmap</label>{stepHeatmap&&<span>Blue: few steps · Red: many · Magenta: exhausted</span>}</div>}
  <div className="inspection-caption"><span>{sceneName} · Live · tick 0 · {stats.fps.toFixed(1)} fps / target 30 · frame {stats.sequence} · capture {stats.render.toFixed(1)} ms · PNG {stats.encode.toFixed(1)} ms</span><button onClick={()=>{const view={...initialView(),...frame};setProjection(view.projection);setFieldOfView(view.fieldOfView);update(view);}} disabled={!stats.sequence||busy||disabled}>Reset view</button></div>
  <div className="inspection-help">F: frame selection · Shift-F: align selection to view · Drag: orbit · Shift-drag: pan · Scroll: dolly / scale</div>
 </div>;
}
