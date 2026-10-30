"use client";
import {useCallback,useEffect,useRef,useState} from "react";
import {InspectionViewport} from "./inspection-viewport";
import type {RaymarchedSettings} from "@/lib/raymarched-terrain";
const noop=()=>{};const commit=async()=>{};
export function TerrainLivePreview({projectId,settings}:{projectId:string;settings:RaymarchedSettings}){
 const [session,setSession]=useState<string|null>(null),[error,setError]=useState("");const owned=useRef<string|null>(null);
 useEffect(()=>{let disposed=false;async function start(){try{
  const response=await fetch("/api/inspection",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,terrain:settings})}),data=await response.json();if(!response.ok)throw Error(data.error);
  if(disposed){await fetch("/api/inspection",{method:"DELETE",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,id:data.id})});return;}
  owned.current=data.id;setSession(data.id);
 }catch(e){if(!disposed)setError(e instanceof Error?e.message:"Live terrain preview failed.");}}
 const timer=setTimeout(()=>void start(),0);return()=>{clearTimeout(timer);disposed=true;const id=owned.current;owned.current=null;if(id)void fetch("/api/inspection",{method:"DELETE",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,id})}).catch(()=>{});};
 },[projectId,settings]);
 const ready=useCallback(()=>{},[]),failure=useCallback(()=>setError("Live terrain preview disconnected. Generate terrain to retry."),[]);
 const cave=settings.biome==="cave-cavern",frame={target:[0,cave?settings.relief*0.25:settings.relief*0.3,0],distance:cave?Math.max(24,settings.cave_radius*3)*0.45:settings.size*1.6};
 return <>{error&&<p className="terrain-error" role="alert">{error}</p>}{session?<InspectionViewport key={session} projectId={projectId} sessionId={session} sceneName="Terrain preview" frame={frame} readOnly terrainPreview selectedId={null} revision="draft" space="local" disabled={false} onSelect={noop} onCommit={commit} onBusy={noop} onReady={ready} onFailure={failure}/>:!error&&<p role="status">Starting live terrain preview…</p>}</>;
}
