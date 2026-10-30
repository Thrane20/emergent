"use client";
import { useLiveInspector } from "@/hooks/use-live-inspector";
import type { PhysicsSettings, SceneInstance } from "@/lib/scene-placement";

const defaults:PhysicsSettings={enabled:false,body:"dynamic",gravity_scale:1,mass:1,friction:0.5,restitution:0,collider:{shape:"box"}};
export function PhysicsInspector({instance,disabled,onApply,onDirty,onBusy}:{instance:SceneInstance;disabled:boolean;onApply:(value:PhysicsSettings|null)=>Promise<void>;onDirty:(dirty:boolean)=>void;onBusy:(busy:boolean)=>void}) {
  const initial:PhysicsSettings=instance.physics??(instance.terrain?{...defaults,body:"static",collider:{shape:"terrain"}}:defaults);
  const save=useLiveInspector(initial,next=>{
    for(const [key,min,max] of [["gravity_scale",0,100],["mass",0.0001,1000000],["friction",0,10],["restitution",0,1]] as const){const n=next[key];if(!Number.isFinite(n)||n<min||n>max)throw new Error(`Enter a valid ${key.replaceAll("_"," ")}.`);}
    for(const key of ["center","half_extents"] as const){const v=next.collider[key];if(v?.some(n=>!Number.isFinite(n)||Math.abs(n)>1000000||(key==="half_extents"&&n<0.0001)))throw new Error("Enter valid collider dimensions.");}
    return next;
  },onApply,onDirty,onBusy);
  const draft=save.draft,setDraft=save.change,blocked=disabled||save.saving;
  const auto=!draft.collider.half_extents;
  return <div className="physics-inspector">
    <h3>Physics</h3>
    <form onSubmit={event=>{event.preventDefault();}}>
      <fieldset disabled={blocked}>
        <label className="uniform-scale"><input type="checkbox" checked={draft.enabled} onChange={event=>setDraft({...draft,enabled:event.target.checked})}/>Physics enabled</label>
        {draft.enabled&&<>
          <label>Body<select disabled={draft.collider.shape==="terrain"} aria-label="Physics body" value={draft.body} onChange={event=>setDraft({...draft,body:event.target.value as PhysicsSettings["body"]})}><option value="dynamic">Dynamic — gravity and contacts</option><option value="static">Static — immovable collider</option></select></label>
          {draft.body==="dynamic"&&instance.parent&&<p className="transform-error">Move this instance to the scene root before enabling dynamic physics.</p>}
          {([["gravity_scale","Gravity scale",0,100],["mass","Mass (kg)",0.0001,1000000],["friction","Friction",0,10],["restitution","Bounce",0,1]] as const).map(([key,label,min,max])=><label key={key}>{label}<input aria-label={label} type="number" min={min} max={max} step="any" required value={Number.isNaN(draft[key])?"":draft[key]} onChange={event=>setDraft({...draft,[key]:event.target.valueAsNumber})}/></label>)}
          <label>Collider<select aria-label="Collider type" value={draft.collider.shape} onChange={event=>setDraft({...draft,body:event.target.value==="terrain"?"static":draft.body,collider:{shape:event.target.value as "box"|"terrain"}})}><option value="box">Box</option>{instance.terrain&&<option value="terrain">Terrain — follows surface</option>}</select></label>
          {draft.collider.shape==="terrain"&&<p className="transform-note">Static height-field collision follows the generated terrain surface.</p>}
          {draft.collider.shape==="box"&&<>
          <label className="uniform-scale"><input type="checkbox" checked={auto} onChange={event=>setDraft({...draft,collider:event.target.checked?{shape:"box"}:{shape:"box",center:[0,0,0],half_extents:[0.5,0.5,0.5]}})}/>Auto-fit from geometry</label>
          {!auto&&(["center","half_extents"] as const).map(key=><fieldset key={key}><legend>{key==="center"?"Collider centre":"Collider half-extents"}</legend><div className="transform-vector">{["X","Y","Z"].map((axis,index)=><label key={axis}>{axis}<input aria-label={`${key} ${axis}`} type="number" required min={key==="half_extents"?0.0001:undefined} step="any" value={Number.isNaN(draft.collider[key]?.[index])?"":draft.collider[key]?.[index]??0} onChange={event=>setDraft({...draft,collider:{...draft.collider,[key]:(draft.collider[key]??[0,0,0]).map((n,i)=>i===index?event.target.valueAsNumber:n)}})}/></label>)}</div></fieldset>)}
          </>}
        </>}
      </fieldset>
      {save.error&&<p role="alert" className="transform-error">{save.error}</p>}<p className="transform-note" role="status">{save.status}</p>
      <div className="transform-actions"><button type="button" disabled={blocked||(!instance.physicsOverridden&&!save.dirty)} onClick={save.reset}>Use asset defaults</button></div>
      <p className="transform-note">Saved on this scene instance. Inspection stays paused. Box outlines are shown; terrain collision follows its mesh. Restart Run to simulate. Colliders require positive scale; scene parenting does not create joints.</p>
    </form>
  </div>;
}
