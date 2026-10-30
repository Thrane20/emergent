"use client";
import {useEffect,useId,useRef,type ReactNode} from "react";
export function InfoTip({label,children}:{label:string;children:ReactNode}){
 const id=useId(),tip=useRef<HTMLSpanElement>(null);
 useEffect(()=>{const dismiss=(event:Event)=>{if(!(event.target instanceof Node)||!tip.current?.contains(event.target))tip.current?.hidePopover();};window.addEventListener("scroll",dismiss,true);window.addEventListener("resize",dismiss);return()=>{window.removeEventListener("scroll",dismiss,true);window.removeEventListener("resize",dismiss);};},[]);
 function show(){
  const element=tip.current,button=element?.previousElementSibling;if(!element||!button)return;
  // A top-layer popover escapes scroll clipping; clamp it to its dialog or viewport.
  const dialog=button.closest("dialog")?.getBoundingClientRect(),anchor=button.getBoundingClientRect();
  const left=Math.max(8,(dialog?.left??0)+8),right=Math.min(innerWidth-8,(dialog?.right??innerWidth)-8),top=Math.max(8,(dialog?.top??0)+8),bottom=Math.min(innerHeight-8,(dialog?.bottom??innerHeight)-8);
  element.style.maxWidth=`${Math.max(0,right-left)}px`;element.style.maxHeight=`${Math.max(0,bottom-top)}px`;
  element.showPopover();const bounds=element.getBoundingClientRect();
  element.style.left=`${Math.max(left,Math.min(anchor.left,right-bounds.width))}px`;
  element.style.top=`${Math.max(top,Math.min(anchor.top-bounds.height>=top?anchor.top-bounds.height:anchor.bottom,bottom-bounds.height))}px`;
 }
 function hide(){tip.current?.hidePopover();}
 return <span className="info-tip" onMouseEnter={show} onMouseLeave={event=>{if(!event.currentTarget.contains(document.activeElement))hide();}} onFocus={show} onBlur={event=>{if(!event.currentTarget.contains(event.relatedTarget))hide();}} onKeyDown={event=>{if(event.key==="Escape"&&tip.current?.matches(":popover-open")){hide();event.preventDefault();event.stopPropagation();}}}><button type="button" className="info-tip-button" aria-label={label} aria-describedby={id}>i</button><span ref={tip} role="tooltip" id={id} popover="manual">{children}</span></span>;
}
