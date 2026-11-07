"use client";
import {useEffect,useRef} from "react";
import {addMode,MAX_MODES,modesProblem,moveMode,removeMode,renameMode,type BrainMode} from "@/lib/brain-modes";
import type {FieldOption} from "@/components/field-input";

/** Which mode panels are open, by object and mode name: saving remounts the inspector, so this outlives it. */
const openPanels=new Map<string,boolean>();

/** The Brain component's modes as collapsible panels: each runs a brain while active and hands over on finish or abort. */
export function BrainModesEditor({scope,modes,brains,onChange}:{scope:string;modes:BrainMode[];brains:FieldOption[];onChange:(modes:BrainMode[])=>void}){
 const problem=modesProblem(modes);
 // Panels start open for short lists and when just added; afterwards the author's toggles stand.
 const loaded=useRef(false);useEffect(()=>{loaded.current=true;},[]);
 const key=(name:string)=>`${scope}:${name}`;
 const initOpen=(name:string)=>(el:HTMLDetailsElement|null)=>{if(el&&!el.dataset.ready){el.dataset.ready="1";el.open=openPanels.get(key(name))??(loaded.current||modes.length<=2);}};
 const brainName=(id:string)=>brains.find(item=>item.id===id)?.name??(id?`${id} (missing)`:"No brain");
 const update=(i:number,change:Partial<BrainMode>)=>onChange(modes.map((mode,j)=>j===i?{...mode,...change}:mode));
 const link=(i:number,key:"on_next"|"on_previous",label:string,help:string)=><label className="component-field" title={help}>
  <span>{label}</span>
  <select aria-label={`${modes[i].name} ${label.toLowerCase()}`} value={modes[i][key]??""} onChange={event=>update(i,{[key]:event.target.value||undefined})}>
   <option value="">None</option>
   {modes.filter((_,j)=>j!==i).map(other=><option key={other.name} value={other.name}>{other.name}</option>)}
  </select><span/>
 </label>;
 return <div className="brain-modes" aria-label="Brain modes">
  {modes.map((mode,i)=><details key={i} ref={initOpen(mode.name)} className="brain-mode" onToggle={event=>openPanels.set(key(mode.name),event.currentTarget.open)}>
   <summary><span className={`brain-mode-dot${mode.active?" on":""}`} title={mode.active?"Active at start":"Starts off"}/><b>{mode.name}</b><span className="brain-mode-brain">{brainName(mode.brain)}</span></summary>
   <label className="component-field" title="1–32 characters of a–z, 0–9, _ and -. Switch mode nodes and hand-overs use it.">
    <span>Name</span><input type="text" aria-label={`Mode ${i+1} name`} maxLength={32} value={mode.name} onChange={event=>{const name=event.target.value.toLowerCase();openPanels.set(key(name),openPanels.get(key(mode.name))??true);onChange(renameMode(modes,i,name));}}/><span/>
   </label>
   <label className="component-field" title="The brain this mode runs.">
    <span>Brain</span>
    <select aria-label={`${mode.name} brain`} value={mode.brain} onChange={event=>update(i,{brain:event.target.value})}>
     {!mode.brain&&<option value="">Choose…</option>}
     {mode.brain&&!brains.some(item=>item.id===mode.brain)&&<option value={mode.brain}>{mode.brain} (missing)</option>}
     {brains.map(item=><option key={item.id} value={item.id}>{item.name}</option>)}
    </select><span/>
   </label>
   <label className="component-field" title="Running when the scene starts."><span>Active at start</span><input type="checkbox" aria-label={`${mode.name} active at start`} checked={mode.active} onChange={event=>update(i,{active:event.target.checked})}/><span/></label>
   {link(i,"on_next","On next","Started when this mode finishes: its Sequence ends, a finishing node arrives, or a Next mode node runs.")}
   {link(i,"on_previous","On previous","Started when this mode is aborted by a Previous mode node.")}
   <label className="component-field" title="Stay active after handing over."><span>Keep running</span><input type="checkbox" aria-label={`${mode.name} keep running`} checked={mode.keep_running===true} onChange={event=>update(i,{keep_running:event.target.checked||undefined})}/><span/></label>
   <div className="brain-mode-actions">
    <button type="button" aria-label={`Move ${mode.name} up`} title="Earlier modes steer first" disabled={i===0} onClick={()=>onChange(moveMode(modes,i,-1))}>↑</button>
    <button type="button" aria-label={`Move ${mode.name} down`} disabled={i===modes.length-1} onClick={()=>onChange(moveMode(modes,i,1))}>↓</button>
    <button type="button" aria-label={`Remove mode ${mode.name}`} disabled={modes.length===1} onClick={()=>onChange(removeMode(modes,i))}>Remove</button>
   </div>
  </details>)}
  <button type="button" className="brain-mode-add" disabled={modes.length>=MAX_MODES||brains.length===0} title={brains.length?"Add a mode running another brain":"Create a brain in the Brain Editor first"} onClick={()=>onChange(addMode(modes,brains[0]?.id??""))}>＋ Add new</button>
  {problem&&<p className="transform-error" role="alert">{problem}</p>}
 </div>;
}
