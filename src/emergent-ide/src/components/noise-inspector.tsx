"use client";
import {useEffect,useRef,useState} from "react";
import {useLiveInspector} from "@/hooks/use-live-inspector";
import {createPortal} from "react-dom";
import {decodeNoiseField,DEFAULT_NOISE,noiseSettings,type NoiseField,type NoiseSettings} from "@/lib/noise-field";
type Document={path:string;revision:string;settings:NoiseSettings};
const descriptions={perlin:"Smooth gradient noise: a useful starting point for terrain and textures.","open-simplex":"Another smooth gradient pattern, useful for organic variation.",worley:"Distance-based cellular noise, useful for cells, stones and clustered masks."};
export function NoiseInspector({projectId,path,previewTarget,onDirty,onBusy,onSaved}:{projectId:string;path:string;previewTarget:HTMLElement|null;onDirty:(dirty:boolean)=>void;onBusy:(busy:boolean)=>void;onSaved:()=>void}){
 const [document,setDocument]=useState<Document|null>(null),[field,setField]=useState<NoiseField|null>(null),[generated,setGenerated]=useState<NoiseSettings|null>(null),[slice,setSlice]=useState(0),[error,setError]=useState(""),[message,setMessage]=useState(""),[busy,setBusy]=useState(false);
 const liveSave=useLiveInspector(document?.settings??DEFAULT_NOISE,noiseSettings,async settings=>{
  if(!document||!settings)throw new Error("Noise definition is not loaded.");
  const response=await fetch("/api/filters/noise",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path,settings,revision:document.revision})});
  const data=await response.json();if(!response.ok)throw new Error(data.error||"Noise settings could not be saved.");
  setDocument(data);onSaved();setMessage("Settings saved automatically.");
 },onDirty,onBusy);
 const draft=liveSave.draft;
 const feedback=useRef<HTMLDivElement>(null);
 useEffect(()=>{if(error||message)feedback.current?.scrollIntoView({block:"nearest"});},[error,message]);
 useEffect(()=>{let active=true;void fetch(`/api/filters/noise?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}`,{cache:"no-store"}).then(async response=>{const data=await response.json();if(!response.ok)throw new Error(data.error);if(active){setDocument(data);}}).catch(error=>{if(active)setError(error.message);});return()=>{active=false;};},[projectId,path,onDirty]);
 function change(settings:NoiseSettings){liveSave.change(settings);setMessage("");}
 async function perform(){
  if(!document)return;setBusy(true);onBusy(true);setError("");setMessage("");
  try{const response=await fetch("/api/filters/noise",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path,settings:draft,revision:document.revision})});
   if(!response.ok){const data=await response.json();throw new Error(data.error||"Noise operation failed.");}
   {const value=decodeNoiseField(await response.arrayBuffer());setField(value);setGenerated({...draft,offset:[...draft.offset]});setSlice(0);setMessage("Field generated from the current draft. Settings save automatically.");}
  }catch(error){setError(error instanceof Error?error.message:"Noise operation failed.");}finally{setBusy(false);onBusy(false);}
 }
 const preview=<NoisePreview field={field} settings={generated} slice={slice} setSlice={setSlice} stale={Boolean(generated&&JSON.stringify(generated)!==JSON.stringify(draft))}/>;
 return <div className="noise-inspector">
  <p className="noise-description">A reusable filter asset. Generate a grayscale field from editable inputs; save settings separately.</p>
  <fieldset disabled={busy||liveSave.saving||!document}>
   <label>Algorithm<select aria-label="Noise algorithm" value={draft.algorithm} onChange={event=>change({...draft,algorithm:event.target.value as NoiseSettings["algorithm"]})}><option value="perlin">Perlin</option><option value="open-simplex">OpenSimplex</option><option value="worley">Worley (cellular)</option></select></label>
   <p className="noise-description">{descriptions[draft.algorithm]}</p>
   <label>Field<select aria-label="Noise dimensions" value={draft.dimensions} onChange={event=>change({...draft,dimensions:Number(event.target.value),size:Number(event.target.value)===3?32:128})}><option value="2">2D field</option><option value="3">3D volume</option></select></label>
   <div className="noise-input-row"><label>Seed<input aria-label="Noise seed" type="number" min="0" max="4294967295" step="1" value={Number.isFinite(draft.seed)?draft.seed:""} onChange={event=>change({...draft,seed:event.target.valueAsNumber})}/></label><button type="button" onClick={()=>change({...draft,seed:crypto.getRandomValues(new Uint32Array(1))[0]})}>New seed</button></div>
   <label>Resolution per axis<input aria-label="Noise resolution" type="number" min="2" max={draft.dimensions===2?512:128} step="1" value={Number.isFinite(draft.size)?draft.size:""} onChange={event=>change({...draft,size:event.target.valueAsNumber})}/></label>
   <div className="noise-parameters">{([["frequency","Frequency",0.0001,64],["extent","Domain extent",0.001,1024],["octaves","Octaves",1,8],["lacunarity","Lacunarity",1,4],["persistence","Persistence",0,1]]as const).map(([key,label,min,max])=><label key={key}>{label}<input aria-label={`Noise ${label.toLowerCase()}`} type="number" min={min} max={max} step={key==="octaves"?1:"any"} value={Number.isFinite(draft[key])?draft[key]:""} onChange={event=>change({...draft,[key]:event.target.valueAsNumber})}/></label>)}</div>
   <div className="noise-offset">{["X","Y","Z"].map((axis,index)=><label key={axis}>Offset {axis}<input aria-label={`Noise offset ${axis}`} type="number" min="-10000" max="10000" step="any" disabled={draft.dimensions===2&&index===2} value={Number.isFinite(draft.offset[index])?draft.offset[index]:""} onChange={event=>change({...draft,offset:draft.offset.map((v,i)=>i===index?event.target.valueAsNumber:v)})}/></label>)}</div>
  </fieldset>
  <div className="noise-actions"><button disabled={busy||liveSave.saving||liveSave.dirty||!document} onClick={()=>void perform()}>{busy?"Working…":"Generate preview"}</button></div>
  <p className="noise-description" role="status">{liveSave.status}</p>{liveSave.error&&<p className="noise-error" role="alert">{liveSave.error}</p>}
  <div ref={feedback}>{error&&<p className="noise-error" role="alert">{error}</p>}{message&&<p className="noise-message" role="status">{message}</p>}</div>
  {previewTarget?createPortal(preview,previewTarget):preview}
 </div>;
}
function NoisePreview({field,settings,slice,setSlice,stale}:{field:NoiseField|null;settings:NoiseSettings|null;slice:number;setSlice:(slice:number)=>void;stale:boolean}){
 const canvas=useRef<HTMLCanvasElement>(null);
 useEffect(()=>{
  if(!field||!canvas.current)return;const context=canvas.current.getContext("2d");if(!context)return;
  const image=context.createImageData(field.width,field.height),start=slice*field.width*field.height;
  for(let i=0;i<field.width*field.height;i++){const gray=Math.round(field.values[start+i]*255);image.data[i*4]=gray;image.data[i*4+1]=gray;image.data[i*4+2]=gray;image.data[i*4+3]=255;}
  context.putImageData(image,0,0);
 },[field,slice]);
 return <section className="noise-preview" aria-label="Noise field preview">
  <header><span className="eyebrow">FILTER / NOISE FUNCTION</span><h2>{field?`${settings?.algorithm} · ${field.depth===1?"2D field":"3D volume"}`:"Noise field"}</h2><p>{field?`${field.width} × ${field.height}${field.depth>1?` × ${field.depth}`:""} · ${field.values.length.toLocaleString()} samples · Seed ${settings?.seed}`:"Choose inputs in the inspector, then Generate preview."}</p></header>
  {field?<><canvas ref={canvas} width={field.width} height={field.height} role="img" aria-label="Grayscale noise field" data-depth={field.depth} data-seed={settings?.seed} data-slice={slice}/>
   {field.depth>1&&<label className="noise-slice">Z slice <input aria-label="Noise Z slice" type="range" min="0" max={field.depth-1} step="1" value={slice} onChange={event=>setSlice(Number(event.target.value))}/><output>{slice} / {field.depth-1}</output></label>}
   <p className="noise-range">Field range {field.min.toFixed(4)} — {field.max.toFixed(4)}{stale?" · Preview uses earlier inputs; generate again to update.":""}</p>
  </>:<div className="noise-empty">2D fields · 3D volume slices<br/>Deterministic generation in Rust</div>}
  <div className="noise-legend"><span>0 · Black</span><i/><span>1 · White</span></div>
  <p className="noise-description">Linear grayscale · no automatic contrast stretching{field&&field.depth>1?" · XY slice of a complete 3D field":""}</p>
 </section>;
}
