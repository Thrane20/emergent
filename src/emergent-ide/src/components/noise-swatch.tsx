"use client";
import {useEffect,useRef,useState} from "react";
import {decodeNoiseField,type NoiseSettings} from "@/lib/noise-field";
export type NoiseAsset={path:string;name:string;displayName?:string;modifiedAt?:string};
const thumbnails=new Map<string,Promise<string>>();
let thumbnailQueue:Promise<unknown>=Promise.resolve();
function thumbnail(projectId:string,asset:NoiseAsset){
 const key=JSON.stringify([projectId,asset.path,asset.modifiedAt]);const cached=thumbnails.get(key);if(cached)return cached;
 const promise=thumbnailQueue.catch(()=>{}).then(async()=>{
  const response=await fetch(`/api/filters/noise?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(asset.path)}`,{cache:"no-store"});const data=await response.json();if(!response.ok)throw new Error(data.error);
  const settings:NoiseSettings={...data.settings,size:24};
  const result=await fetch("/api/filters/noise",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path:asset.path,settings})});
  if(!result.ok){const error=await result.json();throw new Error(error.error??"Noise thumbnail unavailable.");}
  const field=decodeNoiseField(await result.arrayBuffer()),canvas=document.createElement("canvas");canvas.width=field.width;canvas.height=field.height;
  const context=canvas.getContext("2d");if(!context)throw new Error("Noise preview unavailable.");
  const image=context.createImageData(field.width,field.height),start=Math.floor(field.depth/2)*field.width*field.height;
  for(let i=0;i<field.width*field.height;i++){const n=Math.round(field.values[start+i]*255);image.data.set([n,n,n,255],i*4);}
  context.putImageData(image,0,0);return canvas.toDataURL();
 });
 thumbnails.set(key,promise);thumbnailQueue=promise;void promise.catch(()=>thumbnails.delete(key));return promise;
}
export function NoiseSwatch({projectId,channel,assetPath,assets,disabled,onChange}:{projectId:string;channel:string;assetPath?:string;assets:NoiseAsset[];disabled:boolean;onChange:(path:string)=>void}){
 const [open,setOpen]=useState(false),[preview,setPreview]=useState({key:"",image:"",error:""});const root=useRef<HTMLSpanElement>(null);
 const asset=assets.find(file=>file.path===assetPath),key=JSON.stringify([projectId,asset?.path,asset?.modifiedAt]);
 const image=preview.key===key?preview.image:"",error=preview.key===key?preview.error:"";
 useEffect(()=>{if(!asset)return;let active=true;void thumbnail(projectId,asset).then(image=>{if(active)setPreview({key,image,error:""});}).catch(e=>{if(active)setPreview({key,image:"",error:e.message});});return()=>{active=false;};},[projectId,asset,key]);
 useEffect(()=>{if(!open)return;const outside=(event:PointerEvent)=>{if(!root.current?.contains(event.target as Node))setOpen(false);};const escape=(event:KeyboardEvent)=>{if(event.key==="Escape")setOpen(false);};document.addEventListener("pointerdown",outside);document.addEventListener("keydown",escape);return()=>{document.removeEventListener("pointerdown",outside);document.removeEventListener("keydown",escape);};},[open]);
 function choose(path:string){setOpen(false);onChange(path);}
 return <span className="noise-swatch-picker" ref={root}>
  <button type="button" className={`noise-swatch${assetPath?" assigned":""}`} disabled={disabled} aria-label={`${channel} noise map`} aria-haspopup="dialog" aria-expanded={open} title={error||asset?.name||"Choose a noise map"} style={image?{backgroundImage:`url(${image})`}:undefined} onClick={()=>setOpen(!open)}>{image?null:assetPath?(error?"!":"…"):"+"}</button>
  {open&&<span className="noise-swatch-menu" role="dialog" aria-label={`Choose ${channel} noise map`}><button type="button" onClick={()=>choose("")}>No noise map</button>{assets.map(file=><button type="button" key={file.path} aria-pressed={assetPath===file.path} onClick={()=>choose(file.path)}>{file.displayName??file.name.replace(/\.noise\.toml$/,"")}</button>)}{!assets.length&&<span>No noise filters in this project.</span>}</span>}
 </span>;
}
