"use client";
import {useEffect,useRef,useState} from "react";
import type {TextureMap} from "@/lib/material-settings";
export type TextureAsset={path:string;name:string;displayName?:string;modifiedAt?:string};
export function TexturePicker({projectId,label,value,assets,disabled,onChange,onImporting}:{projectId:string;label:string;value?:TextureMap;assets:TextureAsset[];disabled:boolean;onChange:(value:TextureMap|undefined)=>void;onImporting:(busy:boolean)=>void}){
 const [open,setOpen]=useState(false),[imports,setImports]=useState<TextureAsset[]>([]),[error,setError]=useState(""),[uploading,setUploading]=useState(false),root=useRef<HTMLDivElement>(null),file=useRef<HTMLInputElement>(null);
 const choices=[...new Map([...imports,...assets].map(asset=>[asset.path,asset])).values()];
 const preview=(path:string)=>`/api/project-asset?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}&v=${encodeURIComponent(choices.find(asset=>asset.path===path)?.modifiedAt??"")}`;
 useEffect(()=>{if(!open)return;const outside=(event:PointerEvent)=>{if(!root.current?.contains(event.target as Node))setOpen(false);};const escape=(event:KeyboardEvent)=>{if(event.key==="Escape")setOpen(false);};document.addEventListener("pointerdown",outside);document.addEventListener("keydown",escape);return()=>{document.removeEventListener("pointerdown",outside);document.removeEventListener("keydown",escape);};},[open]);
 async function upload(image:File){
  setUploading(true);onImporting(true);setError("");
  try{
   if(image.size>16*1024*1024)throw new Error("Use a PNG/JPEG of at most 16 MiB.");
   const response=await fetch(`/api/textures?projectId=${encodeURIComponent(projectId)}&name=${encodeURIComponent(image.name)}`,{method:"POST",headers:{"Content-Type":"application/octet-stream"},body:image});const result=await response.json();if(!response.ok)throw new Error(result.error??"Image import failed.");
   setImports(previous=>[...previous,result]);setOpen(false);onChange({asset:result.path,scale:value?.scale??1});
  }catch(error){setError(error instanceof Error?error.message:"Image import failed.");}finally{setUploading(false);onImporting(false);if(file.current)file.current.value="";}
 }
 return <div className="texture-picker" ref={root}>
  <div className="material-field"><span>{label}</span><button type="button" className={`noise-swatch${value?" assigned":""}`} disabled={disabled||uploading} aria-label={label} aria-haspopup="dialog" aria-expanded={open} title={value?.asset??"Choose a PNG/JPEG texture"} style={value?{backgroundImage:`url("${preview(value.asset)}")`}:undefined} onClick={()=>setOpen(!open)}>{value?null:"+"}</button></div>
  {open&&<div className="noise-swatch-menu texture-menu" role="dialog" aria-label={`Choose ${label}`}><button type="button" disabled={uploading} onClick={()=>{onChange(undefined);setOpen(false);}}>No image</button><button type="button" disabled={uploading} onClick={()=>file.current?.click()}>{uploading?"Importing…":"Import PNG / JPEG…"}</button>{choices.map(asset=><button type="button" disabled={uploading} key={asset.path} aria-pressed={value?.asset===asset.path} onClick={()=>{onChange({asset:asset.path,scale:value?.scale??1});setOpen(false);}}><span className="texture-thumbnail" style={{backgroundImage:`url("${preview(asset.path)}")`}}/>{asset.displayName??asset.name}</button>)}{!choices.length&&<span>No project textures yet. Import an image.</span>}</div>}
  <input ref={file} type="file" aria-label={`Import ${label}`} hidden accept=".png,.jpg,.jpeg,image/png,image/jpeg" disabled={disabled||uploading} onChange={event=>{const image=event.target.files?.[0];if(image)void upload(image);}}/>
  {value&&<label className="material-field">Repeat / local metre<input aria-label={`${label} repeat`} type="number" min="0.001" max="100" step="any" value={Number.isFinite(value.scale)?value.scale:""} disabled={disabled||uploading} onChange={event=>onChange({...value,scale:event.target.valueAsNumber})}/></label>}
  {error&&<p className="transform-error" role="alert">{error}</p>}
 </div>;
}
