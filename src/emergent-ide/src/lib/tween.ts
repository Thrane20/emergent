import {parse} from "smol-toml";

export const EASINGS = [
  {id:"linear",name:"Linear / Lerp"},
  {id:"sine-in",name:"Sine In"},{id:"sine-out",name:"Sine Out"},{id:"sine-in-out",name:"Sine In-Out"},
  {id:"quad-in",name:"Quadratic In"},{id:"quad-out",name:"Quadratic Out"},{id:"quad-in-out",name:"Quadratic In-Out"},
  {id:"cubic-in",name:"Cubic In"},{id:"cubic-out",name:"Cubic Out"},{id:"cubic-in-out",name:"Cubic In-Out"},
] as const;
export type Easing = typeof EASINGS[number]["id"];
export type TweenSettings = {easing:Easing;duration_seconds:number;from:number;to:number;playback:"once"|"loop"|"ping-pong"};
export const DEFAULT_TWEEN:TweenSettings = {easing:"sine-in-out",duration_seconds:2,from:-3,to:3,playback:"ping-pong"};
export type TweenAsset = {path:string;id:string;name:string;settings:TweenSettings};
export function tweenSettings(value:unknown):TweenSettings {
  if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Tween settings are required.");
  const s=value as TweenSettings;
  if(Object.keys(s).some(key=>!Object.hasOwn(DEFAULT_TWEEN,key))||!EASINGS.some(e=>e.id===s.easing)||!["once","loop","ping-pong"].includes(s.playback))throw new Error("Choose a supported easing and playback mode.");
  if(typeof s.duration_seconds!=="number"||!Number.isFinite(s.duration_seconds)||s.duration_seconds<0.001||s.duration_seconds>3600)throw new Error("Duration must be 0.001–3600 seconds.");
  if(![s.from,s.to].every(v=>typeof v==="number"&&Number.isFinite(v)&&Math.abs(v)<=1000000))throw new Error("Start and end must be finite values within ±1000000.");
  return {...s};
}
export function ease(easing:Easing,progress:number):number {
  const t=Math.max(0,Math.min(1,progress));if(t===0||t===1)return t;
  switch(easing){
    case "linear":return t;
    case "sine-in":return 1-Math.cos(t*Math.PI/2);
    case "sine-out":return Math.sin(t*Math.PI/2);
    case "sine-in-out":return (1-Math.cos(t*Math.PI))/2;
    case "quad-in":return t*t;
    case "quad-out":return 1-(1-t)**2;
    case "quad-in-out":return t<0.5?2*t*t:1-(-2*t+2)**2/2;
    case "cubic-in":return t*t*t;
    case "cubic-out":return 1-(1-t)**3;
    case "cubic-in-out":return t<0.5?4*t*t*t:1-(-2*t+2)**3/2;
  }
}
export function tweenProgress(settings:TweenSettings,elapsed:number):number {
  const time=Math.max(0,elapsed),duration=settings.duration_seconds;
  if(settings.playback==="once")return Math.min(1,time/duration);
  if(settings.playback==="loop")return (time%duration)/duration;
  const phase=(time%(duration*2))/duration;return phase<=1?phase:2-phase;
}
export function sampleTween(settings:TweenSettings,elapsed:number):number {
  const t=ease(settings.easing,tweenProgress(settings,elapsed));return settings.from*(1-t)+settings.to*t;
}
export function tweenAsset(path:string,source:string):TweenAsset {
  const definition=parse(source);
  if(Object.keys(definition).some(key=>!["schema_version","kind","id","name","tween"].includes(key))||definition.schema_version!==1||definition.kind!=="tween"||typeof definition.id!=="string"||!/^[a-z0-9_.-]{1,128}$/i.test(definition.id)||typeof definition.name!=="string"||!definition.name.trim()||Array.from(definition.name).length>64||/[\p{Cc}\p{Cf}]/u.test(definition.name))throw new Error("Invalid schema-1 tween asset.");
  return {path,id:definition.id,name:definition.name,settings:tweenSettings(definition.tween)};
}
