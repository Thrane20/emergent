"use client";
import {useState} from "react";
import {pairValue,pairsValue,type ComponentField,type ComponentValue,type PairValue} from "@/lib/components";
import {hexToLinear,linearToHex} from "@/lib/material-settings";

export type FieldOption={id:string;name:string};
const numbers=(value:ComponentValue|undefined)=>Array.isArray(value)&&value.every(n=>typeof n==="number")?value as number[]:undefined;

/** One schema field (components and brain nodes share these kinds). Empty text clears the field. */
export function FieldInput({field,label,value,instances,brains=[],profiles=[],clips=[],onChange}:{field:ComponentField;label:string;value:ComponentValue|undefined;instances:FieldOption[];brains?:FieldOption[];profiles?:FieldOption[];clips?:FieldOption[];onChange:(value:ComponentValue)=>void}){
 const select=(options:FieldOption[],current:string|undefined)=><select aria-label={label} value={current??""} onChange={event=>onChange(event.target.value)}>
  <option value="">{field.default===undefined?"Choose…":"Default"}</option>
  {current&&!options.some(item=>item.id===current)&&<option value={current}>{current} (missing)</option>}
  {options.map(item=><option key={item.id} value={item.id}>{item.name}</option>)}
 </select>;
 switch(field.kind){
  case "colour":return <input type="color" aria-label={label} value={linearToHex(numbers(value)??[1,1,1])} onChange={event=>onChange(hexToLinear(event.target.value))}/>;
  case "bool":return <input type="checkbox" aria-label={label} checked={value===true} onChange={event=>onChange(event.target.checked)}/>;
  case "entity":return select(instances,typeof value==="string"?value:undefined);
  case "brain":return select(brains,typeof value==="string"?value:undefined);
  case "locomotion":return select(profiles,typeof value==="string"?value:undefined);
  case "audio":return select(clips,typeof value==="string"?value:undefined);
  // The inspector edits modes in BrainModesEditor; elsewhere they show read-only.
  case "modes":{const modes=Array.isArray(value)?value.filter((m):m is Record<string,string|boolean>=>!!m&&typeof m==="object"&&!Array.isArray(m)):[];return <span aria-label={label}>{modes.length?modes.map(m=>`${m.name}${m.active===true?" (on)":""}`).join(", "):"None"}</span>;}
  case "pairs":return <PairsInput label={label} value={pairsValue(value)?value:{}} onChange={onChange}/>;
  case "text":return <input type="text" aria-label={label} maxLength={field.max ?? 64} value={typeof value==="string"?value:""} placeholder={typeof field.default==="string"?field.default:""} onChange={event=>onChange(event.target.value)}/>;
  case "choice":return <select aria-label={label} value={typeof value==="string"?value:typeof field.default==="string"?field.default:""} onChange={event=>onChange(event.target.value)}>{(field.options??[]).map(option=><option key={option} value={option}>{option.replaceAll("_"," ")}</option>)}</select>;
  case "entities":case "brains":{
   const options=field.kind==="brains"?brains:instances;
   const list=Array.isArray(value)?value.filter((item):item is string=>typeof item==="string"):[];
   const name=(id:string)=>options.find(item=>item.id===id)?.name??`${id} (missing)`;
   return <span className="field-list" aria-label={label}>
    {list.map((id,i)=><span className="field-chip" key={`${id}:${i}`}>{i+1}. {name(id)}<button type="button" aria-label={`Remove ${name(id)} from ${label}`} onClick={()=>onChange(list.filter((_,j)=>j!==i))}>×</button></span>)}
    <select aria-label={`Add to ${label}`} value="" onChange={event=>{if(event.target.value)onChange([...list,event.target.value]);}}><option value="">Add…</option>{options.filter(item=>field.kind!=="brains"||!list.includes(item.id)).map(item=><option key={item.id} value={item.id}>{item.name}</option>)}</select>
   </span>;
  }
  case "vector3":{const vector=numbers(value)?.length===3?numbers(value)!:[0,0,0];return <span className="component-vector">{["X","Y","Z"].map((axis,i)=><input key={axis} type="number" aria-label={`${label} ${axis}`} min={field.min} max={field.max} step="any" value={Number.isFinite(vector[i])?vector[i]:""} onChange={event=>onChange(vector.map((n,j)=>j===i?event.target.valueAsNumber:n))}/>)}</span>;}
  default:return <input type="number" aria-label={label} min={field.min} max={field.max} step="any" value={typeof value==="number"&&Number.isFinite(value)?value:""} placeholder={typeof field.default==="number"?String(field.default):""} onChange={event=>onChange(event.target.valueAsNumber)}/>;
 }
}


/** Name = value rows for metadata: edit, remove, and add a row. Names must be unique. */
function PairsInput({label,value,onChange}:{label:string;value:Record<string,PairValue>;onChange:(value:ComponentValue)=>void}){
 const [name,setName]=useState(""),[text,setText]=useState(""),entries=Object.entries(value);
 const add=()=>{const key=name.trim();if(!key||key.length>64||key in value||entries.length>=32)return;onChange({...value,[key]:pairValue(text)});setName("");setText("");};
 return <span className="field-pairs" aria-label={label}>
  {entries.map(([key,item])=><span className="field-pair" key={`${key}=${String(item)}`}><b title={key}>{key}</b>
   <input type="text" aria-label={`${label}: ${key}`} maxLength={256} defaultValue={String(item)} onBlur={event=>{const next=pairValue(event.target.value);if(next!==item)onChange({...value,[key]:next});}}/>
   <button type="button" aria-label={`Remove ${key}`} onClick={()=>{const rest={...value};delete rest[key];onChange(rest);}}>×</button></span>)}
  {entries.length<32&&<span className="field-pair field-pair-new"><input type="text" aria-label={`New ${label} name`} placeholder="name" maxLength={64} value={name} onChange={event=>setName(event.target.value)}/><input type="text" aria-label={`New ${label} value`} placeholder="value" maxLength={256} value={text} onChange={event=>setText(event.target.value)} onKeyDown={event=>{if(event.key==="Enter"){event.preventDefault();add();}}}/><button type="button" aria-label={`Add to ${label}`} disabled={!name.trim()||name.trim() in value} onClick={add}>+</button></span>}
 </span>;
}
