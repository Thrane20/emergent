"use client";
import { useEffect,useState } from "react";
import type { TomlDocument } from "@/lib/project-toml";

export function TomlEditor({projectId,path,onDirty,onBusy,onSaved}:{projectId:string;path:string;onDirty:(dirty:boolean)=>void;onBusy:(busy:boolean)=>void;onSaved:()=>void}) {
  const [document,setDocument]=useState<TomlDocument|null>(null),[draft,setDraft]=useState(""),[error,setError]=useState(""),[busy,setBusy]=useState(false),[message,setMessage]=useState("");
  useEffect(()=>{let active=true;void fetch(`/api/project-toml?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}`,{cache:"no-store"}).then(async response=>{const data=await response.json();if(!response.ok)throw new Error(data.error);if(active){setDocument(data);setDraft(data.content);onDirty(false);}}).catch(error=>{if(active)setError(error.message);});return()=>{active=false;};},[projectId,path,onDirty]);
  useEffect(()=>{const before=(event:BeforeUnloadEvent)=>{if(document&&draft!==document.content){event.preventDefault();event.returnValue="";}};window.addEventListener("beforeunload",before);return()=>window.removeEventListener("beforeunload",before);},[document,draft]);
  async function reload(){
    if(document&&draft!==document.content&&!window.confirm("Discard unsaved TOML edits and reload?"))return;
    setBusy(true);onBusy(true);setError("");setMessage("");
    try{const response=await fetch(`/api/project-toml?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}`,{cache:"no-store"});const result=await response.json();if(!response.ok)throw new Error(result.error);setDocument(result);setDraft(result.content);onDirty(false);}catch(error){setError(error instanceof Error?error.message:"Reload failed.");}finally{setBusy(false);onBusy(false);}
  }
  async function save(){
    if(!document)return;setBusy(true);onBusy(true);setError("");setMessage("");
    try{const response=await fetch("/api/project-toml",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path,content:draft,revision:document.revision})});const result=await response.json();if(!response.ok)throw new Error(result.error);setDocument(result);setDraft(result.content);onDirty(false);onSaved();setMessage(path.endsWith(".noise.toml")?"Saved and validated. Generate a new preview to use these noise settings.":"Saved and validated. Return to Scene or use Load to refresh source changes.");}catch(error){setError(error instanceof Error?error.message:"Save failed.");}finally{setBusy(false);onBusy(false);}
  }
  return <div className="toml-editor"><div className="toml-editor-head"><code>{path}</code><div><button disabled={busy||!document||draft===document.content} onClick={()=>void save()}>{busy?"Validating…":"Save TOML"}</button><button disabled={busy} onClick={()=>void reload()}>Reload TOML</button></div></div>
    <p className="toml-scope">{path.endsWith(".entity.toml")?"Shared entity definition: edits affect every instance referencing this asset.":path.endsWith(".noise.toml")?"Reusable noise filter: saves validate the definition through Rust.":"Whole scene definition: hierarchy and instance overrides are saved here."}</p>
    {error&&<p className="toml-error" role="alert">{error}</p>}{message&&<p className="toml-message" role="status">{message}</p>}
    <textarea aria-label="TOML source" spellCheck={false} wrap="off" value={draft} disabled={busy||!document} onChange={event=>{setDraft(event.target.value);setMessage("");onDirty(event.target.value!==document?.content);}} />
  </div>;
}
