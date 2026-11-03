export type Classification={layer:number;tags:number};
export type ObjectLabels={schema_version:1;layers:string[];tags:string[]};
export const defaultLabels=():ObjectLabels=>({schema_version:1,layers:["Default"],tags:[]});
export function objectLabels(value:unknown):ObjectLabels{
 const c=value as ObjectLabels;
 if(!c||c.schema_version!==1||Object.keys(c).some(k=>!["schema_version","layers","tags"].includes(k)))throw Error("Invalid label configuration.");
 for(const list of [c.layers,c.tags]){if(!Array.isArray(list)||list.length>32)throw Error("Use at most 32 slots.");const names=new Set<string>();for(const n of list){if(typeof n!=="string"||new TextEncoder().encode(n).length>64||n.trim()!==n||/[\p{Cc}]/u.test(n)||n&&names.has(n.toLowerCase()))throw Error("Names must be unique, trimmed and at most 64 bytes.");if(n)names.add(n.toLowerCase());}}
 if(!c.layers[0])throw Error("Layer 0 needs a name.");return c;
}
export function classification(value:unknown):Classification{
 if(value!=null&&(typeof value!=="object"||Array.isArray(value)))throw Error("Expected object classification settings.");
 const c={layer:0,tags:0,...(value??{}) as object} as Classification;
 if(!c||Object.keys(c).some(k=>!["layer","tags"].includes(k))||!Number.isInteger(c.layer)||c.layer<0||c.layer>31||!Number.isInteger(c.tags)||c.tags<0||c.tags>0xffffffff)throw Error("Layer must be 0–31 and tags an unsigned 32-bit mask.");return c;
}
export const toggleTag=(mask:number,index:number)=>{classification({layer:0,tags:mask});if(!Number.isInteger(index)||index<0||index>31)throw Error("Tag index must be 0–31.");return (mask^(1<<index))>>>0;};
