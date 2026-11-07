"use client";
import {useEffect,useRef,type ReactNode} from "react";

export type CarouselItem={path:string;name:string;title:string;thumbnail:ReactNode;error?:string};

/** Saved project assets as a swatch strip with a sticky "new" tile, as in the other editors. */
export function AssetCarousel({label,listLabel,newLabel,items,selected,disabled,onSelect,onNew,onContextMenu}:{label:string;listLabel?:string;newLabel:string;items:CarouselItem[];selected?:string;disabled:boolean;onSelect:(path:string)=>void;onNew:()=>void;onContextMenu?:(path:string)=>void}){
 const track=useRef<HTMLDivElement>(null);
 useEffect(()=>{track.current?.querySelector<HTMLElement>('[aria-pressed="true"]')?.scrollIntoView({block:"nearest",inline:"nearest"});},[selected,items]);
 const scroll=(direction:number)=>track.current?.scrollBy({left:direction*Math.max(140,track.current.clientWidth*.7),behavior:"smooth"});
 return <section className="particle-effect-library brain-carousel" aria-label={label}>
  <div className="particle-library-toolbar"><h2>{label} <span>{items.length}</span></h2><button className="quiet-button" type="button" disabled={!items.length} aria-label={`Previous in ${label}`} onClick={()=>scroll(-1)}>‹</button><button className="quiet-button" type="button" disabled={!items.length} aria-label={`Next in ${label}`} onClick={()=>scroll(1)}>›</button></div>
  <div className="particle-swatch-track" ref={track} role="list" aria-label={listLabel??`Saved ${label.toLowerCase().replace(/ library$/,"")}s`} tabIndex={0}>
   <article className="particle-swatch particle-new-swatch" role="listitem"><button className="particle-swatch-edit" type="button" aria-label={`New blank ${newLabel}`} disabled={disabled} onClick={onNew}><span aria-hidden="true">＋</span><b>New</b></button></article>
   {items.map(item=><article key={item.path} className={`particle-swatch${selected===item.path?" selected":""}`} role="listitem">
    <button className="particle-swatch-edit" type="button" disabled={disabled||!!item.error} aria-pressed={selected===item.path} aria-label={`Open ${item.name}`} title={item.error??item.title} onContextMenu={event=>{if(onContextMenu){event.preventDefault();if(!disabled&&!item.error)onContextMenu(item.path);}}} onKeyDown={event=>{if(onContextMenu&&(event.key==="ContextMenu"||event.shiftKey&&event.key==="F10")){event.preventDefault();onContextMenu(item.path);}}} onClick={()=>onSelect(item.path)}>
     {item.thumbnail}
     <b>{item.name}</b>
    </button>
   </article>)}
  </div>
 </section>;
}
