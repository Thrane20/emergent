"use client";

import { useState } from "react";
import { useLiveInspector } from "@/hooks/use-live-inspector";
import type { SceneInstance, TransformSpace, TransformValues } from "@/lib/scene-placement";
import { validParents } from "@/lib/scene-hierarchy";
import {lookDirection} from "@/lib/inspection-navigation";

const groups = [["translation","Position"],["rotation_degrees","Rotation (degrees)"],["scale","Scale"]] as const;

export function TransformInspector({ instance, disabled, live = false, space="local",instances,onReparent,onApply, onReload,onDirty,onBusy }: {
  instance: SceneInstance;
  disabled: boolean;
  live?: boolean;
  space?:TransformSpace;
  instances?:SceneInstance[];
  onReparent?:(parent:string|null)=>Promise<void>;
  onApply: (value: TransformValues | null) => Promise<void>;
  onReload: () => void;
  onDirty:(dirty:boolean)=>void;
  onBusy:(busy:boolean)=>void;
}) {
  const values=instance.display??instance.transform;
  const translationOnly=space==="global"&&instance.globalTranslationOnly;
  const initial=Object.fromEntries(groups.map(([key])=>[key,values[key].map(String)])) as Record<keyof TransformValues,string[]>;
  const [uniform,setUniform]=useState(values.scale.every(n=>n===values.scale[0]));
  const [parentError,setParentError]=useState("");
  const save=useLiveInspector(initial,next=>{
    const value={} as TransformValues;
    for(const [key] of groups){if(next[key].some(text=>!text.trim()||!Number.isFinite(Number(text))))throw new Error("Enter a finite number in every field.");value[key]=next[key].map(Number);}
    if(value.scale.some(n=>space==="global"?n===0:Math.abs(n)<0.0001))throw new Error("Scale axes must be non-zero.");
    return value;
  },onApply,onDirty,onBusy);
  const draft=save.draft,blocked=disabled||save.saving;
  const direction=lookDirection(instance,instances??[instance]);
  return <div className="transform-inspector">
    <div className="instance-context"><b title={instance.id}>{instance.name??instance.id}</b><span>Asset · {instance.asset.split("/").at(-1)}</span>{instances&&onReparent?<label>Parent<select aria-label="Parent" value={instance.parent??""} disabled={blocked} onChange={event=>{setParentError("");void onReparent(event.target.value||null).catch(error=>setParentError(error.message));}}><option value="">Scene root</option>{validParents(instances,instance.id).map(parent=><option value={parent.id} key={parent.id} title={parent.id}>{parent.name??parent.id}</option>)}</select></label>:<span>Parent · {instances?.find(parent=>parent.id===instance.parent)?.name??instance.parent??"Scene root"}</span>}</div>
    <p className="transform-note">{space==="global"?"Global coordinates":"Local to parent"} · {instance.overridden ? "Scene override" : "Asset defaults"}</p>
    {translationOnly&&<p className="transform-note">This hierarchy produces shear. Global position is editable; use Local mode for rotation and scale.</p>}
    <p className="transform-note" id="transform-units-help">1 world unit = 1 m. Position is in metres; scale is a multiplier of the asset’s dimensions. Scale 1× keeps the authored size.</p>
    <form onSubmit={event => { event.preventDefault(); }}>
      {groups.map(([key,label]) => <fieldset disabled={blocked||Boolean(translationOnly&&key!=="translation")} key={key}>
        <legend>{label}{key==="translation"?" (m)":key==="scale"?" (×)":""}</legend>
        <div className="transform-vector">{["X","Y","Z"].map((axis,index) => <label key={axis}><span>{axis}</span><input
          aria-label={`${label} ${axis}`} aria-describedby="transform-units-help" type="number" step="any" required value={translationOnly&&key!=="translation"?"":draft[key][index]}
          disabled={key === "scale" && uniform && index > 0}
          onChange={event => {
            const text = event.target.value;
            save.change({...draft,[key]: key === "scale" && uniform ? [text,text,text] : draft[key].map((n,i) => i === index ? text : n)});
          }}
        /></label>)}</div>
      </fieldset>)}
      <label className="uniform-scale"><input type="checkbox" checked={uniform} disabled={blocked||Boolean(translationOnly)} onChange={event => {
        setUniform(event.target.checked);
        if (event.target.checked) save.change({...draft,scale:[draft.scale[0],draft.scale[0],draft.scale[0]]});
      }} />Uniform scale</label>
      {(save.error||parentError)&&<p className="transform-error" role="alert">{save.error||parentError}</p>}
      <p className="transform-note" role="status">{save.status}</p>
      <fieldset className="look-direction"><legend>Look-at direction · world</legend><div className="transform-vector">{["X","Y","Z"].map((axis,index)=><label key={axis}><span>{axis}</span><input aria-label={`Look-at ${axis}`} readOnly value={direction?direction[index].toFixed(4):"—"}/></label>)}</div><p className="transform-note">Normalized forward (-Z), including parent transforms. F: frame selection · Shift-F: move selection to editor view.</p></fieldset>
      <div className="transform-actions"><button type="button" disabled={blocked||(!instance.overridden&&!save.dirty)} onClick={save.reset}>Reset to asset defaults</button><button type="button" disabled={blocked} onClick={onReload}>Reload inspector</button></div>
      <p className="transform-note">{live ? "Applies to the retained inspection world. Restart Run to use saved placement." : "Updates the paused preview. Restart Run to use saved placement."}</p>
    </form>
  </div>;
}
