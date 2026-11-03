"use client";
import Link from "next/link";
import {useEffect,useState} from "react";
import type {ShaderAsset} from "@/lib/shader-assets";
import type {MaterialSettings} from "@/lib/material-settings";
export function ShaderMaterialControls({projectId,draft,change}:{projectId:string;draft:MaterialSettings;change:(value:MaterialSettings)=>void}){
 const [items,setItems]=useState<ShaderAsset[]>([]),[error,setError]=useState("");
 useEffect(()=>{const controller=new AbortController();fetch(`/api/shaders?projectId=${encodeURIComponent(projectId)}`,{signal:controller.signal}).then(async r=>{const data=await r.json();if(!r.ok)throw new Error(data.error);setItems(data.shaders);}).catch(e=>{if(!controller.signal.aborted)setError(e.message);});return()=>controller.abort();},[projectId]);
 const graph=items.find(i=>i.path===draft.shader)?.graph;
 return <section aria-label="Shader material settings"><label className="material-field">Surface shader<select aria-label="Material shader" value={draft.shader??""} disabled={Boolean(draft.terrain_paint)} onChange={e=>{const next={...draft};if(e.target.value){next.shader=e.target.value;next.shader_parameters={};next.cast_shadows=false;}else{delete next.shader;delete next.shader_parameters;}change(next);}}><option value="">Standard surface</option>{items.map(i=><option key={i.path} value={i.path}>{i.graph.name}</option>)}</select></label>{graph?.parameters.map(p=><label className="material-field" key={p.id}>{p.name}<input aria-label={`Shader ${p.name}`} type="number" min={p.min} max={p.max} step="any" value={draft.shader_parameters&&Object.hasOwn(draft.shader_parameters,p.id)?draft.shader_parameters[p.id]:p.default} onChange={e=>change({...draft,shader_parameters:{...draft.shader_parameters,[p.id]:e.target.valueAsNumber}})}/></label>)}{draft.shader&&<p>Custom shaders control colour, emission and opacity. Shadow casting is disabled.</p>}<Link href="/shaders">Open Shader Editor ↗</Link>{error&&<p role="alert">{error}</p>}</section>;
}
