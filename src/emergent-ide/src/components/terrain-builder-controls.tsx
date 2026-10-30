"use client";
import Link from "next/link";
import {useId,useRef} from "react";
import {TERRAIN_BIOMES,type TerrainBiome} from "@/lib/terrain-biomes";
export function TerrainMethodSwitch({method,biome,scene,canLeave}:{method:"heightfield"|"raymarch";biome:string;scene:string;canLeave:()=>boolean}){
 return <div className="terrain-method-switch" role="group" aria-label="Terrain method">{([['heightfield','Height Field'],['raymarch','Ray March']] as const).map(([id,label])=><Link key={id} className="quiet-button" role="button" aria-pressed={method===id} href={`/terrain?method=${id}&biome=${encodeURIComponent(biome)}&scene=${encodeURIComponent(scene)}`} onKeyDown={event=>{if(event.key===" "){event.preventDefault();event.currentTarget.click();}}} onNavigate={event=>{if(id===method||!canLeave())event.preventDefault();}}>{label}</Link>)}</div>;
}
export function TerrainBiomePicker({value,method,disabled,onChange}:{value:string;method:"heightfield"|"raymarch";disabled:boolean;onChange:(biome:TerrainBiome)=>void}){
 const dialog=useRef<HTMLDialogElement>(null),selected=TERRAIN_BIOMES.find(b=>b.id===value),id=useId();
 return <div className="terrain-biome-picker"><label htmlFor={id}>Biome</label><button id={id} type="button" className="quiet-button terrain-biome-choice" disabled={disabled} aria-haspopup="dialog" onClick={()=>dialog.current?.showModal()}><span aria-hidden="true">{selected?.symbol??"◇"}</span>{selected?.name??(value==="blend"?"Mixed biomes":value==="legacy"?"Legacy test shapes":"Choose biome")} <small>Change…</small></button>
 <dialog ref={dialog} className="terrain-biome-dialog" aria-labelledby={`${id}-title`}><div className="panel-head"><h2 id={`${id}-title`}>Choose a biome</h2><button type="button" className="quiet-button" onClick={()=>dialog.current?.close()}>Close</button></div><div className="terrain-biome-grid">{TERRAIN_BIOMES.map(b=>{const unsupported=method==="heightfield"&&b.id==="cave-cavern";return <button type="button" key={b.id} disabled={disabled||unsupported} aria-pressed={value===b.id} onClick={()=>{onChange(b.id);dialog.current?.close();}}><span className={`terrain-biome-art biome-${b.id}`} aria-hidden="true">{b.symbol}</span><b>{b.name}</b><small>{b.description}</small>{unsupported&&<em>Select Ray March above to use this biome.</em>}</button>;})}</div></dialog></div>;
}
