"use client";
import type {ComponentField,ComponentValue} from "@/lib/components";
import {hexToLinear,linearToHex} from "@/lib/material-settings";

export type FieldOption={id:string;name:string};
const numbers=(value:ComponentValue|undefined)=>Array.isArray(value)&&value.every(n=>typeof n==="number")?value as number[]:undefined;

/** One schema field (components and brain nodes share these kinds). Empty text clears the field. */
export function FieldInput({field,label,value,instances,brains=[],onChange}:{field:ComponentField;label:string;value:ComponentValue|undefined;instances:FieldOption[];brains?:FieldOption[];onChange:(value:ComponentValue)=>void}){
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
  case "text":return <input type="text" aria-label={label} maxLength={64} value={typeof value==="string"?value:""} placeholder={typeof field.default==="string"?field.default:""} onChange={event=>onChange(event.target.value)}/>;
  case "choice":return <select aria-label={label} value={typeof value==="string"?value:typeof field.default==="string"?field.default:""} onChange={event=>onChange(event.target.value)}>{(field.options??[]).map(option=><option key={option} value={option}>{option.replaceAll("_"," ")}</option>)}</select>;
  case "entities":{
   const list=Array.isArray(value)?value.filter((item):item is string=>typeof item==="string"):[];
   const name=(id:string)=>instances.find(item=>item.id===id)?.name??`${id} (missing)`;
   return <span className="field-list" aria-label={label}>
    {list.map((id,i)=><span className="field-chip" key={`${id}:${i}`}>{i+1}. {name(id)}<button type="button" aria-label={`Remove ${name(id)} from ${label}`} onClick={()=>onChange(list.filter((_,j)=>j!==i))}>×</button></span>)}
    <select aria-label={`Add to ${label}`} value="" onChange={event=>{if(event.target.value)onChange([...list,event.target.value]);}}><option value="">Add…</option>{instances.map(item=><option key={item.id} value={item.id}>{item.name}</option>)}</select>
   </span>;
  }
  case "vector3":{const vector=numbers(value)?.length===3?numbers(value)!:[0,0,0];return <span className="component-vector">{["X","Y","Z"].map((axis,i)=><input key={axis} type="number" aria-label={`${label} ${axis}`} min={field.min} max={field.max} step="any" value={Number.isFinite(vector[i])?vector[i]:""} onChange={event=>onChange(vector.map((n,j)=>j===i?event.target.valueAsNumber:n))}/>)}</span>;}
  default:return <input type="number" aria-label={label} min={field.min} max={field.max} step="any" value={typeof value==="number"&&Number.isFinite(value)?value:""} placeholder={typeof field.default==="number"?String(field.default):""} onChange={event=>onChange(event.target.valueAsNumber)}/>;
 }
}
