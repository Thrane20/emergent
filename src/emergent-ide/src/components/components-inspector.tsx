"use client";
import {useState} from "react";
import {useLiveInspector} from "@/hooks/use-live-inspector";
import {objectComponentTypes,type AuthoredComponent,type ComponentField,type ComponentPatch,type ComponentType,type ComponentValue} from "@/lib/components";
import {FieldInput,type FieldOption} from "@/components/field-input";

type Draft={enabled:boolean;fields:Record<string,ComponentValue>};
type Instance={id:string;name:string};

/** Schema-driven component sections for one scene instance; unset fields come from the asset, then the schema default. */
export function ComponentsInspector({types,registryError,kind,authored,defaults,variant,instances,brains=[],selfId,onApply,onDirty,onBusy,disabled}:{
 types:ComponentType[]|null;registryError:string;kind:string|undefined;authored:AuthoredComponent[];defaults:Record<string,unknown>|undefined;variant:string|undefined;
 instances:Instance[];brains?:FieldOption[];selfId:string;onApply:(type:string,patch:ComponentPatch|null)=>Promise<void>;onDirty:(dirty:boolean)=>void;onBusy:(busy:boolean)=>void;disabled:boolean;
}){
 const [adding,setAdding]=useState(false),[addError,setAddError]=useState("");
 const shown=types?objectComponentTypes(types,kind,authored):[];
 const addable=types?.filter(type=>type.applies_to==="any"&&!authored.some(component=>component.type===type.id))??[];
 async function add(type:ComponentType){
  setAdding(true);setAddError("");
  // Required fields without a default (a follow target) need a starting value for the component to load.
  const first={entity:instances.find(item=>item.id!==selfId)?.id,brain:brains[0]?.id,text:"group"} as Record<string,string|undefined>;
  const required=Object.fromEntries(type.fields.filter(field=>field.default===undefined&&first[field.kind]).map(field=>[field.name,first[field.kind]!]));
  try{await onApply(type.id,required);}catch(error){setAddError(error instanceof Error?error.message:"Could not add the component.");}finally{setAdding(false);}
 }
 return <section className="components-inspector" aria-label="Components">
  {registryError&&<p className="transform-error" role="alert">Component types unavailable: {registryError}</p>}
  {!types&&!registryError&&<p className="transform-note">Loading component types…</p>}
  {shown.map(type=>{const current=authored.find(component=>component.type===type.id);return <ComponentSection key={type.id} type={type} current={current} added={type.applies_to==="any"} defaults={type.applies_to===kind?defaults:undefined} variant={variant} instances={instances.filter(item=>item.id!==selfId)} brains={brains} onApply={patch=>onApply(type.id,patch)} onDirty={onDirty} onBusy={onBusy} disabled={disabled}/>;})}
  <label className="components-add">Add Component<select aria-label="Add component" value="" disabled={disabled||adding||!addable.length} onChange={event=>{const type=addable.find(item=>item.id===event.target.value);if(type)void add(type);}}>
   <option value="">{addable.length?"Choose…":"No more types"}</option>
   {addable.map(type=><option key={type.id} value={type.id} title={type.description}>{type.label}</option>)}
  </select></label>
  {addError&&<p className="transform-error" role="alert">{addError}</p>}
 </section>;
}

function ComponentSection({type,current,added,defaults,variant,instances,brains,onApply,onDirty,onBusy,disabled}:{type:ComponentType;current:AuthoredComponent|undefined;added:boolean;defaults:Record<string,unknown>|undefined;variant:string|undefined;instances:Instance[];brains:FieldOption[];onApply:(patch:ComponentPatch|null)=>Promise<void>;onDirty:(dirty:boolean)=>void;onBusy:(busy:boolean)=>void;disabled:boolean}){
 const initial:Draft={enabled:current?.enabled??true,fields:current?.fields??{}};
 const fields=type.fields.filter(field=>!field.variants?.length||variant!==undefined&&field.variants.includes(variant));
 // The full patch replaces this component's authored fields; absent fields fall back to the asset or schema default.
 const prepare=(draft:Draft):ComponentPatch=>{
  if(Object.values(draft.fields).some(v=>typeof v==="number"&&!Number.isFinite(v)||Array.isArray(v)&&v.some(n=>typeof n==="number"&&!Number.isFinite(n))))throw new Error("Enter a number.");
  return Object.fromEntries([["enabled",draft.enabled],...type.fields.map(field=>[field.name,draft.fields[field.name]??null])]);
 };
 const save=useLiveInspector(initial,prepare,patch=>onApply(patch),onDirty,onBusy),draft=save.draft,blocked=disabled||save.saving;
 const value=(field:ComponentField)=>draft.fields[field.name]??defaults?.[field.name] as ComponentValue|undefined??field.default;
 const set=(name:string,next:ComponentValue|undefined)=>{const nextFields={...draft.fields};if(next===undefined)delete nextFields[name];else nextFields[name]=next;save.change({...draft,fields:nextFields});};
 return <div className="component-section" data-component={type.id}>
  <header><h3>{type.label}</h3><label className="component-enabled"><input type="checkbox" aria-label={`${type.label} enabled`} checked={draft.enabled} disabled={blocked} onChange={event=>save.change({...draft,enabled:event.target.checked})}/>Enabled</label></header>
  {type.description&&<p className="transform-note">{type.description}</p>}
  <form onSubmit={event=>event.preventDefault()}><fieldset disabled={blocked}>
   {fields.map(field=>{const overridden=draft.fields[field.name]!==undefined,label=`${type.label} ${field.label.toLowerCase()}`;return <div className="component-field" key={field.name} title={field.help}>
    <span>{field.label}{field.unit?` (${field.unit})`:""}</span>
    <FieldInput field={field} label={label} value={value(field)} instances={instances} brains={brains} onChange={next=>set(field.name,next===""?undefined:next)}/>
    <button type="button" className="component-reset" aria-label={`Reset ${label}`} title={added?"Use the default":"Use the asset value"} disabled={!overridden||field.kind==="entity"&&field.default===undefined} onClick={()=>set(field.name,undefined)}>↺</button>
   </div>;})}
  </fieldset>{save.error&&<p className="transform-error" role="alert">{save.error}</p>}<p className="transform-note" role="status">{save.status}</p>
  <div className="transform-actions"><button type="button" disabled={blocked||!added&&!current&&!save.dirty} onClick={save.reset}>{added?"Remove component":"Use asset defaults"}</button></div></form>
 </div>;
}
