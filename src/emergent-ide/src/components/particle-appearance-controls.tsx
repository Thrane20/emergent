/* eslint-disable @next/next/no-img-element -- Swatches show original project sprites and atlases. */
"use client";
import {useRef} from "react";
import type {LibraryImage} from "@/lib/image-library";

export function ParticleColorControl({label,value,onChange}:{label:string;value:number[];onChange:(value:number[])=>void}){
 const rgb=value.slice(0,3).map(v=>Math.round(v*255));
 return <fieldset className="particle-color"><legend>{label}</legend><div className="particle-color-row"><input type="color" aria-label={`${label} colour`} value={`#${rgb.map(v=>v.toString(16).padStart(2,"0")).join("")}`} onChange={event=>{const hex=event.target.value;onChange([1,3,5].map(i=>parseInt(hex.slice(i,i+2),16)/255).concat(value[3]));}}/><div className="particle-color-values">{["Red","Green","Blue","Opacity"].map((channel,i)=><label key={channel}><span>{"RGBA"[i]}</span><input type="number" min={0} max={i===3?1:255} step={i===3?.01:1} aria-label={`${label} ${channel}`} value={i===3?value[3]:rgb[i]} onChange={event=>onChange(value.map((v,c)=>c===i?event.target.valueAsNumber/(i===3?1:255):v))}/></label>)}</div></div></fieldset>;
}

export function ParticleSpritePicker({projectId,images,value,disabled,onChange}:{projectId:string;images:LibraryImage[];value?:string;disabled:boolean;onChange:(path?:string)=>void}){
 const dialog=useRef<HTMLDialogElement>(null),name=images.find(i=>i.path===value)?.name??value?.split("/").pop()??"Procedural soft circle";
 const sprite=(path?:string)=>path?<img loading="lazy" alt="" src={`/api/project-asset?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}`}/>:<span className="particle-circle-swatch"/>;
 const options:{path:string|undefined;name:string}[]=[{path:undefined,name:"Procedural soft circle"},...images.filter(i=>/\.(png|jpe?g)$/i.test(i.path))];
 if(value&&!images.some(i=>i.path===value))options.push({path:value,name});
 return <div className="particle-sprite-picker"><span>Library image</span><button type="button" className="quiet-button particle-selected-sprite" aria-label="Choose particle sprite" aria-haspopup="dialog" disabled={disabled} onClick={()=>dialog.current?.showModal()}>{sprite(value)}<span>{name}<small>Choose sprite or flipbook…</small></span></button><dialog ref={dialog} className="particle-sprite-dialog" aria-labelledby="particle-sprite-title"><div className="panel-head"><h2 id="particle-sprite-title">Choose particle sprite</h2><button type="button" className="quiet-button" onClick={()=>dialog.current?.close()}>Close</button></div><p className="particle-note">Choose an image from this project. Saved sprite-sheet settings are applied when assigning a different sprite.</p><div className="particle-swatch-grid">{options.map(option=><button type="button" key={option.path??"procedural"} className="quiet-button" aria-pressed={option.path===value} disabled={disabled} onClick={()=>{onChange(option.path);dialog.current?.close();}}>{sprite(option.path)}<span>{option.name}</span></button>)}</div></dialog></div>;
}
