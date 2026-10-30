"use client";
import {TerrainLibrary} from "./terrain-library";
import {TerrainMethodSwitch,TerrainBiomePicker} from "./terrain-builder-controls";
import type {TerrainBiome} from "@/lib/terrain-biomes";
import type {TerrainLibraryItem} from "@/lib/terrain-library-types";
import {FalBalance} from "@/components/fal-balance";
import Link from "next/link";
import {useEditorNavigationGuard} from "./editor-navigation";
import { useCallback, useEffect, useRef, useState, type SyntheticEvent } from "react";
import {TerrainMaterialControls,type LibraryMaterial} from "./terrain-material-controls";
import {terrainPaint,type TerrainPaint} from "@/lib/terrain-paint";
import {terrainPaintPreview} from "@/lib/terrain-paint-preview";
import {TerrainSculptor} from "./terrain-sculptor";
import {terrainGlbHeights,terrainHeights} from "@/lib/terrain-brush";
import { ModelPreview } from "./model-preview";
import { TERRAIN_RESOLUTIONS, TERRAIN_COMPARE_MAX_RESOLUTION, DEFAULT_TERRAIN, TERRAIN_PRESETS, terrainSettings, terrainGlb, type TerrainSettings, type TerrainMetrics } from "@/lib/terrain";

type Variant={url:string;settings:TerrainSettings;label:string;metrics:TerrainMetrics;values:Float32Array};

export function TerrainBuilder({projectId,projectName,scenes,initialScene,initialInstance,initialMaterials,initialLibrary,initialBiome}:{initialBiome?:TerrainBiome;initialLibrary:TerrainLibraryItem[];initialMaterials:LibraryMaterial[];projectId:string;projectName:string;scenes:{path:string;name:string;revision:string}[];initialScene:string;initialInstance?:string}){
 const [instance,setInstance]=useState(initialInstance),[library,setLibrary]=useState(initialLibrary),[libraryMessage,setLibraryMessage]=useState(""),[pending,setPending]=useState<{path:string}|{create:true}|null>(null),discardDialog=useRef<HTMLDialogElement>(null);
 useEffect(()=>{if(pending)discardDialog.current?.showModal();else discardDialog.current?.close();},[pending]);
 const [draft,setDraft]=useState<TerrainSettings>(()=>{const b=TERRAIN_PRESETS.find(b=>b.id===initialBiome);return b?{...DEFAULT_TERRAIN,preset:b.id,height:b.height,frequency:b.frequency,octaves:b.octaves,roughness:b.roughness}:DEFAULT_TERRAIN;}),[preview,setPreview]=useState<{url:string;settings:TerrainSettings;metrics:TerrainMetrics;values:Float32Array;edited?:boolean;variants?:Variant[]}|null>(null),[busy,setBusy]=useState(Boolean(initialInstance)),[error,setError]=useState("");
 const [destination,setDestination]=useState(initialScene),[name,setName]=useState("Terrain"),[collider,setCollider]=useState(true),[saving,setSaving]=useState(false),[saved,setSaved]=useState<{id:string;scenePath:string;sceneRevision:string}|null>(null),[revisions,setRevisions]=useState<Record<string,string>>({});
 const [edit,setEdit]=useState<{assetPath:string;revision:string;libraryRevision?:string;libraryName?:string}|null>(null),[changed,setChanged]=useState(false);
 const [painting,setPainting]=useState(false);
 const [paint,setPaint]=useState<TerrainPaint|null>(null),[materials,setMaterials]=useState<LibraryMaterial[]>(initialMaterials);
 const [paintedPreview,setPaintedPreview]=useState<{url:string;paint:TerrainPaint;values:Float32Array;materials:LibraryMaterial[];failed?:boolean}|null>(null);
 const dirty=changed||Boolean(preview&&Object.entries(draft).some(([key,value])=>preview.settings[key as keyof TerrainSettings]!==value));
 const paintedUrl=paint?paintedPreview?.url:undefined,previewing=Boolean(paint&&preview&&!preview.variants&&(paintedPreview?.paint!==paint||paintedPreview?.values!==preview.values||paintedPreview?.materials!==materials));
 useEditorNavigationGuard(()=>!busy&&!saving&&!painting&&(!dirty||window.confirm("Discard unsaved terrain changes?")));
 useEffect(()=>{const before=(event:BeforeUnloadEvent)=>{if(dirty)event.preventDefault();};window.addEventListener("beforeunload",before);return()=>window.removeEventListener("beforeunload",before);},[dirty]);
 const previewUrls=useRef(new Set<string>());
 const previewLoaded=useCallback((current:string)=>{for(const url of previewUrls.current)if(url!==current){URL.revokeObjectURL(url);previewUrls.current.delete(url);}},[]);
 useEffect(()=>()=>{for(const url of previewUrls.current)URL.revokeObjectURL(url);},[]);
 async function refreshMaterials(){try{const response=await fetch(`/api/materials?projectId=${encodeURIComponent(projectId)}`),result=await response.json();if(!response.ok)throw new Error(result.error);setMaterials(result.materials);}catch(error){setError(error instanceof Error?error.message:"Could not load materials.");}}
 useEffect(()=>{
  if(!preview||preview.variants||!paint)return;
  if(!materials.length)return;
  let active=true;
  terrainPaintPreview(projectId,preview.settings,preview.values,paint,materials).then(bytes=>{if(active){const url=URL.createObjectURL(new Blob([bytes],{type:"model/gltf-binary"}));previewUrls.current.add(url);setPaintedPreview({url,paint,values:preview.values,materials});}}).catch(error=>{if(active){setError(error.message);setPaintedPreview(previous=>({url:previous?.url??"",paint,values:preview.values,materials,failed:true}));}});
  return()=>{active=false;};
 },[projectId,preview?.settings,preview?.values,preview?.variants,paint,materials]); // eslint-disable-line react-hooks/exhaustive-deps
 function updatePaint(value:TerrainPaint|null){setPaint(value);setChanged(true);setSaved(null);}

 const [generation,setGeneration]=useState(0);
 const request=useRef<AbortController|null>(null);
 useEffect(()=>()=>request.current?.abort(),[]);
 useEffect(()=>()=>{if(preview){for(const item of preview.variants??[preview])URL.revokeObjectURL(item.url);}},[preview]);
 useEffect(()=>{
  const instanceId=initialInstance;if(!instanceId)return;
  const query=new URLSearchParams({projectId,scene:initialScene,instance:instanceId});
  const controller=new AbortController();request.current=controller;
  async function load(){
   try{
    const response=await fetch(`/api/terrain?${query}`,{signal:controller.signal}),result=await response.json();
    if(!response.ok)throw new Error(result.error??"Terrain could not be loaded.");
    if(controller.signal.aborted)return;
    const settings=terrainSettings(result.settings),values=terrainHeights(result.heights,settings),start=performance.now(),bytes=terrainGlb(settings,values);
    setPaint(result.paint?terrainPaint(result.paint,settings.resolution):null);setDraft(settings);setName(result.name);setCollider(result.collider);setEdit({assetPath:result.assetPath,revision:result.editRevision,libraryRevision:result.assetRevision,libraryName:result.assetName});setRevisions(previous=>({...previous,[initialScene]:result.sceneRevision}));
    setPreview({url:URL.createObjectURL(new Blob([bytes],{type:"model/gltf-binary"})),settings,values,edited:true,metrics:{generationMs:result.generationMs,meshMs:performance.now()-start,fieldBytes:values.byteLength,glbBytes:bytes.byteLength}});setChanged(false);
   }catch(error){if(!controller.signal.aborted)setError(error instanceof Error?error.message:"Terrain could not be loaded.");}finally{if(!controller.signal.aborted)setBusy(false);}
  }
  void load();return()=>controller.abort();
 },[projectId,initialScene,initialInstance]);
 const preset=TERRAIN_PRESETS.find(preset=>preset.id===draft.preset)!;
 async function generate(event:SyntheticEvent,compare=false){
  event.preventDefault();if(busy||saving||painting)return;setBusy(true);setError("");setSaved(null);
  const controller=new AbortController();request.current=controller;
  try{
   const settings=terrainSettings(draft),response=await fetch("/api/terrain",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,settings,compare}),signal:controller.signal});
   if(!response.ok){const result=await response.json();throw new Error(result.error??"Terrain generation failed.");}setGeneration(previous=>previous+1);setChanged(true);setPaint(null);setPaintedPreview(null);
   if(compare){
    const result=await response.json();if(controller.signal.aborted)return;
    const variants:Variant[]=result.variants.map((item:{glb:string;settings:TerrainSettings;label:string;metrics:TerrainMetrics})=>({settings:item.settings,label:item.label,metrics:item.metrics,values:terrainGlbHeights(Uint8Array.from(atob(item.glb),char=>char.charCodeAt(0)).buffer,item.settings),url:URL.createObjectURL(new Blob([Uint8Array.from(atob(item.glb),char=>char.charCodeAt(0))],{type:"model/gltf-binary"}))}));
    setPreview({...variants[2],variants});
   }else{
    const blob=await response.blob();if(controller.signal.aborted)return;setPreview({url:URL.createObjectURL(blob),settings,values:terrainGlbHeights(await blob.arrayBuffer(),settings),metrics:JSON.parse(response.headers.get("X-Terrain-Metrics")!)});
   }
  }catch(error){if(!controller.signal.aborted)setError(error instanceof Error?error.message:"Terrain generation failed.");}finally{if(!controller.signal.aborted)setBusy(false);}
 }
 async function save(){
  if(!preview||busy||saving||painting||Boolean(instance&&!edit))return;setSaving(true);setError("");setSaved(null);
  try{
   const scenePath=instance?initialScene:destination,scene=scenes.find(scene=>scene.path===scenePath);if(!scene)throw new Error("Choose a destination scene.");
   const response=await fetch("/api/terrain/save",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,scenePath,sceneRevision:revisions[scenePath]??scene.revision,name,settings:preview.settings,paint,heights:(instance||preview.edited)?Array.from(preview.values):undefined,collider,instanceId:instance,editRevision:edit?.revision})});
   const result=await response.json();if(!response.ok)throw new Error(result.error??"Terrain could not be saved.");
   setRevisions(previous=>({...previous,[result.scenePath]:result.sceneRevision}));setSaved(result);setChanged(false);setEdit({assetPath:result.path,revision:instance?result.editRevision:result.assetRevision,libraryRevision:result.assetRevision,libraryName:result.assetName});await reloadLibrary();
  }catch(error){setError(error instanceof Error?error.message:"Terrain could not be saved.");}finally{setSaving(false);}
 }
 async function reloadLibrary(){const response=await fetch(`/api/terrain/library?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"}),result=await response.json();if(!response.ok)throw new Error(result.error);setLibrary(result.items);setRevisions(Object.fromEntries(result.scenes.map((scene:{path:string;revision:string})=>[scene.path,scene.revision])));}
 async function loadLibrary(path:string){if(busy||saving||painting)return;setBusy(true);setError("");setLibraryMessage("");try{const response=await fetch(`/api/terrain/library?projectId=${encodeURIComponent(projectId)}&asset=${encodeURIComponent(path)}`),result=await response.json();if(!response.ok)throw new Error(result.error);const settings=terrainSettings(result.settings),values=terrainHeights(result.heights,settings),start=performance.now(),bytes=terrainGlb(settings,values);setDraft(settings);setName(result.name);setCollider(result.collider);setPaintedPreview(null);setInstance(undefined);setEdit({assetPath:result.assetPath,revision:result.revision,libraryRevision:result.revision,libraryName:result.name});setPaint(result.paint?terrainPaint(result.paint,settings.resolution):null);setPreview({url:URL.createObjectURL(new Blob([bytes],{type:"model/gltf-binary"})),settings,values,edited:true,metrics:{generationMs:result.generationMs,meshMs:performance.now()-start,fieldBytes:values.byteLength,glbBytes:bytes.byteLength}});setGeneration(n=>n+1);setSaved(null);setChanged(false);setLibraryMessage(`${result.name} loaded.`);}catch(error){setError(error instanceof Error?error.message:"Could not load terrain.");}finally{setBusy(false);}}
 function newTerrain(){setPreview(null);setDraft({...DEFAULT_TERRAIN});setName("Terrain");setCollider(true);setInstance(undefined);setEdit(null);setPaint(null);setPaintedPreview(null);setSaved(null);setChanged(false);setError("");setLibraryMessage("");setGeneration(n=>n+1);}
 function requestTerrain(target:{path:string}|{create:true}){if(busy||saving||painting)return;if(dirty)setPending(target);else if("path"in target)void loadLibrary(target.path);else newTerrain();}
 function discardTerrain(){const target=pending;setPending(null);if(target){if("path"in target)void loadLibrary(target.path);else newTerrain();}}
 async function saveLibrary(copy=false){if(!preview||busy||saving||painting)return;setSaving(true);setError("");try{const response=await fetch("/api/terrain/library",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,...(!copy&&edit?{assetPath:edit.assetPath,revision:edit.libraryRevision??edit.revision}:{}),name:copy?`${name.slice(0,59)} copy`:edit?.libraryName??name,settings:preview.settings,heights:Array.from(preview.values),paint,collider})}),result=await response.json();if(!response.ok)throw new Error(result.error);setInstance(undefined);setEdit({assetPath:result.assetPath,revision:result.revision,libraryRevision:result.revision,libraryName:result.name});setName(result.name);setSaved(null);setChanged(false);await reloadLibrary();setLibraryMessage(`${result.name} saved to the terrain library.`);}catch(error){setError(error instanceof Error?error.message:"Terrain could not be saved.");}finally{setSaving(false);}}
 async function placeTerrain(item:TerrainLibraryItem){if(busy||saving||painting)return;setSaving(true);setError("");try{const scene=scenes.find(scene=>scene.path===destination);if(!scene)throw new Error("Choose a destination scene.");const response=await fetch("/api/terrain/library",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,action:"place",assetPath:item.path,scenePath:destination,sceneRevision:revisions[destination]??scene.revision})}),result=await response.json();if(!response.ok)throw new Error(result.error);setSaved(result);await reloadLibrary();setLibraryMessage(`${item.name} added to ${scene.name}.`);}catch(error){setError(error instanceof Error?error.message:"Terrain could not be placed.");}finally{setSaving(false);}}
 function updateDraft(value:TerrainSettings){setDraft(value);setChanged(true);}
 function sculpt(values:Float32Array){
  if(!preview||busy||saving)return;
  const start=performance.now(),bytes=terrainGlb(preview.settings,values),url=URL.createObjectURL(new Blob([bytes],{type:"model/gltf-binary"}));
  setPreview({...preview,url,values,edited:true,variants:undefined,metrics:{...preview.metrics,meshMs:performance.now()-start,glbBytes:bytes.byteLength}});setSaved(null);setChanged(true);
 }
 function number(key:"seed"|"size"|"height"|"frequency"|"octaves"|"mountainCount",label:string,min:number,max:number,step:number){return <label className="settings-field"><span>{label}</span><input type="number" aria-label={label} min={min} max={max} step={step} required value={Number.isFinite(draft[key])?draft[key]:""} onChange={event=>setDraft({...draft,[key]:event.target.valueAsNumber})}/></label>;}
 function slider(key:"sunStrength"|"sunAzimuth"|"sunElevation"|"domainWarp"|"ridgeStrength",label:string,min:number,max:number,step:number,unit=""){
  return <label className="settings-field terrain-range"><span>{label}<output>{(key==="sunStrength"||key==="domainWarp"||key==="ridgeStrength")?draft[key].toFixed(2):draft[key]}{unit}</output></span><input type="range" aria-label={label} min={min} max={max} step={step} value={draft[key]} onChange={event=>setDraft({...draft,[key]:event.target.valueAsNumber})}/></label>;
 }
 return <main className="object-shell terrain-shell">
  <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Terrain Builder</b></span><Link href="/objects" className="settings-back">Object Creator</Link><Link href="/textures" className="quiet-button">Texture Creator</Link><Link href="/materials" className="quiet-button">Material Generator</Link><Link href={`/?scene=${encodeURIComponent(destination)}`} className="quiet-button">Project map ↗</Link><FalBalance/></header>
  <div className="object-heading terrain-library-heading"><div className="terrain-library-title"><h1>Terrain Builder</h1><TerrainMethodSwitch method="heightfield" biome={draft.preset} scene={destination} canLeave={()=>!busy&&!saving&&!painting&&(!dirty||window.confirm("Discard unsaved terrain changes?"))}/><TerrainBiomePicker value={draft.preset} method="heightfield" disabled={busy||saving||painting} onChange={biome=>{const item=TERRAIN_PRESETS.find(item=>item.id===biome);if(item)updateDraft({...draft,preset:item.id,height:item.height,frequency:item.frequency,octaves:item.octaves,roughness:item.roughness});}}/></div><TerrainLibrary items={library} projectId={projectId} selected={edit?.assetPath} disabled={busy||saving||painting} canSave={Boolean(preview)&&(!edit||changed)} scene={destination} scenes={scenes} onScene={setDestination} onLoad={path=>requestTerrain({path})} onPlace={item=>void placeTerrain(item)} onNew={()=>requestTerrain({create:true})} onSave={()=>void saveLibrary()} onRefresh={()=>void reloadLibrary().catch(error=>setError(error.message))} message={libraryMessage} error={error}/></div>
  <div className="object-layout terrain-layout">
   <aside className="panel terrain-controls"><div className="panel-head"><h2>Terrain generation</h2><span className="kind-badge">SEED / SHAPE</span></div>
    <form className="terrain-form" onSubmit={generate}><fieldset disabled={busy||saving||painting}>
     <legend className="sr-only">Terrain parameters</legend>
     <p className="terrain-description">{preset.description}</p>
     <div className="terrain-seed">{number("seed","Seed",0,4294967295,1)}<button type="button" className="quiet-button" aria-label="Randomize terrain seed" onClick={()=>updateDraft({...draft,seed:crypto.getRandomValues(new Uint32Array(1))[0]})}>↻</button></div>
     <div className="terrain-parameters">{number("size","Width (m)",1,10000,1)}{number("height","Relief (m)",0,1000,1)}{number("frequency","Frequency",0.1,8,0.05)}<label className="settings-field terrain-range"><span>Roughness <output>{draft.roughness.toFixed(2)}</output></span><input aria-label="Roughness" type="range" min="0" max="1" step="0.01" value={draft.roughness} onChange={event=>updateDraft({...draft,roughness:event.target.valueAsNumber})}/></label>{number("octaves","Octaves",1,8,1)}
      <label className="settings-field"><span>Resolution</span><select aria-label="Terrain resolution" value={draft.resolution} onChange={event=>updateDraft({...draft,resolution:Number(event.target.value)})}>{TERRAIN_RESOLUTIONS.map(size=><option key={size} value={size}>{size} × {size}</option>)}</select></label>
     </div>
     <label className="settings-field"><span>Base generator</span><select aria-label="Base generator" value={draft.generator} onChange={event=>updateDraft({...draft,generator:event.target.value as TerrainSettings["generator"]})}><option value="noise">Perlin noise</option><option value="diamond-square">Diamond-square</option></select></label>
     <p className="terrain-description">Frequency and octaves affect Perlin only. Roughness controls octave amplitude or diamond-square displacement decay.</p>
     <div className="terrain-parameters">{slider("domainWarp","Domain warp",0,1,0.01)}{slider("ridgeStrength","Ridge strength",0,1,0.01)}</div>
     <label className="settings-field terrain-range"><span>Thermal erosion passes <output>{draft.erosionIterations}</output></span><input aria-label="Thermal erosion passes" type="range" min="0" max="64" step="1" value={draft.erosionIterations} onChange={event=>updateDraft({...draft,erosionIterations:event.target.valueAsNumber})}/></label>
     <p className="terrain-description">Ridges connect major mountains. Erosion moves material down steep slopes; zero passes disables it.</p>
     {draft.preset==="mountainous"&&<>{number("mountainCount","Mountain count",1,12,1)}<p className="terrain-description">Counts major mountain groups. Each group has uneven summits, saddles and broad shoulders.</p></>}
     <section className="terrain-light-controls" aria-label="Directional light"><h3>☀ Directional light <small>Generate to apply</small></h3>
      <div className="terrain-light-grid">{slider("sunStrength","Sun strength",0,1,0.01)}
      {slider("sunAzimuth","Sun direction",0,360,1,"°")}
      {slider("sunElevation","Sun elevation",5,85,1,"°")}</div>
     </section>
     <button type="submit" className="settings-save terrain-generate">{busy?"Generating terrain…":"Generate terrain"}</button>
    <button type="button" className="quiet-button terrain-compare-button" disabled={draft.resolution>TERRAIN_COMPARE_MAX_RESOLUTION} onClick={event=>void generate(event,true)}>Compare algorithms</button>{draft.resolution>TERRAIN_COMPARE_MAX_RESOLUTION&&<p className="terrain-description">Three-way comparison is available up to 256 × 256. Generate terrain uses the selected full resolution.</p>}
    </fieldset>{error&&<p className="terrain-error" role="alert">{error}</p>}
    {preview&&!preview.variants&&<><TerrainMaterialControls settings={preview.settings} heights={preview.values} paint={paint} materials={materials} disabled={busy||saving||painting} onChange={updatePaint}/><button type="button" className="quiet-button" disabled={saving||painting} onClick={()=>void refreshMaterials()}>Refresh material library</button><p className="terrain-description">3D editing preview shows diffuse colours and images at reduced resolution. Native scene inspection renders noise, specular, emission, bump and shadow settings. Generate replaces sculpting and painting.</p></>}
    <section className="terrain-save" aria-label="Save terrain"><h3>{edit?"Update terrain asset":"Save terrain"}</h3>{instance&&<p className="terrain-description">Editing {name}. Saves update the existing terrain asset and every instance using it. Placement and physics are preserved. Material painting updates the shared terrain surface.</p>}<label className="settings-field"><span>Name</span><input aria-label="Terrain name" maxLength={64} value={name} disabled={saving||Boolean(edit)} onKeyDown={event=>{if(event.key==="Enter"){event.preventDefault();void(instance?save():saveLibrary());}}} onChange={event=>{setName(event.target.value);setSaved(null);setChanged(true);}}/></label><label className="settings-field"><span>Destination scene</span><select aria-label="Destination scene" value={instance?initialScene:destination} disabled={saving||Boolean(instance)} onChange={event=>{setDestination(event.target.value);setSaved(null);}}>{scenes.map(scene=><option value={scene.path} key={scene.path}>{scene.name}</option>)}</select></label>{!instance&&<label className="terrain-save-collider"><input type="checkbox" checked={collider} disabled={saving||Boolean(edit)} onChange={event=>{setCollider(event.target.checked);setChanged(true);setSaved(null);}}/>Terrain collider</label>}<p className="terrain-description">Saves the displayed {preview?.edited?"sculpted":preview?.variants?"Enhanced pipeline":"generated"} result. Unsaved input changes do not affect it. Collider follows the terrain and is static.</p><button type="button" className="settings-save" disabled={!preview||busy||saving||painting||Boolean(edit&&!changed)} onClick={()=>void(instance?save():saveLibrary())}>{saving?"Saving terrain…":edit?"Save terrain changes":"Save to terrain library"}</button>{edit&&<button type="button" className="quiet-button" disabled={!preview||busy||saving||painting} onClick={()=>void saveLibrary(true)}>Save as new terrain</button>}{!edit&&<button type="button" className="settings-save" disabled={!preview||busy||saving||painting||Boolean(edit&&!instance)||!scenes.length||!name.trim()||Boolean(instance&&(!edit||!changed))} onClick={()=>void save()}>{saving?"Saving terrain…":instance?"Update terrain in scene":"Save terrain to scene"}</button>}{saved&&<p role="status">{instance?"Terrain updated.":"Terrain saved."} <Link href={`/?scene=${encodeURIComponent(saved.scenePath)}&instance=${encodeURIComponent(saved.id)}&terrainRevision=${encodeURIComponent(edit?.revision??saved.sceneRevision)}`}>View in scene ↗</Link></p>}</section>
    <p className="terrain-description">Rivers carve a valley; water comes later. Comparison keeps seed, extent, relief, resolution, sunlight and initial camera matched. Cameras orbit independently.</p>
    </form>
   </aside>
   <section className="panel object-stage"><div className="object-stage-head"><div><span className="eyebrow">{instance?`EDITING ${name}`:"GRAYSCALE / DIRECTIONAL SUNLIGHT"}</span><h2>{preview?`${TERRAIN_PRESETS.find(item=>item.id===preview.settings.preset)!.name} · seed ${preview.settings.seed}`:"Terrain preview"}</h2></div>{busy?<span className="kind-badge" role="status">{instance&&!preview?"LOADING TERRAIN":"GENERATING"}</span>:preview&&<span className="kind-badge">☀ Sun {Math.round(preview.settings.sunStrength*100)}%</span>}</div>
    <div className="object-viewport">{preview?preview.variants?<div className="terrain-comparison">{preview.variants.map(item=><article className="terrain-comparison-card" key={item.url}><h3>{item.label}</h3><ModelPreview src={item.url} initialOrbit={`30deg 55deg ${Math.hypot(item.settings.size,item.settings.height)*1.8}m`} initialTarget={`0m ${item.settings.height*0.25}m 0m`} maximumOrbit={`auto auto ${Math.hypot(item.settings.size,item.settings.height)*5}m`} exposure={1} environmentImage="/terrain-ambient.hdr" name={`${item.label} terrain`}/><TerrainStats metrics={item.metrics}/></article>)}</div>:<div className="terrain-editor-layout"><TerrainSculptor key={generation} values={preview.values} settings={preview.settings} disabled={busy||saving} onPaintingChange={setPainting} onCommit={sculpt} paint={paint} onPaintCommit={updatePaint}/><div className="terrain-viewer"><ModelPreview onLoad={previewLoaded} src={paintedUrl||preview.url} initialOrbit={`30deg 55deg ${Math.hypot(preview.settings.size,preview.settings.height)*1.8}m`} exposure={1} environmentImage="/terrain-ambient.hdr" initialTarget={`0m ${preview.settings.height*0.25}m 0m`} maximumOrbit={`auto auto ${Math.hypot(preview.settings.size,preview.settings.height)*5}m`} name={`${preview.settings.preset} terrain`}/><p role="status">{previewing?"Updating material preview…":paintedPreview?.failed?"Material preview unavailable":paint?"Painted diffuse preview":""}</p><TerrainStats metrics={preview.metrics}/></div></div>:<div className="object-empty"><div className="terrain-contour" aria-hidden="true">△</div><h3>A landscape from a single seed.</h3><p>Choose a terrain style and generate to explore it in 3D.</p></div>}</div>
   </section>
  </div>
  <dialog ref={discardDialog} className="particle-discard-dialog" aria-labelledby="discard-terrain-title" onCancel={()=>setPending(null)}><h2 id="discard-terrain-title">Discard unsaved terrain changes?</h2><p>{pending&&"create"in pending?"Your current edits will be replaced by a new terrain draft.":"Your current edits will be replaced by the selected terrain."}</p><div className="particle-actions"><button type="button" className="quiet-button" onClick={()=>setPending(null)}>Cancel</button><button type="button" className="settings-save" onClick={discardTerrain}>Discard changes</button></div></dialog>
 </main>;
}

function TerrainStats({metrics}:{metrics:TerrainMetrics}){return <p className="terrain-stats">Rust {metrics.generationMs.toFixed(2)} ms · GLB prep {metrics.meshMs.toFixed(2)} ms<br/>Field {(metrics.fieldBytes/1024).toFixed(0)} KiB · GLB {(metrics.glbBytes/1024).toFixed(0)} KiB<br/><small>Output sizes, not peak RAM or GPU memory.</small></p>;}
