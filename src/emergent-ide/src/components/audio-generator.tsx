"use client";
import {useEffect,useState} from "react";
import {FalModelSelector} from "@/components/fal-model-selector";
import {AUDIO_MODE_DURATION,type AudioJob,type AudioMode} from "@/lib/audio-types";
import {DEFAULT_FAL_MODELS} from "@/lib/fal-model-types";

const MODES:{id:AudioMode;label:string;placeholder:string}[]=[
 {id:"sound",label:"Sound effect",placeholder:"Heavy wooden door creaking open, then slamming shut"},
 {id:"music",label:"Music",placeholder:"Tense orchestral loop for a stealth level, low strings and soft percussion"},
 {id:"speech",label:"Speech",placeholder:"Halt! Who goes there?"},
];
// ElevenLabs preset voices; other models may use their own names.
const VOICES=["Rachel","Aria","Roger","Sarah","Laura","Charlie","George","Callum","River","Liam","Charlotte","Alice","Matilda","Will","Jessica","Eric","Chris","Brian","Daniel","Lily","Bill"];
const PENDING=new Set(["submitting","queued","generating"]);
const STATE_LABEL:Record<AudioJob["state"],string>={submitting:"Submitting…",uncertain:"Unconfirmed",queued:"Queued…",generating:"Generating…",ready:"Ready",failed:"Failed"};

async function post<T>(url:string,body:unknown):Promise<T>{const response=await fetch(url,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify(body)}),data=await response.json();if(!response.ok)throw new Error(data.error??"Request failed.");return data as T;}
const defaultName=(job:AudioJob)=>job.text.replace(/[^\w ,'-]+/g," ").split(/\s+/).filter(Boolean).slice(0,5).join(" ").slice(0,64)||"Generated clip";

/** fal generation for the Audio editor: describe, generate a few candidates, listen, keep one as a clip. */
export function AudioGenerator({projectId,onSaved}:{projectId:string;onSaved:(path:string)=>void}){
 const [mode,setMode]=useState<AudioMode>("sound"),[text,setText]=useState(""),[duration,setDuration]=useState<number>(AUDIO_MODE_DURATION.sound.default),[voice,setVoice]=useState("Rachel");
 const [loop,setLoop]=useState(false),[instrumental,setInstrumental]=useState(true),[count,setCount]=useState(2),[models,setModels]=useState<Record<AudioMode,{id:string;ready:boolean}>>({sound:{id:DEFAULT_FAL_MODELS.sound.id,ready:true},music:{id:DEFAULT_FAL_MODELS.music.id,ready:true},speech:{id:DEFAULT_FAL_MODELS.speech.id,ready:true}});
 const [jobs,setJobs]=useState<AudioJob[]>([]),[names,setNames]=useState<Record<string,string>>({}),[busy,setBusy]=useState(false),[error,setError]=useState("");
 const model=models[mode],[modelVoices,setModelVoices]=useState<Record<string,string[]>>({});
 // Voice suggestions: ElevenLabs presets for the default, otherwise whatever the model's schema lists.
 const voices=model.id===DEFAULT_FAL_MODELS.speech.id?VOICES:modelVoices[model.id]??[];
 useEffect(()=>{
  if(mode!=="speech"||!model.ready||model.id===DEFAULT_FAL_MODELS.speech.id||modelVoices[model.id])return;let active=true;
  void fetch(`/api/fal-models?projectId=${encodeURIComponent(projectId)}&kind=speech&model=${encodeURIComponent(model.id)}`).then(response=>response.ok?response.json():{}).then((data:{voices?:unknown})=>{if(active)setModelVoices(current=>({...current,[model.id]:Array.isArray(data.voices)?data.voices.filter((v):v is string=>typeof v==="string"):[]}));}).catch(()=>{});
  return()=>{active=false;};
 },[mode,model,modelVoices,projectId]);
 useEffect(()=>{let active=true;void fetch(`/api/audio-jobs?projectId=${encodeURIComponent(projectId)}`).then(response=>response.ok?response.json():{jobs:[]}).then(data=>{if(active)setJobs(data.jobs);}).catch(()=>{});return()=>{active=false;};},[projectId]);
 // Poll pending jobs; refresh also downloads a finished result.
 useEffect(()=>{
  const pending=jobs.filter(job=>PENDING.has(job.state));if(!pending.length)return;
  const timer=setTimeout(()=>{void Promise.all(pending.map(job=>post<AudioJob>(`/api/audio-jobs/${job.id}`,{projectId,action:"refresh"}).catch(()=>job))).then(updated=>setJobs(current=>current.map(job=>updated.find(item=>item.id===job.id)??job)));},3000);
  return()=>clearTimeout(timer);
 },[jobs,projectId]);

 function switchMode(next:AudioMode){setMode(next);if(next!=="speech")setDuration(AUDIO_MODE_DURATION[next].default);}
 async function generate(){
  setBusy(true);setError("");
  try{
   const base=Math.floor(Math.random()*2147483000),started:AudioJob[]=[];
   for(let i=0;i<count;i++){
    const request={mode,text,seed:base+i,...(mode!=="speech"?{duration}:{voice}),...(mode==="sound"&&loop?{loop:true}:{}),...(mode==="music"&&instrumental?{instrumental:true}:{}),...(model.id!==DEFAULT_FAL_MODELS[mode].id?{model:model.id}:{})};
    started.push(await post<AudioJob>("/api/audio-jobs",{projectId,id:crypto.randomUUID(),request}));
   }
   setJobs(current=>[...started,...current]);
  }catch(e){setError(e instanceof Error?e.message:"Generation failed.");}finally{setBusy(false);}
 }
 async function keep(job:AudioJob){
  setError("");
  try{const result=await post<{job:AudioJob;clip:{path:string}}>(`/api/audio-jobs/${job.id}`,{projectId,action:"keep",name:names[job.id]??defaultName(job)});setJobs(current=>current.map(item=>item.id===job.id?result.job:item));onSaved(result.clip.path);}
  catch(e){setError(e instanceof Error?e.message:"Saving failed.");}
 }
 async function discard(job:AudioJob){
  setError("");
  try{await post(`/api/audio-jobs/${job.id}`,{projectId,action:"delete"});setJobs(current=>current.filter(item=>item.id!==job.id));}
  catch(e){setError(e instanceof Error?e.message:"Delete failed.");}
 }
 const bounds=mode!=="speech"?AUDIO_MODE_DURATION[mode]:null;

 return <div className="audio-generator">
  <div className="view-switch audio-modes" role="tablist" aria-label="What to generate">{MODES.map(item=><button type="button" role="tab" aria-selected={mode===item.id} key={item.id} className={mode===item.id?"selected":""} onClick={()=>switchMode(item.id)}>{item.label}</button>)}</div>
  <textarea aria-label={mode==="speech"?"Line to speak":"Describe the sound"} maxLength={2000} rows={4} placeholder={MODES.find(item=>item.id===mode)!.placeholder} value={text} onChange={event=>setText(event.target.value)}/>
  {bounds&&<label className="component-field"><span>Length (s)</span><input type="number" min={bounds.min} max={bounds.max} step={mode==="sound"?0.5:5} value={duration} onChange={event=>setDuration(Math.min(Math.max(Number(event.target.value),bounds.min),bounds.max))}/><span/></label>}
  {mode==="sound"&&<label className="component-field" title="Ask the model for a seamless loop (ElevenLabs)."><span>Seamless loop</span><input type="checkbox" checked={loop} onChange={event=>setLoop(event.target.checked)}/><span/></label>}
  {mode==="music"&&<label className="component-field"><span>Instrumental</span><input type="checkbox" checked={instrumental} onChange={event=>setInstrumental(event.target.checked)}/><span/></label>}
  {mode==="speech"&&<label className="component-field"><span>Voice</span><input type="text" list="audio-voices" maxLength={64} value={voice} onChange={event=>setVoice(event.target.value)}/><span/><datalist id="audio-voices">{voices.map(name=><option key={name} value={name}/>)}</datalist></label>}
  <label className="component-field"><span>Candidates</span><select value={count} onChange={event=>setCount(Number(event.target.value))}>{[1,2,3].map(n=><option key={n} value={n}>{n}</option>)}</select><span/></label>
  <FalModelSelector key={mode} projectId={projectId} kind={mode} value={model.id} onChange={(id,ready)=>setModels(current=>({...current,[mode]:{id,ready}}))} disabled={busy}/>
  <button type="button" className="settings-save" disabled={busy||!model.ready||text.trim().length<3||(mode==="speech"&&!voice.trim())} onClick={()=>void generate()}>{busy?"Submitting…":`Generate ${count>1?`${count} candidates`:"1 candidate"}`}</button>
  <p className="transform-note">Each candidate is one paid fal request.</p>
  {error&&<p className="brain-error" role="alert">{error}</p>}
  <div className="audio-jobs" aria-label="Generated candidates">
   {jobs.map(job=><article key={job.id} className={`audio-job ${job.state}`}>
    <header><span className="kind-badge">{MODES.find(item=>item.id===job.mode)?.label.toUpperCase()}</span><small>{STATE_LABEL[job.state]}</small></header>
    <p title={job.text}>{job.text}</p>
    {job.error&&<p className="brain-error">{job.error}</p>}
    {job.state==="ready"&&<audio controls preload="none" src={`/api/audio-jobs/${job.id}/artifact?projectId=${encodeURIComponent(projectId)}`}/>}
    {job.state==="ready"&&!job.savedPath&&<div className="audio-keep"><input type="text" aria-label="Clip name" maxLength={64} value={names[job.id]??defaultName(job)} onChange={event=>setNames(current=>({...current,[job.id]:event.target.value}))}/><button type="button" className="quiet-button" onClick={()=>void keep(job)}>Keep</button></div>}
    {job.savedPath&&<button type="button" className="quiet-button" onClick={()=>onSaved(job.savedPath!)}>Saved — open clip</button>}
    {!PENDING.has(job.state)&&<button type="button" className="component-reset audio-discard" aria-label="Delete candidate" title="Delete candidate" onClick={()=>void discard(job)}>×</button>}
   </article>)}
  </div>
 </div>;
}
