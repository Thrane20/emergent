"use client";
import {createPortal} from "react-dom";
import {useEffect,useRef,useState} from "react";
import {useLiveInspector} from "@/hooks/use-live-inspector";
import {objectComponentTypes,type AuthoredComponent,type ComponentField,type ComponentPatch,type ComponentType,type ComponentValue} from "@/lib/components";
import {FieldInput,type FieldOption} from "@/components/field-input";
import {BrainModesEditor} from "@/components/brain-modes-editor";
import {fieldsFromModes,modesFromFields,modesProblem} from "@/lib/brain-modes";

type Draft={enabled:boolean;fields:Record<string,ComponentValue>};
type Instance={id:string;name:string};

/** Schema-driven component sections for one scene instance; unset fields come from the asset, then the schema default. */
export function ComponentsInspector({types,registryError,kind,authored,defaults,variant,instances,brains=[],profiles=[],clips=[],selfId,actions,onApply,onDirty,onBusy,disabled}:{
 actions:HTMLElement|null;types:ComponentType[]|null;registryError:string;kind:string|undefined;authored:AuthoredComponent[];defaults:Record<string,unknown>|undefined;variant:string|undefined;
 instances:Instance[];brains?:FieldOption[];profiles?:FieldOption[];clips?:FieldOption[];selfId:string;onApply:(type:string,patch:ComponentPatch|null)=>Promise<void>;onDirty:(dirty:boolean)=>void;onBusy:(busy:boolean)=>void;disabled:boolean;
}){
 const [adding,setAdding]=useState(false),[addError,setAddError]=useState("");
 const [choosing,setChoosing]=useState(false),[search,setSearch]=useState(""),dialog=useRef<HTMLDialogElement>(null);
 useEffect(()=>{if(choosing)dialog.current?.showModal();},[choosing]);
 const shown=types?objectComponentTypes(types,kind,authored):[];
 const addable=types?.filter(type=>type.applies_to==="any"&&!authored.some(component=>component.type===type.id))??[];
 async function add(type:ComponentType){
  setAdding(true);setAddError("");
  // Required fields without a default (a follow target) need a starting value for the component to load.
  const first={entity:instances.find(item=>item.id!==selfId)?.id,brain:brains[0]?.id,locomotion:profiles[0]?.id,audio:clips[0]?.id,text:"group"} as Record<string,string|undefined>;
  const required=Object.fromEntries(type.fields.filter(field=>field.default===undefined&&first[field.kind]).map(field=>[field.name,first[field.kind]!]));
  // Nothing to pick yet: say where to make one instead of saving a component the engine rejects.
  const missing=type.fields.find(field=>field.default===undefined&&!first[field.kind]);
  if(missing){
   const where={brain:"No brains in this project yet. Create one in the Brain Editor, then add the Brain component.",locomotion:"No locomotion profiles in this project yet. Create one on the Locomotion page (for a drone: New locomotion → Drone → Save profile), then add the Locomotion component.",entity:"Add another object to the scene first; this component needs one to point at.",audio:"No audio clips in this project yet. Add a .audio.toml clip under assets/audio, then add the Audio source component."}[missing.kind as string]??`${type.label} needs ${missing.label.toLowerCase()} first.`;
   setAddError(where);setAdding(false);return;
  }
  try{await onApply(type.id,required);setChoosing(false);}catch(error){setAddError(error instanceof Error?error.message:"Could not add the component.");}finally{setAdding(false);}
 }
 return <section className="components-inspector" aria-label="Components">
  {registryError&&<p className="transform-error" role="alert">Component types unavailable: {registryError}</p>}
  {!types&&!registryError&&<p className="transform-note">Loading component types…</p>}
  {shown.map(type=>{const current=authored.find(component=>component.type===type.id);return <ComponentSection key={type.id} scope={selfId} type={type} current={current} added={type.applies_to==="any"} defaults={type.applies_to===kind?defaults:undefined} variant={variant} instances={instances.filter(item=>item.id!==selfId)} brains={brains} profiles={profiles} clips={clips} onApply={patch=>onApply(type.id,patch)} onDirty={onDirty} onBusy={onBusy} disabled={disabled}/>;})}
  {actions&&createPortal(<button className="action-primary" type="button" disabled={disabled||adding||!types} onClick={()=>{setSearch("");setAddError("");setChoosing(true);}}>＋ Add component</button>,actions)}
  {choosing&&<dialog ref={dialog} className="component-picker" aria-label="Add component" onCancel={e=>{e.preventDefault();if(!adding){setChoosing(false);setAddError("");}}}>
   <header><h2>Add component</h2><p>Select a component to add to this object.</p></header>
   <label className="component-picker-search">Search components<input autoFocus type="search" aria-label="Search components" value={search} disabled={adding} onChange={e=>setSearch(e.target.value)}/></label>
   <div className="component-picker-list">{addable.filter(type=>`${type.label} ${type.description} ${type.id}`.toLowerCase().includes(search.toLowerCase())).map(type=><button key={type.id} type="button" disabled={adding||disabled} onClick={()=>void add(type)}><b>{type.label}</b><span>{type.description}</span></button>)}
   {!addable.some(type=>`${type.label} ${type.description} ${type.id}`.toLowerCase().includes(search.toLowerCase()))&&<p>No matching components.</p>}</div>
   {addError&&<p className="transform-error" role="alert">{addError}</p>}
   <footer><button type="button" disabled={adding} onClick={()=>{setChoosing(false);setAddError("");}}>Cancel</button></footer>
  </dialog>}
  {addError&&!choosing&&<p className="transform-error" role="alert">{addError}</p>}
 </section>;
}

function ComponentSection({scope,type,current,added,defaults,variant,instances,brains,profiles,clips,onApply,onDirty,onBusy,disabled}:{scope:string;type:ComponentType;current:AuthoredComponent|undefined;added:boolean;defaults:Record<string,unknown>|undefined;variant:string|undefined;instances:Instance[];brains:FieldOption[];profiles:FieldOption[];clips:FieldOption[];onApply:(patch:ComponentPatch|null)=>Promise<void>;onDirty:(dirty:boolean)=>void;onBusy:(busy:boolean)=>void;disabled:boolean}){
 const initial:Draft={enabled:current?.enabled??true,fields:current?.fields??{}};
 // The Brain card edits brain, stack and modes together as mode panels.
 const brainModes=type.id==="emergent.brain";
 const fields=type.fields.filter(field=>(!field.variants?.length||variant!==undefined&&field.variants.includes(variant))&&!(brainModes&&["brain","stack","modes"].includes(field.name)));
 // The full patch replaces this component's authored fields; absent fields fall back to the asset or schema default.
 const prepare=(draft:Draft):ComponentPatch=>{
  if(brainModes){const problem=modesProblem(modesFromFields(draft.fields));if(problem)throw new Error(problem);}
  if(Object.values(draft.fields).some(v=>typeof v==="number"&&!Number.isFinite(v)||Array.isArray(v)&&v.some(n=>typeof n==="number"&&!Number.isFinite(n))))throw new Error("Enter a number.");
  return Object.fromEntries([["enabled",draft.enabled],...type.fields.map(field=>[field.name,draft.fields[field.name]??null])]);
 };
 const save=useLiveInspector(initial,prepare,patch=>onApply(patch),onDirty,onBusy),draft=save.draft,blocked=disabled||save.saving;
 const value=(field:ComponentField)=>draft.fields[field.name]??defaults?.[field.name] as ComponentValue|undefined??field.default;
 const set=(name:string,next:ComponentValue|undefined)=>{const nextFields={...draft.fields};if(next===undefined)delete nextFields[name];else nextFields[name]=next;save.change({...draft,fields:nextFields});};
 const setMany=(patch:ComponentPatch)=>{const nextFields={...draft.fields};for(const [name,next] of Object.entries(patch)){if(next===null)delete nextFields[name];else nextFields[name]=next;}save.change({...draft,fields:nextFields});};
 const [open,setOpen]=useState(true);
 return <div className={`component-section${open?"":" collapsed"}`} data-component={type.id}>
  <header><h3><ComponentIcon id={type.id}/>{type.label}</h3><label className="component-enabled"><input type="checkbox" aria-label={`${type.label} enabled`} checked={draft.enabled} disabled={blocked} onChange={event=>save.change({...draft,enabled:event.target.checked})}/>Enabled</label><button type="button" className="component-toggle" aria-expanded={open} aria-label={`${open?"Collapse":"Expand"} ${type.label}`} title={open?"Collapse":"Expand"} onClick={()=>setOpen(value=>!value)}><span className="component-chevron" aria-hidden="true">▾</span></button></header>
  <div className="component-body">{type.description&&<p className="transform-note">{type.description}</p>}
  <form onSubmit={event=>event.preventDefault()}><fieldset disabled={blocked}>
   {brainModes&&<BrainModesEditor scope={scope} modes={modesFromFields(draft.fields)} brains={brains} onChange={modes=>setMany(fieldsFromModes(modes))}/>}
   {fields.map(field=>{const overridden=draft.fields[field.name]!==undefined,label=`${type.label} ${field.label.toLowerCase()}`;return <div className="component-field" key={field.name} title={field.help}>
    <span>{field.label}{field.unit?` (${field.unit})`:""}</span>
    <FieldInput field={field} label={label} value={value(field)} instances={instances} brains={brains} profiles={profiles} clips={clips} onChange={next=>set(field.name,next===""?undefined:next)}/>
    <button type="button" className="component-reset" aria-label={`Reset ${label}`} title={added?"Use the default":"Use the asset value"} disabled={!overridden||field.kind==="entity"&&field.default===undefined} onClick={()=>set(field.name,undefined)}>↺</button>
   </div>;})}
  </fieldset>{save.error&&<p className="transform-error" role="alert">{save.error}</p>}<p className="transform-note" role="status">{save.status}</p>
  <div className="transform-actions"><button type="button" disabled={blocked||!added&&!current&&!save.dirty} onClick={save.reset}>{added?"Remove component":"Use asset defaults"}</button></div></form></div>
 </div>;
}

const ICONS:Record<string,string>={
 "emergent.light":"M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2.1h5c0-.9.4-1.6 1-2.1A6 6 0 0 0 12 3Z",
 "emergent.character_controller":"M12 4a2 2 0 1 0 0 .01M12 8v7M8 11l4-3 4 3M9 21l3-6 3 6",
 "emergent.spin":"M20 12a8 8 0 1 1-2.3-5.7M20 4v4h-4",
 "emergent.follow":"M4 12h12M12 7l5 5-5 5M20 5v14",
 "emergent.group":"M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
 "emergent.brain":"M9 4a3 3 0 0 0-3 3 3 3 0 0 0-2 5 3 3 0 0 0 2 5 3 3 0 0 0 6 1V4a3 3 0 0 0-3 0ZM15 4a3 3 0 0 1 3 3 3 3 0 0 1 2 5 3 3 0 0 1-2 5 3 3 0 0 1-6 1",
 "emergent.locomotion":"M3 17h3a2 2 0 1 0 4 0h4a2 2 0 1 0 4 0h1v-5l-3-4H5l-2 4ZM5 12h14",
 "emergent.look_at":"M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12ZM12 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5Z",
 "emergent.follow_camera":"M3 8h13v9H3zM16 11l5-3v9l-5-3",
 "emergent.audio_source":"M4 9v6h4l5 4V5L8 9ZM16 9a4 4 0 0 1 0 6M18.5 6.5a8 8 0 0 1 0 11",
 "emergent.audio_listener":"M5 14v-2a7 7 0 0 1 14 0v2M4 14h3v6H4zM17 14h3v6h-3z",
 "emergent.ui":"M3 5h18v14H3zM3 9h18M7 14h6",
 "emergent.material":"M12 3a9 9 0 1 0 0 18c1.5 0 2-1 1.5-2s0-2 1.5-2h2a3 3 0 0 0 3-3A9 9 0 0 0 12 3ZM8 11h.01M12 8h.01M16 11h.01",
};
const FALLBACK_ICON="M12 3l8 4.5v9L12 21l-8-4.5v-9ZM12 12l8-4.5M12 12v9M12 12L4 7.5";
function ComponentIcon({id}:{id:string}){return <svg className="component-icon" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={ICONS[id]??FALLBACK_ICON}/></svg>;}
