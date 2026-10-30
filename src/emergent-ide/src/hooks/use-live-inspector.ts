"use client";
import {useEffect,useRef,useState} from "react";

let saveQueue:Promise<unknown>=Promise.resolve();
const dirtyInspectors=new Set<symbol>();
let focusedControl:HTMLElement|null=null;

/** Coalesce typing, validate before writing, and retain failed drafts for correction. */
export function useLiveInspector<T,V>(initial:T,prepare:(draft:T)=>V,onSave:(value:V|null)=>Promise<void>,onDirty?:(dirty:boolean)=>void,onBusy?:(busy:boolean)=>void){
 const fingerprint=JSON.stringify(initial);
 const [source,setSource]=useState(fingerprint),[draft,setDraft]=useState(initial),[dirty,setDirty]=useState(false),[saving,setSaving]=useState(false),[error,setError]=useState("");
 const timer=useRef<ReturnType<typeof setTimeout>|null>(null),token=useRef(Symbol()),saveRef=useRef(onSave);
 useEffect(()=>{saveRef.current=onSave;},[onSave]);
 if(source!==fingerprint&&!dirty&&!saving){setSource(fingerprint);setDraft(initial);}
 useEffect(()=>{const id=token.current;const before=(event:BeforeUnloadEvent)=>{if(dirtyInspectors.has(id)){event.preventDefault();event.returnValue="";}};window.addEventListener("beforeunload",before);return()=>{if(timer.current)clearTimeout(timer.current);dirtyInspectors.delete(id);onDirty?.(dirtyInspectors.size>0);window.removeEventListener("beforeunload",before);};},[onDirty]);
 function markDirty(value:boolean){if(value)dirtyInspectors.add(token.current);else dirtyInspectors.delete(token.current);setDirty(value);onDirty?.(dirtyInspectors.size>0);}
 function cancel(){if(timer.current)clearTimeout(timer.current);timer.current=null;}
 function commit(value:V|null){
  cancel();markDirty(true);setSaving(true);setError("");
  const active=document.activeElement;
  if(active instanceof HTMLElement&&active.closest(".material-inspector,.physics-inspector,.transform-inspector,.noise-inspector,.camera-inspector,.components-inspector"))focusedControl=active;
  const operation=saveQueue.catch(()=>{}).then(async()=>{
   onBusy?.(true);
   try{await saveRef.current(value);markDirty(false);}
   catch(e){setError(e instanceof Error?e.message:"Could not save changes.");}
   finally{setSaving(false);onBusy?.(false);requestAnimationFrame(()=>{if(document.activeElement===document.body&&focusedControl?.isConnected&&!focusedControl.matches(":disabled"))focusedControl.focus({preventScroll:true});});}
  });
  saveQueue=operation;return operation;
 }
 function change(next:T){
  cancel();setDraft(next);markDirty(true);setError("");
  try{const value=prepare(next);timer.current=setTimeout(()=>void commit(value),300);}
  catch(e){setError(e instanceof Error?e.message:"Enter a valid value.");}
 }
 return {draft,change,dirty,saving,error,reset:()=>void commit(null),status:saving?"Saving…":dirty?"Unsaved changes":"Saved automatically"};
}
