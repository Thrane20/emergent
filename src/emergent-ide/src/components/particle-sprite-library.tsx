/* eslint-disable @next/next/no-img-element -- Show original project sprite assets. */
"use client";
import Link from "next/link";
import type {LibraryImage} from "@/lib/image-library";
export function ParticleSpriteLibrary({projectId,images,selected,disabled,onAssign,onRefresh}:{projectId:string;images:LibraryImage[];selected?:string;disabled:boolean;onAssign:(path:string)=>void;onRefresh:()=>void}){
 const sprites=images.filter(image=>image.kind==="sprite"&&/\.(png|jpe?g)$/i.test(image.path));
 return <section className="panel particle-workshop" aria-label="Particle sprite library"><div className="panel-head"><h3>Sprite library</h3><button type="button" className="quiet-button" disabled={disabled} onClick={onRefresh}>Refresh sprites</button></div><Link href="/images" className="quiet-button">Open Image Generator ↗</Link>{sprites.length?<div className="particle-swatch-grid">{sprites.map(image=><button type="button" key={image.path} className="quiet-button" disabled={disabled} aria-pressed={image.path===selected} onClick={()=>onAssign(image.path)}><img src={`/api/project-asset?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(image.path)}`} alt=""/><span>{image.name}</span><small>{image.sprite?.columns??1} × {image.sprite?.rows??1} · {image.sprite?.frames??1} frames</small></button>)}</div>:null}</section>;
}
