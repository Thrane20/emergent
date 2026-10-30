"use client";
import {FalBalance} from "@/components/fal-balance";
import Link from "next/link";
import {useEditorNavigationGuard} from "./editor-navigation";
import {useEffect,useRef,useState} from "react";
import {DEFAULT_TWEEN,EASINGS,ease,sampleTween,tweenProgress,tweenSettings,type Easing,type TweenAsset,type TweenSettings} from "@/lib/tween";

function curve(easing:Easing,width=100,height=50){return Array.from({length:81},(_,i)=>`${i/80*width},${height-ease(easing,i/80)*height}`).join(" ");}

export function TweenEditor({projectId,projectName,assets,warnings,initialAsset,scenePath}:{projectId:string;projectName:string;assets:TweenAsset[];warnings:string[];initialAsset?:string;scenePath:string}){
  const initial=assets.find(asset=>asset.path===initialAsset);
  const [draft,setDraft]=useState<TweenSettings>(initial?.settings??DEFAULT_TWEEN),[name,setName]=useState(initial?.name??"Smooth movement"),[library,setLibrary]=useState(assets);
  const [playing,setPlaying]=useState(true),[saving,setSaving]=useState(false),[error,setError]=useState(""),[saved,setSaved]=useState<TweenAsset|null>(null);
  const [seek,setSeek]=useState(0),[dirty,setDirty]=useState(false);
  useEditorNavigationGuard(()=>!saving&&(!dirty||window.confirm("Discard unsaved tween changes?")));
  const elapsed=useRef(0),object=useRef<HTMLDivElement>(null),readout=useRef<HTMLOutputElement>(null),cursor=useRef<SVGCircleElement>(null),scrubber=useRef<HTMLInputElement>(null);
  let settings:TweenSettings|null=null,validation="";
  try{settings=tweenSettings(draft);}catch(error){validation=error instanceof Error?error.message:"Invalid tween.";}
  const valid=settings!==null;
  useEffect(()=>{
    if(!valid)return;
    const current=tweenSettings(draft);let previous=performance.now(),animation=0;
    const draw=(now:number)=>{
      if(playing)elapsed.current+=(now-previous)/1000;previous=now;
      const progress=tweenProgress(current,elapsed.current),eased=ease(current.easing,progress),value=sampleTween(current,elapsed.current);
      const fraction=current.from===current.to?0.5:(value-Math.min(current.from,current.to))/Math.abs(current.to-current.from);
      if(object.current){object.current.style.left=`${6+88*fraction}%`;object.current.dataset.value=value.toFixed(4);}
      if(cursor.current){cursor.current.setAttribute("cx",String(40+progress*440));cursor.current.setAttribute("cy",String(230-eased*200));}
      if(readout.current)readout.current.textContent=`${value.toFixed(3)} · ${Math.min(elapsed.current,current.playback==="once"?current.duration_seconds:Infinity).toFixed(2)} s`;
      if(scrubber.current)scrubber.current.value=String(progress);
      if(playing&&current.playback==="once"&&elapsed.current>=current.duration_seconds){setPlaying(false);return;}
      if(playing)animation=requestAnimationFrame(draw);
    };
    animation=requestAnimationFrame(draw);return()=>cancelAnimationFrame(animation);
  },[draft,playing,valid,seek]);
  function change(value:TweenSettings){setDirty(true);elapsed.current=0;setDraft(value);setSaved(null);setError("");}
  function load(asset:TweenAsset){change({...asset.settings});setName(asset.name);setDirty(false);setPlaying(true);}
  async function save(){
    if(!settings||saving)return;const selected={...settings},selectedName=name;setSaving(true);setError("");
    try{
      const response=await fetch("/api/tweens",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,name:selectedName,settings:selected})});
      const result=await response.json();if(!response.ok)throw new Error(result.error??"Tween could not be saved.");
      setLibrary(previous=>[...previous,result]);setSaved(result);setDirty(false);
    }catch(error){setError(error instanceof Error?error.message:"Tween could not be saved.");}finally{setSaving(false);}
  }
  const label=EASINGS.find(item=>item.id===draft.easing)?.name??"Tween";
  return <main className="object-shell tween-shell">
    <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Tween Editor</b></span><Link href={`/terrain?scene=${encodeURIComponent(scenePath)}`} className="quiet-button">Terrain Builder</Link><Link href="/textures" className="quiet-button">Texture Creator</Link><Link href={`/?scene=${encodeURIComponent(scenePath)}`} className="quiet-button">Project map ↗</Link><FalBalance/></header>
    <div className="object-heading"><div><span className="eyebrow">MOTION / TIMING / FEEL</span><h1>Give movement its character.</h1><p>Compare curves. Watch the motion. Keep the variant that feels right.</p></div><span className="kind-badge">{EASINGS.length} EASING VARIANTS</span></div>
    <div className="tween-layout">
      <aside className="panel tween-controls"><div className="panel-head"><h2>Motion recipe</h2><span className="kind-badge">SCALAR TWEEN</span></div>
        <form onSubmit={event=>{event.preventDefault();void save();}}><fieldset disabled={saving}>
          <label className="settings-field"><span>Name</span><input aria-label="Tween name" value={name} maxLength={64} onChange={event=>{setName(event.target.value);setSaved(null);setDirty(true);}} required/></label>
          <label className="settings-field"><span>Duration per direction (seconds)</span><input aria-label="Tween duration" type="number" min="0.001" max="3600" step="0.001" value={Number.isFinite(draft.duration_seconds)?draft.duration_seconds:""} onChange={event=>change({...draft,duration_seconds:event.target.valueAsNumber})} required/></label>
          <div className="tween-endpoints">{(["from","to"] as const).map(key=><label className="settings-field" key={key}><span>{key==="from"?"Start value":"End value"}</span><input aria-label={key==="from"?"Start value":"End value"} type="number" min="-1000000" max="1000000" step="any" value={Number.isFinite(draft[key])?draft[key]:""} onChange={event=>change({...draft,[key]:event.target.valueAsNumber})} required/></label>)}</div>
          <label className="settings-field"><span>Playback</span><select aria-label="Tween playback" value={draft.playback} onChange={event=>change({...draft,playback:event.target.value as TweenSettings["playback"]})}><option value="ping-pong">Ping-pong · there and back</option><option value="once">Once · hold at end</option><option value="loop">Loop · restart at beginning</option></select></label>
          <p className="tween-note">Duration is one direction. Ping-pong takes twice the duration for a complete trip. Linear is ordinary lerp; the other variants ease the progress.</p>
          <button className="settings-save" type="submit" disabled={!valid||!name.trim()}>{saving?"Validating & saving…":"Save selected variant"}</button>
        </fieldset></form>
        {(error||validation)&&<p className="tween-error" role="alert">{error||validation}</p>}
        {saved&&<p className="tween-saved" role="status">Saved <b>{saved.name}</b> · {EASINGS.find(item=>item.id===saved.settings.easing)?.name}. <Link href={`/tweens?asset=${encodeURIComponent(saved.path)}&scene=${encodeURIComponent(scenePath)}`}>Open saved variant ↗</Link></p>}
        <section className="tween-library" aria-label="Saved tween variants"><h3>Project variants <span>{library.length}</span></h3>{!library.length&&<p className="tween-note">Saved recipes will appear here. Names can repeat; every asset has its own ID.</p>}{library.map(asset=><button key={asset.path} type="button" disabled={saving} onClick={()=>load(asset)}><b>{asset.name}</b><small>{EASINGS.find(item=>item.id===asset.settings.easing)?.name} · {asset.settings.duration_seconds}s</small></button>)}{warnings.length>0&&<p role="alert" className="tween-error">Could not load {warnings.length} invalid tween asset(s): {warnings.join(", ")}. Their source files were left untouched.</p>}</section>
      </aside>
      <section className="tween-workspace">
        <section className="panel tween-preview"><div className="panel-head"><h2>Motion preview</h2><span className="kind-badge">{label}</span></div>
          <div className="tween-lane" aria-label="Tween motion preview"><span className="tween-end-label">{Number.isFinite(draft.from)?Math.min(draft.from,draft.to):"Start"}</span><div className="tween-track"/><div ref={object} className="tween-cube" aria-label="Animated preview object"><svg viewBox="0 0 80 90" aria-hidden="true"><path d="M40 3 75 23 40 43 5 23Z" fill="#c4e6cc"/><path d="M5 23 40 43 40 84 5 64Z" fill="#7baf91"/><path d="M40 43 75 23 75 64 40 84Z" fill="#4f8065"/><path d="M40 3 75 23 75 64 40 84 5 64 5 23Z" fill="none" stroke="#d5f2df" strokeWidth="1.5"/></svg></div><span className="tween-end-label">{Number.isFinite(draft.to)?Math.max(draft.from,draft.to):"End"}</span></div>
          <div className="tween-playback"><button className="quiet-button" type="button" disabled={!valid} onClick={()=>setPlaying(value=>!value)}>{playing?"Pause":"Play"}</button><button className="quiet-button" type="button" disabled={!valid} onClick={()=>{elapsed.current=0;setPlaying(true);}}>Restart</button><input ref={scrubber} type="range" aria-label="Scrub tween progress" min="0" max="1" step="0.001" defaultValue="0" disabled={!valid} onChange={event=>{const value=Number(event.target.value);elapsed.current=value*draft.duration_seconds;setSeek(revision=>revision+1);setPlaying(false);}}/><output ref={readout} aria-label="Tween value">{draft.from.toFixed(3)} · 0.00 s</output></div>
        </section>
        <section className="panel tween-curve"><div className="panel-head"><h2>{label}</h2><span className="tween-note">TIME → VALUE</span></div><svg viewBox="0 0 520 270" role="img" aria-label={`${label} easing curve`}><g stroke="#304939" strokeWidth="1">{[0,0.25,0.5,0.75,1].map(t=><g key={t}><path d={`M40 ${230-t*200}H480`}/><path d={`M${40+t*440} 30V230`}/></g>)}</g><path d="M40 230 480 30" fill="none" stroke="#597363" strokeDasharray="5 6"/><polyline points={curve(draft.easing,440,200)} transform="translate(40 30)" fill="none" stroke="#b8d9c2" strokeWidth="3"/><circle ref={cursor} cx="40" cy="230" r="6" fill="#e5be80" stroke="#1a2b20" strokeWidth="2"/><text x="40" y="253">0 s</text><text x="480" y="253" textAnchor="end">{Number.isFinite(draft.duration_seconds)?draft.duration_seconds:"—"} s</text><text x="20" y="234">0</text><text x="20" y="34">1</text></svg></section>
        <section className="panel tween-variants"><div className="panel-head"><h2>Choose a variant</h2><span className="tween-note">SAME DURATION / DIFFERENT FEEL</span></div><div className="tween-variant-grid" role="group" aria-label="Easing variants">{EASINGS.map(item=><button type="button" key={item.id} aria-pressed={draft.easing===item.id} disabled={saving} onClick={()=>change({...draft,easing:item.id})}><svg viewBox="-4 -4 108 58" aria-hidden="true"><path d="M0 50H100M0 0V50" stroke="#3d5746" fill="none"/><polyline points={curve(item.id)} fill="none" stroke="currentColor" strokeWidth="2"/></svg><span>{item.name}</span></button>)}</div></section>
      </section>
    </div>
  </main>;
}
