"use client";
import Link from "next/link";
import {useCallback,useEffect,useMemo,useRef,useState,type PointerEvent as ReactPointerEvent,type ReactNode} from "react";
import {useEditorNavigationGuard} from "@/components/editor-navigation";
import {AssetCarousel} from "@/components/asset-carousel";
import {AudioGenerator} from "@/components/audio-generator";
import {FalBalance} from "@/components/fal-balance";
import {AUDIO_CATEGORIES,type AudioCategory,type AudioClip,type AudioClipSettings,type AudioClipSummary,type AudioInfo} from "@/lib/audio-types";

type Edit={settings:AudioClipSettings;trim:[number,number];loop:[number,number]|null};
type Handle="trimStart"|"trimEnd"|"loopStart"|"loopEnd";
const CATEGORY_LABEL:Record<AudioCategory,string>={sfx:"Sound effect",music:"Music",voice:"Voice",ui:"UI"};
const round=(n:number)=>Math.round(n*1000)/1000;
const seconds=(n:number)=>`${n.toFixed(2)} s`;
const decibels=(peak:number)=>peak>0?20*Math.log10(peak):-Infinity;

async function request<T>(url:string,init?:RequestInit):Promise<T>{const response=await fetch(url,init),data=await response.json();if(!response.ok)throw new Error(data.error??"Request failed.");return data as T;}

/** File settings (loop relative to the trimmed start) to editor state (absolute source seconds). */
function editFrom(clip:AudioClipSummary,duration:number):Edit{
 const trim:[number,number]=clip.settings.trim?[clip.settings.trim[0],Math.min(clip.settings.trim[1],duration)]:[0,duration];
 const loop=clip.settings.loop?[trim[0]+clip.settings.loop[0],trim[0]+clip.settings.loop[1]] as [number,number]:null;
 return {settings:clip.settings,trim,loop};
}
function settingsFrom(edit:Edit,duration:number):AudioClipSettings{
 const whole=edit.trim[0]<=0.0005&&edit.trim[1]>=duration-0.0005;
 return {...edit.settings,trim:whole?null:[round(edit.trim[0]),round(edit.trim[1])],loop:edit.loop?[round(edit.loop[0]-edit.trim[0]),round(edit.loop[1]-edit.trim[0])]:null};
}

export function AudioEditor({projectId,projectName,scenePath,scenes,initialClips,initialPath}:{projectId:string;projectName:string;scenePath:string;scenes:string[];initialClips:AudioClipSummary[];initialPath?:string}){
 const [clips,setClips]=useState(initialClips),[path,setPath]=useState(initialPath),[tab,setTab]=useState<"import"|"generate">("import");
 const [clip,setClip]=useState<AudioClip|null>(null),[buffer,setBuffer]=useState<AudioBuffer|null>(null),[edit,setEdit]=useState<Edit|null>(null),[saved,setSaved]=useState("");
 const [info,setInfo]=useState<AudioInfo|null>(null),[busy,setBusy]=useState(""),[error,setError]=useState(""),[notice,setNotice]=useState("");
 const [importCategory,setImportCategory]=useState<AudioCategory>("sfx"),[placeScene,setPlaceScene]=useState(scenePath),[position,setPosition]=useState<[number,number,number]>([0,1,0]),[placed,setPlaced]=useState<string|null>(null);
 const duration=buffer?.duration??0,current=edit&&buffer?JSON.stringify(settingsFrom(edit,duration)):"",dirty=Boolean(edit)&&current!==saved;
 useEditorNavigationGuard(useCallback(()=>!dirty||window.confirm("Discard unsaved clip changes?"),[dirty]));

 const refreshList=useCallback(async()=>{const data=await request<{clips:AudioClipSummary[]}>(`/api/audio?projectId=${encodeURIComponent(projectId)}`);setClips(data.clips);},[projectId]);
 // Load the chosen clip, then its audio for the waveform and preview.
 useEffect(()=>{
  if(!path)return;
  let cancelled=false;
  (async()=>{
   const data=await request<AudioClip>(`/api/audio?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}`);if(cancelled)return;setClip(data);
   if(data.error){setBuffer(null);setEdit(null);setError(data.error);return;}
   const response=await fetch(`/api/project-asset?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(data.sourcePath)}`);if(!response.ok)throw new Error("The clip's audio file could not be read.");
   const context=new AudioContext(),decoded=await context.decodeAudioData(await response.arrayBuffer());void context.close();if(cancelled)return;
   const next=editFrom(data,decoded.duration);setBuffer(decoded);setEdit(next);setSaved(JSON.stringify(settingsFrom(next,decoded.duration)));
  })().catch(e=>{if(!cancelled)setError(e instanceof Error?e.message:"The clip could not be opened.");});
  return()=>{cancelled=true;};
 },[path,projectId]);

 /** Every path change goes through here, so the loader effect only ever fetches. */
 function select(next:string|undefined,message=""){stop();setClip(null);setBuffer(null);setEdit(null);setInfo(null);setPlaced(null);setError("");setNotice(message);setPath(next);}
 function choose(next:string){if(next===path||(dirty&&!window.confirm("Discard unsaved clip changes?")))return;select(next);window.history.replaceState(null,"",`/audio?asset=${encodeURIComponent(next)}${scenePath?`&scene=${encodeURIComponent(scenePath)}`:""}`);}
 const change=(patch:Partial<Edit>)=>setEdit(value=>value&&{...value,...patch});
 const set=(patch:Partial<AudioClipSettings>)=>setEdit(value=>value&&{...value,settings:{...value.settings,...patch}});

 async function importFile(file:File){
  setBusy("Importing and decoding…");setError("");setNotice("");
  try{const name=file.name.replace(/\.[^.]+$/,"").replace(/[_-]+/g," ").trim().slice(0,64)||"Clip";
   const created=await request<AudioClip>(`/api/audio?projectId=${encodeURIComponent(projectId)}&name=${encodeURIComponent(name)}&category=${importCategory}`,{method:"POST",body:file});
   await refreshList();select(created.path,`Imported ${created.settings.name}.`);}
  catch(e){setError(e instanceof Error?e.message:"Import failed.");}finally{setBusy("");}
 }
 async function save(){
  if(!clip||!edit||!buffer)return;setBusy("Validating with the engine…");setError("");setNotice("");
  try{const result=await request<AudioClip>("/api/audio",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path:clip.path,revision:clip.revision,settings:settingsFrom(edit,duration)})});
   setClip(result);setInfo(result.info??null);setSaved(current);await refreshList();setNotice(`Saved. The engine plays ${result.info?seconds(result.info.duration_seconds):"it"}.`);}
  catch(e){setError(e instanceof Error?e.message:"Saving failed.");}finally{setBusy("");}
 }
 async function remove(){
  if(!clip||!window.confirm(`Delete ${clip.settings.name} and its audio file?`))return;setBusy("Deleting…");setError("");
  try{await request("/api/audio",{method:"DELETE",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path:clip.path})});select(undefined,"Clip deleted.");await refreshList();}
  catch(e){setError(e instanceof Error?e.message:"Delete failed.");}finally{setBusy("");}
 }
 async function place(){
  if(!clip)return;setBusy("Adding to the scene…");setError("");
  try{const result=await request<{scene:string}>("/api/audio/place",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,clip:clip.path,scene:placeScene,translation:position})});setPlaced(result.scene);await refreshList();}
  catch(e){setError(e instanceof Error?e.message:"Placing failed.");}finally{setBusy("");}
 }

 // Preview through Web Audio, approximating the engine: play from trim start, loop between loop points.
 const player=useRef<{context:AudioContext;source:AudioBufferSourceNode;started:number;from:number}|null>(null),[playing,setPlaying]=useState(false),[loopPreview,setLoopPreview]=useState(true),[playhead,setPlayhead]=useState<number|null>(null);
 function stop(){const current=player.current;player.current=null;if(current){try{current.source.stop();}catch{}void current.context.close();}setPlaying(false);setPlayhead(null);}
 function play(){
  if(!buffer||!edit)return;stop();
  const context=new AudioContext(),source=context.createBufferSource(),gain=context.createGain(),level=10**(edit.settings.gain_db/20),length=edit.trim[1]-edit.trim[0];
  source.buffer=buffer;source.connect(gain).connect(context.destination);
  const looping=loopPreview&&edit.loop!==null;
  gain.gain.setValueAtTime(edit.settings.fade_in>0?0:level,context.currentTime);
  if(edit.settings.fade_in>0)gain.gain.linearRampToValueAtTime(level,context.currentTime+Math.min(edit.settings.fade_in,length));
  if(looping&&edit.loop){source.loop=true;source.loopStart=edit.loop[0];source.loopEnd=edit.loop[1];source.start(0,edit.trim[0]);}
  else{if(edit.settings.fade_out>0){const at=context.currentTime+length;gain.gain.setValueAtTime(level,Math.max(context.currentTime,at-edit.settings.fade_out));gain.gain.linearRampToValueAtTime(0,at);}source.start(0,edit.trim[0],length);}
  source.onended=()=>{if(player.current?.source===source)stop();};
  player.current={context,source,started:context.currentTime,from:edit.trim[0]};setPlaying(true);
 }
 useEffect(()=>{if(!playing)return;let frame=0;const tick=()=>{const p=player.current;if(p&&edit){let at=p.from+(p.context.currentTime-p.started);if(p.source.loop&&edit.loop&&at>=edit.loop[1]){const span=edit.loop[1]-edit.loop[0];at=edit.loop[0]+((at-edit.loop[0])%span);}setPlayhead(at);}frame=requestAnimationFrame(tick);};frame=requestAnimationFrame(tick);return()=>cancelAnimationFrame(frame);},[playing,edit]);
 useEffect(()=>()=>stop(),[]);

 const peak=useMemo(()=>{if(!buffer)return 0;let max=0;for(let c=0;c<buffer.numberOfChannels;c++){const data=buffer.getChannelData(c);for(let i=0;i<data.length;i++)max=Math.max(max,Math.abs(data[i]));}return max;},[buffer]);

 return <main className="object-shell brain-shell audio-shell">
  <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Audio</b></span><Link href={`/?scene=${encodeURIComponent(scenePath)}`} className="quiet-button">Project map ↗</Link><FalBalance/></header>
  <div className="object-heading terrain-library-heading"><div className="terrain-library-title"><span className="eyebrow">SOUND / MUSIC / VOICE</span><h1>Audio</h1><p>Clips for the whole project: import, trim, loop and preview, then play them from objects with the Audio source component.</p><span className="kind-badge">{clips.length} CLIPS</span></div>
   <AssetCarousel label="Audio library" listLabel="Saved audio clips" newLabel="clip" selected={path} disabled={Boolean(busy)} onSelect={choose} onNew={()=>setTab("generate")}
    items={clips.map(item=>({path:item.path,name:item.settings.name||item.path,error:item.error,title:`${item.settings.name} · ${CATEGORY_LABEL[item.settings.category]}${item.generation?" · generated":""}\n${item.usedBy.length?`Used in ${item.usedBy.join(", ")}`:"Not used in a scene yet"}${item.settings.description?`\n${item.settings.description}`:""}`,thumbnail:<ClipThumbnail projectId={projectId} path={item.path} revision={item.revision} category={item.settings.category} generated={Boolean(item.generation)}/>}))}/></div>
  <div className="brain-layout">
   <aside className="panel brain-library audio-library" aria-label="Add clips">
    <div className="panel-head"><div className="view-switch" role="tablist" aria-label="Import or generate">{(["import","generate"] as const).map(item=><button type="button" role="tab" aria-selected={tab===item} key={item} className={tab===item?"selected":""} onClick={()=>setTab(item)}>{item==="import"?"Import":"Generate"}</button>)}</div></div>
    {tab==="generate"?<AudioGenerator projectId={projectId} onSaved={next=>{if(dirty&&!window.confirm("Discard unsaved clip changes?"))return;void refreshList();select(next,"Saved the generated clip.");}}/>:
    <div className="audio-import">
     <p className="brain-hint">Add a WAV, MP3, OGG Vorbis or FLAC file (up to 50 MiB) to the library above. Its category picks the mix bus.</p>
     <div className="audio-import-row"><select aria-label="Category for imported clips" value={importCategory} onChange={event=>setImportCategory(event.target.value as AudioCategory)}>{AUDIO_CATEGORIES.map(category=><option key={category} value={category}>{CATEGORY_LABEL[category]}</option>)}</select>
      <label className={`quiet-button audio-import-button${busy?" disabled":""}`}>Import file…<input type="file" accept=".wav,.mp3,.ogg,.flac,audio/wav,audio/mpeg,audio/ogg,audio/flac" disabled={Boolean(busy)} onChange={event=>{const file=event.target.files?.[0];event.target.value="";if(file)void importFile(file);}}/></label></div>
     {!clips.length&&<p className="brain-hint">No clips yet. Import one, or make one in Generate.</p>}
    </div>}
   </aside>
   <section className="panel brain-workspace audio-workspace">
    <div className="panel-head"><div className="brain-title"><b>{clip?.settings.name??"No clip selected"}</b>{dirty&&<span className="kind-badge">UNSAVED</span>}</div>
     {buffer&&<div className="view-switch"><button type="button" className={playing?"selected":""} onClick={()=>playing?stop():play()}>{playing?"■ Stop":"▶ Play"}</button><label className="audio-toggle"><input type="checkbox" checked={loopPreview} disabled={!edit?.loop} onChange={event=>setLoopPreview(event.target.checked)}/>Loop</label></div>}</div>
    {error&&<p className="brain-error" role="alert">{error}</p>}
    {(busy||notice)&&<p className="brain-status" role="status">{busy||notice}</p>}
    {buffer&&edit?<>
     <Waveform buffer={buffer} edit={edit} playhead={playhead} onChange={change}/>
     <p className="transform-note audio-facts">{seconds(duration)} · {buffer.numberOfChannels===1?"mono":"stereo"} · {buffer.sampleRate} Hz · peak {Number.isFinite(decibels(peak))?`${decibels(peak).toFixed(1)} dBFS`:"silent"}{info?` · engine plays ${seconds(info.duration_seconds)}`:""}</p>
     <p className="transform-note">Drag the white handles to trim and the teal handles to set the loop. A looping source plays from the trim start, then repeats between the loop points.</p>
    </>:path&&!error?<p className="transform-note audio-empty">Loading audio…</p>:!path&&<p className="transform-note audio-empty">Choose a clip, or import one.</p>}
    {clip&&<textarea className="brain-description" aria-label="Clip description" maxLength={500} placeholder="What this sounds like (shown in the library and to the AI)…" value={edit?.settings.description??clip.settings.description} disabled={!edit} onChange={event=>set({description:event.target.value})}/>}
   </section>
   <aside className="panel brain-inspector" aria-label="Clip settings">
    <div className="panel-head"><h2>Clip</h2>{clip&&<span className="kind-badge">{CATEGORY_LABEL[edit?.settings.category??clip.settings.category]}</span>}</div>
    {edit&&buffer?<div className="brain-node-form">
     <label className="component-field"><span>Name</span><input type="text" maxLength={64} value={edit.settings.name} onChange={event=>set({name:event.target.value})}/><span/></label>
     <label className="component-field"><span>Category</span><select value={edit.settings.category} onChange={event=>set({category:event.target.value as AudioCategory})}>{AUDIO_CATEGORIES.map(category=><option key={category} value={category}>{CATEGORY_LABEL[category]}</option>)}</select><span/></label>
     <label className="component-field" title="Applied when the clip loads."><span>Gain (dB)</span><input type="number" step={0.5} min={-60} max={24} value={edit.settings.gain_db} onChange={event=>set({gain_db:Number(event.target.value)})}/><button type="button" className="component-reset" title="Normalise: peak at -1 dBFS" aria-label="Normalise gain" disabled={!peak} onClick={()=>set({gain_db:Math.round((-1-decibels(peak))*10)/10})}>N</button></label>
     <NumberField label="Trim start (s)" value={edit.trim[0]} min={0} max={edit.trim[1]-0.01} onChange={n=>change({trim:[n,edit.trim[1]]})}/>
     <NumberField label="Trim end (s)" value={edit.trim[1]} min={edit.trim[0]+0.01} max={duration} onChange={n=>change({trim:[edit.trim[0],n]})}/>
     <NumberField label="Fade in (s)" value={edit.settings.fade_in} min={0} max={60} onChange={n=>set({fade_in:n})}/>
     <NumberField label="Fade out (s)" value={edit.settings.fade_out} min={0} max={60} onChange={n=>set({fade_out:n})}/>
     <label className="component-field" title="Streaming reads long tracks from disk as they play instead of holding them in memory."><span>Playback</span><select value={edit.settings.stream===null?"auto":edit.settings.stream?"stream":"load"} onChange={event=>set({stream:event.target.value==="auto"?null:event.target.value==="stream"})}><option value="auto">Auto ({edit.settings.category==="music"?"streams":"loads whole"})</option><option value="stream">Stream from disk</option><option value="load">Load whole</option></select><span/></label>
     <label className="component-field"><span>Loop points</span><input type="checkbox" checked={edit.loop!==null} onChange={event=>change({loop:event.target.checked?[edit.trim[0],edit.trim[1]]:null})}/><span/></label>
     {edit.loop&&<><NumberField label="Loop start (s)" value={edit.loop[0]} min={edit.trim[0]} max={edit.loop[1]-0.01} onChange={n=>change({loop:[n,edit.loop![1]]})}/>
      <NumberField label="Loop end (s)" value={edit.loop[1]} min={edit.loop[0]+0.01} max={edit.trim[1]} onChange={n=>change({loop:[edit.loop![0],n]})}/></>}
     <section className="brain-apply" aria-label="Save clip">
      <button type="button" className="settings-save" disabled={Boolean(busy)||!dirty||!edit.settings.name.trim()} onClick={()=>void save()}>{busy?"Working…":"Save clip"}</button>
      <p className="transform-note">Saving changes the clip&apos;s settings only; the audio file is never rewritten.</p>
     </section>
     <section className="brain-apply" aria-label="Add to scene">
      <h3>Add to scene</h3>
      <select aria-label="Scene" value={placeScene} onChange={event=>setPlaceScene(event.target.value)}>{scenes.map(scene=><option key={scene} value={scene}>{scene.replace(/^assets\/scenes\//,"").replace(/\.scene\.toml$/,"")}</option>)}</select>
      <div className="audio-position">{position.map((value,axis)=><input key={axis} type="number" step={0.5} aria-label={`Position ${"XYZ"[axis]}`} value={value} onChange={event=>setPosition(current=>current.map((n,i)=>i===axis?Number(event.target.value):n) as [number,number,number])}/>)}</div>
      <button type="button" className="quiet-button" disabled={Boolean(busy)||dirty||!placeScene} onClick={()=>void place()}>Add sound point</button>
      {placed&&<Link className="quiet-button" href={`/?scene=${encodeURIComponent(placed)}`}>Open scene ↗</Link>}
      <p className="transform-note">Adds a Point playing this clip. Change its range and Doppler in the scene inspector.</p>
     </section>
     <section className="brain-apply" aria-label="Delete clip">
      {clip&&clip.usedBy.length>0&&<p className="transform-note">Used by {clip.usedBy.join(", ")}.</p>}
      <button type="button" className="quiet-button" disabled={Boolean(busy)||Boolean(clip?.usedBy.length)} onClick={()=>void remove()}>Delete clip</button>
     </section>
    </div>:<p className="transform-note audio-empty">{clip?.error?"Fix this clip\u2019s file in the source editor.":"No clip selected."}</p>}
   </aside>
  </div>
 </main>;
}

const CATEGORY_ICONS:Record<AudioCategory,ReactNode>={
 sfx:<path d="M100 22 106 46 128 34 116 56 140 62 116 68 128 90 106 78 100 102 94 78 72 90 84 68 60 62 84 56 72 34 94 46Z"/>,
 music:<><path d="M88 86V36l40-8v50" fill="none"/><ellipse cx="80" cy="86" rx="10" ry="8"/><ellipse cx="120" cy="78" rx="10" ry="8"/></>,
 voice:<><path d="M70 36h60a8 8 0 0 1 8 8v28a8 8 0 0 1-8 8H98l-16 14V80H70a8 8 0 0 1-8-8V44a8 8 0 0 1 8-8Z" fill="none"/><path d="M80 52h40M80 64h28" fill="none"/></>,
 ui:<><rect x="68" y="44" width="64" height="32" rx="8" fill="none"/><path d="M104 60l14 30 5-11 11-5Z"/></>,
};
// Outlines already fetched this session, by clip path and revision.
const waveforms=new Map<string,number[]>();

/** Carousel swatch: the clip's waveform as the engine plays it (category icon until it loads), with a spark on generated clips. */
function ClipThumbnail({projectId,path,revision,category,generated}:{projectId:string;path:string;revision:string;category:AudioCategory;generated:boolean}){
 const key=`${path}@${revision}`,[waveform,setWaveform]=useState<number[]|undefined>(()=>waveforms.get(key));
 useEffect(()=>{
  if(waveforms.has(key))return;let active=true;
  void fetch(`/api/audio?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}&waveform=1`).then(response=>response.ok?response.json():{}).then((data:{waveform?:unknown})=>{
   if(!Array.isArray(data.waveform)||!data.waveform.length)return;const points=data.waveform.map(Number).filter(Number.isFinite);waveforms.set(key,points);if(active)setWaveform(points);
  }).catch(()=>{});
  return()=>{active=false;};
 },[key,path,projectId]);
 const shown=waveform??waveforms.get(key);
 // Scaled to the clip's own peak so quiet clips still show their shape.
 const loudest=shown?Math.max(...shown,1e-3):1,x=(i:number)=>8+i*184/Math.max(1,(shown?.length??1)-1),y=(level:number)=>Math.max(1.5,level/loudest*46);
 const outline=shown&&[...shown.map((level,i)=>`${x(i)},${60-y(level)}`),...shown.map((level,i)=>`${x(i)},${60+y(level)}`).reverse()].join(" ");
 return <svg className={`brain-thumbnail audio-thumbnail ${category}`} viewBox="0 0 200 120" aria-hidden="true">
  {outline?<>
   <polygon className="audio-thumbnail-wave" points={outline}/>
   <g transform="translate(6 4) scale(.2)" stroke="currentColor" strokeWidth="9" strokeLinecap="round" strokeLinejoin="round" fill="currentColor">{CATEGORY_ICONS[category]}</g>
  </>:<g stroke="currentColor" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" fill="currentColor">{CATEGORY_ICONS[category]}</g>}
  {generated&&<path className="audio-thumbnail-spark" d="M172 14l3 8 8 3-8 3-3 8-3-8-8-3 8-3Z"/>}
 </svg>;
}

function NumberField({label,value,min,max,onChange}:{label:string;value:number;min:number;max:number;onChange:(value:number)=>void}){
 return <label className="component-field"><span>{label}</span><input type="number" step={0.01} min={round(min)} max={round(max)} value={round(value)} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n))onChange(Math.min(Math.max(n,min),max));}}/><span/></label>;
}

const WIDTH=960,HEIGHT=200;
function Waveform({buffer,edit,playhead,onChange}:{buffer:AudioBuffer;edit:Edit;playhead:number|null;onChange:(patch:Partial<Edit>)=>void}){
 const canvas=useRef<HTMLCanvasElement>(null),[drag,setDrag]=useState<Handle|null>(null),duration=buffer.duration;
 const x=(time:number)=>time/duration*WIDTH;
 useEffect(()=>{
  const element=canvas.current,context=element?.getContext("2d");if(!element||!context)return;
  const style=getComputedStyle(element),wave=style.getPropertyValue("--audio-wave").trim()||"#8fd3c4";
  context.clearRect(0,0,WIDTH,HEIGHT);context.fillStyle=wave;
  const channels=Array.from({length:buffer.numberOfChannels},(_,c)=>buffer.getChannelData(c)),step=Math.max(1,Math.floor(buffer.length/WIDTH));
  for(let column=0;column<WIDTH;column++){let low=1,high=-1;const start=Math.floor(column/WIDTH*buffer.length);for(let i=start;i<Math.min(start+step,buffer.length);i++){let sum=0;for(const data of channels)sum+=data[i];const s=sum/channels.length;low=Math.min(low,s);high=Math.max(high,s);}if(high<low)continue;context.fillRect(column,(1-high)*HEIGHT/2,1,Math.max(1,(high-low)*HEIGHT/2));}
 },[buffer]);
 function pointer(event:ReactPointerEvent<SVGSVGElement>){const box=event.currentTarget.getBoundingClientRect();return Math.min(Math.max((event.clientX-box.left)/box.width*duration,0),duration);}
 /** Moves one handle; the loop always stays inside the trim. Shared by pointer and keyboard. */
 function apply(name:Handle,t:number){
  const gap=0.01,[start,stop]=edit.trim;
  if(name==="trimStart"){const next=Math.min(Math.max(t,0),stop-gap);onChange({trim:[next,stop],loop:edit.loop&&[Math.min(Math.max(edit.loop[0],next),edit.loop[1]-gap),edit.loop[1]]});}
  if(name==="trimEnd"){const next=Math.max(Math.min(t,duration),start+gap);onChange({trim:[start,next],loop:edit.loop&&[edit.loop[0],Math.max(Math.min(edit.loop[1],next),edit.loop[0]+gap)]});}
  if(name==="loopStart"&&edit.loop)onChange({loop:[Math.min(Math.max(t,start),edit.loop[1]-gap),edit.loop[1]]});
  if(name==="loopEnd"&&edit.loop)onChange({loop:[edit.loop[0],Math.max(Math.min(t,stop),edit.loop[0]+gap)]});
 }
 function move(event:ReactPointerEvent<SVGSVGElement>){if(drag)apply(drag,pointer(event));}
 const handle=(name:Handle,time:number,className:string,label:string)=><g key={name} className={`audio-handle ${className}`} role="slider" aria-label={label} aria-valuemin={0} aria-valuemax={round(duration)} aria-valuenow={round(time)} tabIndex={0}
  onPointerDown={event=>{event.stopPropagation();(event.currentTarget.ownerSVGElement as SVGSVGElement).setPointerCapture(event.pointerId);setDrag(name);}}
  onKeyDown={event=>{const delta=event.key==="ArrowLeft"?-0.01:event.key==="ArrowRight"?0.01:0;if(!delta)return;event.preventDefault();apply(name,time+delta*(event.shiftKey?10:1));}}>
  <line x1={x(time)} x2={x(time)} y1={0} y2={HEIGHT}/><rect x={x(time)-6} y={className==="loop"?HEIGHT-18:0} width={12} height={18} rx={3}/></g>;
 return <div className="audio-wave">
  <canvas ref={canvas} width={WIDTH} height={HEIGHT} aria-hidden="true"/>
  <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none" onPointerMove={move} onPointerUp={()=>setDrag(null)} onPointerCancel={()=>setDrag(null)} aria-label="Waveform with trim and loop handles">
   <rect className="audio-cut" x={0} y={0} width={x(edit.trim[0])} height={HEIGHT}/><rect className="audio-cut" x={x(edit.trim[1])} y={0} width={WIDTH-x(edit.trim[1])} height={HEIGHT}/>
   {edit.loop&&<rect className="audio-loop" x={x(edit.loop[0])} y={0} width={x(edit.loop[1])-x(edit.loop[0])} height={HEIGHT}/>}
   {playhead!==null&&<line className="audio-playhead" x1={x(playhead)} x2={x(playhead)} y1={0} y2={HEIGHT}/>}
   {handle("trimStart",edit.trim[0],"trim","Trim start")}{handle("trimEnd",edit.trim[1],"trim","Trim end")}
   {edit.loop&&<>{handle("loopStart",edit.loop[0],"loop","Loop start")}{handle("loopEnd",edit.loop[1],"loop","Loop end")}</>}
  </svg>
 </div>;
}
