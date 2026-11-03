"use client";
import {ObjectIcon} from "./object-icon";
import { useRef, useState } from "react";
import { sceneHierarchy } from "@/lib/scene-hierarchy";
import type { SceneInstance,ScenePlacement } from "@/lib/scene-placement";

import { prefabDrop, prefabMime, sceneInstanceDrop, sceneInstanceMime as mime } from "@/lib/scene-drag";
export function SceneGraph({projectId,placement,selected,disabled,query="",onSelect,onReparent,onRename,onDelete,onDuplicate,onInstantiate}:{projectId:string;placement:ScenePlacement;selected:string|null;disabled:boolean;query?:string;onSelect:(instance:SceneInstance)=>boolean;onReparent:(id:string,parent:string|null,revision:string)=>Promise<void>;onRename:(id:string,name:string,revision:string)=>Promise<void>;onDelete:(id:string,revision:string)=>Promise<boolean>;onDuplicate:(id:string,revision:string)=>Promise<string|undefined>;onInstantiate:(asset:string,parent:string|null,revision:string)=>Promise<void>}) {
  const [closed,setClosed]=useState(new Set<string>()),[error,setError]=useState("");
  const [editing,setEditing]=useState<{id:string;name:string;revision:string}|null>(null),[saving,setSaving]=useState(false);
  const pending=useRef(false),deleting=useRef(false),duplicating=useRef(false),cancelled=useRef(false),root=useRef<HTMLElement>(null);
  function focusRow(id:string){requestAnimationFrame(()=>root.current?.querySelector<HTMLButtonElement>(`button[data-instance-id="${CSS.escape(id)}"]`)?.focus());}
  async function saveName(){
    if(!editing||pending.current||cancelled.current)return;
    const edit=editing;pending.current=true;setSaving(true);setError("");
    try{await onRename(edit.id,edit.name,edit.revision);setEditing(null);focusRow(edit.id);}
    catch(error){setError(error instanceof Error?error.message:"Rename failed.");}
    finally{pending.current=false;setSaving(false);}
  }
  const rows=sceneHierarchy(placement.instances),parents=new Set(placement.instances.map(item=>item.parent));
  const matches=new Set<string>(),nodes=new Map(placement.instances.map(node=>[node.id,node]));
  if(query)for(const node of placement.instances)if(`${node.name??""} ${node.id} ${node.asset}`.toLowerCase().includes(query.toLowerCase())){let cursor:SceneInstance|undefined=node;while(cursor){matches.add(cursor.id);cursor=cursor.parent?nodes.get(cursor.parent):undefined;}}
  const visible:typeof rows=[];let hiddenDepth:number|null=null;
  for(const row of rows){if(query&&!matches.has(row.instance.id))continue;if(!query&&hiddenDepth!==null&&row.depth>hiddenDepth)continue;hiddenDepth=null;visible.push(row);if(!query&&closed.has(row.instance.id))hiddenDepth=row.depth;}
  const over=(event:React.DragEvent)=>{if(!disabled&&(event.dataTransfer.types.includes(mime)||event.dataTransfer.types.includes(prefabMime))){event.preventDefault();event.dataTransfer.dropEffect=event.dataTransfer.types.includes(prefabMime)?"copy":"move";}};
  async function drop(event:React.DragEvent,parent:string|null){
    event.preventDefault();event.stopPropagation();if(disabled)return;setError("");
    try{if(event.dataTransfer.types.includes(prefabMime))await onInstantiate(prefabDrop(event.dataTransfer.getData(prefabMime),projectId,placement),parent,placement.revision);else await onReparent(sceneInstanceDrop(event.dataTransfer.getData(mime),projectId,placement),parent,placement.revision);setClosed(previous=>{const next=new Set(previous);if(parent)next.delete(parent);return next;});}
    catch(error){setError(error instanceof Error?error.message:"Scene drop failed.");}
  }
  async function duplicate(id:string) {
    if(disabled||editing||duplicating.current)return;
    duplicating.current=true;setError("");
    try{const created=await onDuplicate(id,placement.revision);if(created)focusRow(created);}
    catch(error){setError(error instanceof Error?error.message:"Duplicate failed.");}
    finally{duplicating.current=false;}
  }
  async function deleteSubtree(instance:SceneInstance,descendants:number) {
    if(disabled||deleting.current)return;
    const label=instance.name??instance.id;
    const detail=descendants?` and ${descendants} descendant${descendants===1?"":"s"}`:"";
    if(!window.confirm(`Delete "${label}"${detail} from this scene? The asset files will remain in the project.`))return;
    deleting.current=true;setError("");
    try{await onDelete(instance.id,placement.revision);}
    catch(error){setError(error instanceof Error?error.message:"Delete failed.");}
    finally{deleting.current=false;}
  }
  return <section ref={root} className="scene-graph" aria-label="Scene instances" onKeyDown={event=>{
    if(event.key.toLowerCase()!=="d"||event.repeat||event.nativeEvent.isComposing||event.ctrlKey||event.metaKey||event.altKey||event.shiftKey||!selected||(event.target instanceof HTMLElement&&event.target.closest("input,textarea,select,[contenteditable]")))return;
    event.preventDefault();event.stopPropagation();void duplicate(selected);
  }} onDragOver={over} onDrop={event=>void drop(event,null)}>
    <div className="scene-root" title="Drop a prefab to add it at the scene root, or an instance to unparent it" onDragOver={over} onDrop={event=>void drop(event,null)}><b>Scene root</b><small>Drop to add / unparent</small></div>
    {error&&<p className="graph-error" role="alert">{error}</p>}
    <div role="tree" aria-label="Scene graph">{visible.map(({instance,depth})=>{
      const index=rows.findIndex(row=>row.instance.id===instance.id);
      const label=instance.name??instance.id;
      let descendants=0;
      for(let child=index+1;child<rows.length&&rows[child].depth>depth;child++)descendants++;
      return <div role="treeitem" aria-label={instance.id} aria-level={depth+1} aria-selected={selected===instance.id} aria-expanded={parents.has(instance.id)?!closed.has(instance.id):undefined} key={instance.id} className={`scene-node${selected===instance.id?" selected":""}`} style={{paddingLeft:8+Math.min(depth,12)*14}} draggable={!disabled&&!editing}
      onDragStart={event=>{if(!onSelect(instance)){event.preventDefault();return;}event.dataTransfer.effectAllowed="copyMove";event.dataTransfer.setData(mime,JSON.stringify({projectId,id:instance.id,scene:placement.path,revision:placement.revision}));}}
      onDragOver={over} onDrop={event=>void drop(event,instance.id)}>
      {parents.has(instance.id)?<button className="node-toggle" aria-label={`${closed.has(instance.id)?"Expand":"Collapse"} ${instance.id}`} onClick={()=>setClosed(previous=>{const next=new Set(previous);if(next.has(instance.id))next.delete(instance.id);else next.add(instance.id);return next;})}>{closed.has(instance.id)?"▸":"▾"}</button>:<span className="node-leaf" aria-hidden="true"/>}
      <ObjectIcon prefab={Boolean(instance.prefabLabel)}/>
      {editing?.id===instance.id ? <input className="node-rename" aria-label={`Rename instance ${instance.id}`} autoFocus maxLength={128} disabled={saving} value={editing.name} onFocus={event=>event.currentTarget.select()} onChange={event=>setEditing({...editing,name:event.target.value})} onBlur={()=>void saveName()} onKeyDown={event=>{
        if(event.nativeEvent.isComposing)return;
        if(event.key==="Enter"){event.preventDefault();event.stopPropagation();void saveName();}
        else if(event.key==="Escape"){event.preventDefault();event.stopPropagation();cancelled.current=true;setEditing(null);setError("");focusRow(instance.id);}
      }}/> : <button className="node-name" data-instance-id={instance.id} disabled={disabled} aria-label={`Select instance ${instance.id}`} onClick={()=>onSelect(instance)} title="Enter to rename" onKeyDown={event=>{if(event.key==="Enter"&&!event.nativeEvent.isComposing&&selected===instance.id&&!disabled){event.preventDefault();event.stopPropagation();cancelled.current=false;setError("");setEditing({id:instance.id,name:instance.name??instance.id,revision:placement.revision});}}}>{instance.name??instance.id}</button>}
      <button type="button" className="node-duplicate" aria-label={`Duplicate ${label}`} title={`Duplicate ${label} (D)`} disabled={disabled||Boolean(editing)} onClick={event=>{event.stopPropagation();void duplicate(instance.id);}}>⧉</button>
      <button type="button" className="node-delete" aria-label={`Delete ${label}`} title={`Delete ${label} and descendants`} disabled={disabled||Boolean(editing)} onClick={event=>{event.stopPropagation();void deleteSubtree(instance,descendants);}}>×</button>
    </div>})}</div>
    {!rows.length&&<p className="empty">Add an asset to start this scene.</p>}
  </section>;
}
