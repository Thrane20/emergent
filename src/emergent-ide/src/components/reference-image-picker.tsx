/* eslint-disable @next/next/no-img-element -- project library images served by a scoped route */
"use client";
import Link from "next/link";
import {useEffect,useRef,useState} from "react";
import {createPortal} from "react-dom";
import type {LibraryImage} from "@/lib/image-library";

const TYPES=["image/png","image/jpeg","image/webp"];

/** Chooses a generation reference from the project's image library, or from disk as a fallback. */
export function ReferenceImagePicker({projectId,images,preview,disabled,maxBytes,onPick,onError}:{projectId:string;images:LibraryImage[];preview:string;disabled:boolean;maxBytes:number;onPick:(file:File)=>void;onError:(message:string)=>void}){
 const [open,setOpen]=useState(false),[loading,setLoading]=useState(""),file=useRef<HTMLInputElement>(null);
 const url=(image:LibraryImage)=>`/api/project-asset?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(image.path)}&v=${encodeURIComponent(image.modifiedAt)}`;
 useEffect(()=>{if(!open)return;const onKey=(event:KeyboardEvent)=>{if(event.key==="Escape"&&!loading)setOpen(false);};window.addEventListener("keydown",onKey);return ()=>window.removeEventListener("keydown",onKey);},[open,loading]);
 function accept(chosen:File){
  if(!TYPES.includes(chosen.type)){onError("Choose a PNG, JPEG or WebP image.");return false;}
  if(chosen.size>maxBytes){onError(`Choose an image smaller than ${Math.round(maxBytes/1024/1024)} MiB.`);return false;}
  onPick(chosen);setOpen(false);return true;
 }
 // A library image becomes the same File an upload would, so generation is unchanged.
 async function pick(image:LibraryImage){
  setLoading(image.path);
  try{const response=await fetch(url(image));if(!response.ok)throw new Error();const blob=await response.blob();accept(new File([blob],image.path.split("/").pop()??`${image.name}.png`,{type:blob.type}));}
  catch{onError(`${image.name} could not be loaded.`);}
  finally{setLoading("");}
 }
 return <>
  <button type="button" className="object-upload" disabled={disabled} aria-haspopup="dialog" onClick={()=>setOpen(true)}>{preview?<img src={preview} alt="New generation reference"/>:<div><span>＋</span><b>Choose a reference image</b></div>}</button>
  <input ref={file} type="file" hidden accept={TYPES.join(",")} aria-label="Upload reference image" onChange={event=>{const chosen=event.target.files?.[0];event.target.value="";if(chosen)accept(chosen);}}/>
  {open&&createPortal(<div className="asset-modal-backdrop" onMouseDown={event=>{if(event.target===event.currentTarget&&!loading)setOpen(false);}}>
   <section className="new-scene-modal reference-picker" role="dialog" aria-modal="true" aria-labelledby="reference-picker-title">
    <header className="asset-modal-head"><div><span className="eyebrow">IMAGE LIBRARY</span><h2 id="reference-picker-title">Choose a reference</h2></div><button type="button" aria-label="Close image library" disabled={!!loading} onClick={()=>setOpen(false)}>×</button></header>
    <div className="reference-picker-body">
     {images.length?<div className="reference-picker-grid" role="list" aria-label="Project images">{images.map(image=><button type="button" role="listitem" key={image.path} title={image.path} disabled={!!loading} aria-busy={loading===image.path} onClick={()=>void pick(image)}><img src={url(image)} alt="" loading="lazy"/><span>{image.name}</span></button>)}</div>
      :<p className="object-muted">No images in this project yet. <Link href="/images">Open Image Generator ↗</Link></p>}
     <footer><button type="button" className="asset-cancel" disabled={!!loading} onClick={()=>file.current?.click()}>Upload from disk…</button></footer>
    </div>
   </section>
  </div>,document.body)}
 </>;
}
