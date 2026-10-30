"use client";
import {useEffect,useRef,useState,type PointerEvent} from "react";
import {paintTerrain,paintTerrainStroke,type TerrainBrush} from "@/lib/terrain-brush";
import {paintMaterialStroke,quantizeWeights,TERRAIN_MATERIAL_NAMES,type TerrainPaint} from "@/lib/terrain-paint";
import type {TerrainSettings} from "@/lib/terrain";

const paintColors=[[110,90,65],[65,140,65],[125,120,115],[240,240,250]];
export function TerrainSculptor({values,settings,disabled,onCommit,onPaintingChange,paint,onPaintCommit}:{paint?:TerrainPaint|null;onPaintCommit?:(paint:TerrainPaint)=>void;values:Float32Array;settings:TerrainSettings;disabled:boolean;onCommit:(values:Float32Array)=>void;onPaintingChange:(painting:boolean)=>void}){
 const [mode,setMode]=useState("height"),[slot,setSlot]=useState(0);
 const target=paint?mode:"height";
 const weights=useRef<Float32Array>(new Float32Array()),undoWeights=useRef<Float32Array|null>(null);
 const image=useRef<ImageData|null>(null),keyboard=useRef({x:0,z:0});
 const canvas=useRef<HTMLCanvasElement>(null),field=useRef(new Float32Array(0)),scratch=useRef(new Float32Array(0)),undo=useRef<Float32Array|null>(null),frame=useRef(0);
 const pointer=useRef<{id:number;x:number;z:number;last:number;previousX:number;previousZ:number}|null>(null);
 const [brush,setBrush]=useState<TerrainBrush>({mode:"raise",radius:settings.size*0.1,strength:0.5,height:settings.height/2}),[canUndo,setCanUndo]=useState(false),[painting,setPainting]=useState(false),[applied,setApplied]=useState(false);
 const brushRef=useRef(brush);brushRef.current=brush;
 function draw(bounds?:{left:number;top:number;right:number;bottom:number}){
  const c=canvas.current;if(!c)return;const ctx=c.getContext("2d")!,n=settings.resolution;image.current??=ctx.createImageData(n,n);const pixels=image.current;
  const left=Math.max(0,bounds?.left??0),top=Math.max(0,bounds?.top??0),right=Math.min(n-1,bounds?.right??n-1),bottom=Math.min(n-1,bounds?.bottom??n-1);
  for(let z=top;z<=bottom;z++)for(let x=left;x<=right;x++){const i=z*n+x,gray=Math.round(settings.height?field.current[i]/settings.height*255:0),offset=i*4;
   for(let channel=0;channel<3;channel++){let value=gray;if(target==="material"&&paint){value=0;for(let layer=0;layer<4;layer++)value+=paintColors[layer][channel]*(weights.current[offset+layer]??0);value*=0.5+gray/510;}pixels.data[offset+channel]=value;}pixels.data[offset+3]=255;
  }
  ctx.putImageData(pixels,0,0,left,top,right-left+1,bottom-top+1);
 }

 useEffect(()=>{field.current=values.slice();scratch.current=new Float32Array(values.length);draw();},[values]); // eslint-disable-line react-hooks/exhaustive-deps -- draw uses this field and its fixed recipe.
 useEffect(()=>{weights.current=Float32Array.from(paint?.weights??[],value=>value/255);draw();},[paint,mode]); // eslint-disable-line react-hooks/exhaustive-deps
 useEffect(()=>()=>cancelAnimationFrame(frame.current),[]);
 function position(event:PointerEvent<HTMLCanvasElement>){const box=event.currentTarget.getBoundingClientRect();return {x:(Math.max(0,Math.min(1,(event.clientX-box.left)/box.width))-0.5)*settings.size,z:(Math.max(0,Math.min(1,(event.clientY-box.top)/box.height))-0.5)*settings.size};}
 function advance(now:number){
  const p=pointer.current;if(!p)return;
  if(target==="material"&&paint)paintMaterialStroke(weights.current,settings,Math.min(slot,paint.materials.length-1),brushRef.current.radius,brushRef.current.strength,p.previousX,p.previousZ,p.x,p.z,Math.min(0.1,Math.max(0,(now-p.last)/1000)));
  else paintTerrainStroke(field.current,scratch.current,settings,brushRef.current,p.previousX,p.previousZ,p.x,p.z,Math.min(0.1,Math.max(0,(now-p.last)/1000)));
  const radius=brushRef.current.radius,n=settings.resolution,grid=(value:number)=>(value/settings.size+0.5)*(n-1);
  const bounds={left:Math.floor(grid(Math.min(p.previousX,p.x)-radius)),right:Math.ceil(grid(Math.max(p.previousX,p.x)+radius)),top:Math.floor(grid(Math.min(p.previousZ,p.z)-radius)),bottom:Math.ceil(grid(Math.max(p.previousZ,p.z)+radius))};
  p.previousX=p.x;p.previousZ=p.z;p.last=now;keyboard.current={x:p.x,z:p.z};draw(bounds);
 }
 function tick(now:number){if(!pointer.current)return;advance(now);frame.current=requestAnimationFrame(tick);}
 function start(event:PointerEvent<HTMLCanvasElement>){
  if(disabled||event.button!==0||pointer.current)return;event.preventDefault();event.currentTarget.focus({preventScroll:true});const p=position(event);setApplied(false);undo.current=field.current.slice();undoWeights.current=weights.current.slice();setCanUndo(true);setPainting(true);onPaintingChange(true);event.currentTarget.setPointerCapture(event.pointerId);
  pointer.current={id:event.pointerId,...p,previousX:p.x,previousZ:p.z,last:performance.now()-100};tick(performance.now());
 }
 // shortcut: rebuild the GLB at stroke end; use dynamic mesh updates for live 3D sculpting.
 function finish(event:PointerEvent<HTMLCanvasElement>){if(pointer.current?.id!==event.pointerId)return;cancelAnimationFrame(frame.current);if(event.type!=="lostpointercapture")Object.assign(pointer.current,position(event));advance(performance.now());pointer.current=null;setPainting(false);setApplied(true);onPaintingChange(false);if(target==="material"&&paint)onPaintCommit?.({...paint,weights:quantizeWeights(weights.current)});else onCommit(field.current.slice());}
 function cancel(event:PointerEvent<HTMLCanvasElement>){if(pointer.current?.id!==event.pointerId)return;cancelAnimationFrame(frame.current);pointer.current=null;field.current=undo.current!.slice();if(undoWeights.current)weights.current=undoWeights.current;undo.current=null;setPainting(false);onPaintingChange(false);setCanUndo(false);draw();}
 return <section className="terrain-sculptor" aria-label="Terrain brush">
  <h3>Paint here · 2D heightmap</h3><label>Editing<select aria-label="Terrain brush target" value={target} disabled={disabled||painting} onChange={event=>{setMode(event.target.value);setCanUndo(false);}}><option value="height">Terrain height</option><option value="material" disabled={!paint}>Materials</option></select></label>{target==="material"&&paint&&<label>Paint material<select aria-label="Paint material" value={Math.min(slot,paint.materials.length-1)} disabled={disabled||painting} onChange={event=>setSlot(Number(event.target.value))}>{paint.materials.map((path,index)=><option key={index} value={index}>{TERRAIN_MATERIAL_NAMES[index]}</option>)}</select></label>}  <div className="terrain-paint-area"><div className="terrain-heightmap" style={{"--brush-diameter":`${brush.radius/settings.size*100*2}%`} as React.CSSProperties}><canvas ref={canvas} width={settings.resolution} height={settings.resolution} aria-label="Paint terrain heightmap" aria-describedby="terrain-paint-help" tabIndex={disabled?-1:0} onKeyDown={event=>{
   if(disabled||painting)return;const p=keyboard.current,step=settings.size/(settings.resolution-1);
   if(["ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(event.key)){event.preventDefault();p.x=Math.max(-settings.size/2,Math.min(settings.size/2,p.x+(event.key==="ArrowLeft"?-step:event.key==="ArrowRight"?step:0)));p.z=Math.max(-settings.size/2,Math.min(settings.size/2,p.z+(event.key==="ArrowUp"?-step:event.key==="ArrowDown"?step:0)));}
   else if(event.key===" "){event.preventDefault();undo.current=field.current.slice();setCanUndo(true);if(target==="material"&&paint){undoWeights.current=weights.current.slice();paintMaterialStroke(weights.current,settings,Math.min(slot,paint.materials.length-1),brush.radius,brush.strength,p.x,p.z,p.x,p.z,0.1);onPaintCommit?.({...paint,weights:quantizeWeights(weights.current)});}else {paintTerrain(field.current,scratch.current,settings,brush,p.x,p.z,0.1);onCommit(field.current.slice());}draw();setApplied(true);}
   else return;
   event.currentTarget.parentElement!.style.setProperty("--brush-x",`${(p.x/settings.size+0.5)*100}%`);event.currentTarget.parentElement!.style.setProperty("--brush-z",`${(p.z/settings.size+0.5)*100}%`);
  }} onPointerDown={start} onPointerMove={event=>{const p=pointer.current;if(p?.id===event.pointerId)Object.assign(p,position(event));const box=event.currentTarget.getBoundingClientRect();event.currentTarget.parentElement!.style.setProperty("--brush-x",`${event.clientX-box.left}px`);event.currentTarget.parentElement!.style.setProperty("--brush-z",`${event.clientY-box.top}px`);}} onPointerUp={finish} onPointerCancel={cancel} onLostPointerCapture={finish} style={{cursor:disabled?"wait":"crosshair"}}/><span className="terrain-brush-cursor" aria-hidden="true"/></div>
   <div className="terrain-brush-tools"><label>Brush mode<select aria-label="Brush mode" value={brush.mode} disabled={disabled||painting||target==="material"} onChange={e=>setBrush({...brush,mode:e.target.value as TerrainBrush["mode"]})}><option value="raise">Raise</option><option value="lower">Lower</option><option value="smooth">Smooth</option><option value="flatten">Set height / flatten</option></select></label>
   <label>Radius (m) <output>{brush.radius.toFixed(1)}</output><input aria-label="Brush radius" type="range" min={settings.size/(settings.resolution-1)} max={settings.size/2} step={settings.size/1000} value={brush.radius} disabled={disabled||painting} onChange={e=>setBrush({...brush,radius:e.target.valueAsNumber})}/></label>
   <label>Strength <output>{brush.strength.toFixed(2)}</output><input aria-label="Brush strength" type="range" min="0" max="1" step="0.01" value={brush.strength} disabled={disabled||painting} onChange={e=>setBrush({...brush,strength:e.target.valueAsNumber})}/></label>
   <label>Flatten height (m)<input aria-label="Flatten height" type="number" min="0" max={settings.height} step="0.1" value={brush.height} disabled={disabled||painting||target==="material"||brush.mode!=="flatten"} onChange={e=>{const height=e.target.valueAsNumber;if(Number.isFinite(height))setBrush({...brush,height:Math.max(0,Math.min(settings.height,height))});}}/></label>
   <button type="button" className="quiet-button" disabled={disabled||painting||!canUndo} onClick={()=>{field.current=undo.current!.slice();undo.current=null;setCanUndo(false);setApplied(false);if(target==="material"&&paint&&undoWeights.current){weights.current=undoWeights.current;onPaintCommit?.({...paint,weights:quantizeWeights(weights.current)});}else onCommit(field.current.slice());draw();}}>Undo stroke</button>
  </div>
   <div className="terrain-brush-guide">
   <div className="terrain-brush-mask" aria-label="Soft circular grayscale brush"/>
   <p id="terrain-paint-help"><span className="terrain-stroke-status" role="status">{brush.strength===0?"Strength is zero — increase it to paint.":painting?"Painting… release to update 3D.":applied?"Stroke applied. Paint again or undo.":"Click and drag in the heightmap above."}</span><br/>Black = 0 m, white = {settings.height} m</p>
  </div></div>
 </section>;
}
