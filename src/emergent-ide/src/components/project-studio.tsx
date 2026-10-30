"use client";

import {FalBalance} from "@/components/fal-balance";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import {EditorRail} from "./editor-navigation";
import { useRouter } from "next/navigation";
import { AddAssetModal } from "@/components/add-asset-modal";
import { NewSceneModal } from "@/components/new-scene-modal";
import { ProjectChat } from "@/components/project-chat";
import { ProjectRunControl } from "@/components/project-run-control";
import { StatementsPanel } from "@/components/statements-panel";
import { ModelPreview } from "@/components/model-preview";
import { MaterialInspector } from "@/components/material-inspector";
import type { MaterialSettings } from "@/lib/material-settings";
import { PhysicsInspector } from "@/components/physics-inspector";
import {CameraInspector} from "@/components/camera-inspector";
import {cameraAssetSettings,replaceCameraAsset,type CameraSettings} from "@/lib/camera-settings";
import type { PhysicsSettings } from "@/lib/scene-placement";
import { TransformInspector } from "@/components/transform-inspector";
import { InspectionViewport } from "@/components/inspection-viewport";
import { useSceneInspection } from "@/hooks/use-scene-inspection";
import type { SceneInstance, ScenePlacement, TransformSpace, TransformValues } from "@/lib/scene-placement";
import { SceneGraph } from "@/components/scene-graph";
import { NoiseInspector } from "@/components/noise-inspector";
import { TomlEditor } from "@/components/toml-editor";
import { projectScenes } from "@/lib/project-scenes";
import type { PublicAISettings } from "@/lib/ai-settings";
import type {
  ProjectFile,
  ProjectSnapshot,
} from "@/lib/project";

type Props = { project: ProjectSnapshot; aiSettings: PublicAISettings; initialScene?:string;initialInstance?:string;initialView?:"scene"|"source" };
type View = "preview" | "source" | "toml" | "scene";

function fileView(file: ProjectFile): View {
  return file.kind === "filter" || file.kind === "model" || file.name === "object.asset.json" ? "preview" : "source";
}

export function ProjectStudio({ project, aiSettings,initialScene=project.entryScene,initialInstance,initialView="scene" }: Props) {
  const router = useRouter();
  const searchRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [noisePreviewTarget,setNoisePreviewTarget]=useState<HTMLDivElement|null>(null);
  const [selectedPath, setSelectedPath] = useState(initialScene);
  const [view, setView] = useState<View>(initialView);
  const scenes = useMemo(() => projectScenes(project), [project]);
  const [scenePath, setScenePath] = useState(initialScene);
  const [sceneError, setSceneError] = useState("");
  const [sceneWorking, setSceneLoading] = useState(false);
  const [gizmoBusy,setGizmoBusy]=useState(false);
  const markGizmoBusy=useCallback((busy:boolean)=>setGizmoBusy(busy),[]);
  const [space,setSpace]=useState<TransformSpace>("local");
  const [tomlDirty,setTomlDirty]=useState(false),[inspectorBusy,setTomlBusy]=useState(false),[tomlEpoch,setTomlEpoch]=useState(0),[tomlScope,setTomlScope]=useState("scene");
  const [chatBusy, setChatBusy] = useState(false);
  const tomlBusy = inspectorBusy || chatBusy;
  const markTomlDirty=useCallback((dirty:boolean)=>setTomlDirty(dirty),[]);
  const markTomlBusy=useCallback((busy:boolean)=>setTomlBusy(busy),[]);
  const [readyInspection,setReadyInspection]=useState<string|null>(null);
  const inspectionReady=useCallback((id:string)=>setReadyInspection(id),[]);
  const inspectionFailed=useCallback(()=>setReadyInspection(null),[]);
  const activeScenePath = scenes.some(scene => scene.path === scenePath) ? scenePath : scenes[0]?.path;
  const { inspection, busy: inspectionLoading, error: inspectionError, reload: restartInspection } = useSceneInspection(project.id, activeScenePath, view === "scene");
  const sceneLoading = sceneWorking || inspectionLoading;
  const [placement,setPlacement] = useState<ScenePlacement | null>(null);
  const [placementError,setPlacementError] = useState("");
  const [instanceId,setInstanceId] = useState<string | null>(initialInstance??null);
  const [inspectorReload,setInspectorReload] = useState(0);
  const currentPlacement = placement?.path === activeScenePath && (placement.space??"local")===space ? placement : null;
  const selectedInstance = currentPlacement?.instances.find(instance => instance.id === instanceId);
  const terrainEditHref=selectedInstance?.terrain&&currentPlacement?`/terrain?scene=${encodeURIComponent(currentPlacement.path)}&instance=${encodeURIComponent(selectedInstance.id)}`:undefined;
  useEffect(() => {
    let active = true;
    if (activeScenePath) void fetch(`/api/scenes/placement?projectId=${encodeURIComponent(project.id)}&path=${encodeURIComponent(activeScenePath)}&space=${space}`,{cache:"no-store"}).then(async response => {
      const data = await response.json();
      if (!response.ok) throw new Error(data.error ?? "Scene instances unavailable.");
      if (active) { setPlacement(data); setPlacementError(""); }
    }).catch(error => { if (active) { setPlacement(null); setPlacementError(error.message); } });
    return () => { active = false; };
  },[project,activeScenePath,inspectorReload,space]);
  // External (MCP) edits: refresh when safe; never discard drafts or interrupt a gesture.
  const [externalNotice,setExternalNotice]=useState("");
  const externalState=useRef({busy:false,dirty:false,scene:activeScenePath,view});
  externalState.current={busy:sceneLoading||tomlBusy||gizmoBusy,dirty:tomlDirty,scene:activeScenePath,view};
  useEffect(()=>{
    const events=new EventSource(`/api/project-events?projectId=${encodeURIComponent(project.id)}`);
    events.addEventListener("changed",message=>{
      let change:{summary:string;scenes:string[];live:boolean};
      try{change=JSON.parse((message as MessageEvent<string>).data);}catch{return;}
      const current=externalState.current;
      router.refresh();
      if(current.busy||current.dirty){setExternalNotice(`AI changed the project (${change.summary}). Save or discard your edit, then reload.`);return;}
      setInspectorReload(value=>value+1);setTomlEpoch(value=>value+1);
      if(current.view==="scene"&&current.scene&&change.scenes.includes(current.scene)&&!change.live)restartInspection();
      setExternalNotice(`AI: ${change.summary}`);
    });
    return ()=>events.close();
  },[project.id,router,restartInspection]);
  useEffect(()=>{if(!externalNotice)return;const timer=setTimeout(()=>setExternalNotice(""),8000);return ()=>clearTimeout(timer);},[externalNotice]);
  const selected = project.files.find((file) => file.path === selectedPath) ?? project.files[0];
  const selectedNoise=!selectedInstance&&selected.path.endsWith(".noise.toml");
  let selectedCamera:CameraSettings|null=null;
  if(!selectedInstance&&selected.content&&selected.path.endsWith(".entity.toml")){try{selectedCamera=cameraAssetSettings(selected.content);}catch{/* Invalid camera sources remain editable through TOML. */}}
  const tomlPath=selectedInstance?(tomlScope==="asset"?selectedInstance.asset:activeScenePath):selected.path;
  const tomlEditable=Boolean(tomlPath&&/\.(entity|scene|noise|tween)\.toml$/.test(tomlPath));
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return needle
      ? project.files.filter((file) => `${file.displayName??""} ${file.name} ${file.path} ${file.kind}`.toLowerCase().includes(needle))
      : project.files;
  }, [project.files, query]);
  const fileTree = useMemo(() => buildFileTree(filtered), [filtered]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const selectFile = (file: ProjectFile, nextView?: View) => {
    if (!canNavigate()) return;
    setInstanceId(null);
    setSelectedPath(file.path);
    if (nextView) setView(nextView);
    else setView(fileView(file));
  };

  function canNavigate(){
    if(sceneLoading||tomlBusy||gizmoBusy)return false;
    if(tomlDirty){if(!window.confirm("Discard unsaved asset edits?"))return false;setTomlDirty(false);setTomlEpoch(value=>value+1);}
    return true;
  }
  function selectInstance(instance:SceneInstance){
    if(!canNavigate())return false;setInstanceId(instance.id);setSelectedPath(instance.asset);return true;
  }

  async function renameInstance(id:string,name:string,revision:string){
    if(!canNavigate()||!currentPlacement)throw new Error("Wait for the current edit to finish.");
    setSceneLoading(true);
    try{
      const response=await fetch("/api/scenes/placement",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,path:currentPlacement.path,id,name,revision,action:"rename",space})});
      const data=await response.json();if(!response.ok)throw new Error(data.error??"Rename failed.");
      setPlacement(data);router.refresh();
    }finally{setSceneLoading(false);}
  }

  async function deleteSceneInstance(id:string,revision:string):Promise<boolean>{
    if(!canNavigate()||!currentPlacement)return false;
    setSceneLoading(true);setSceneError("");
    try{
      const response=await fetch("/api/scenes/placement",{method:"DELETE",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,path:currentPlacement.path,id,revision,space})});
      const data=await response.json();if(!response.ok)throw new Error(data.error??"Scene instance could not be deleted.");
      const updated=data as ScenePlacement;
      setPlacement(updated);
      if(!updated.instances.some(instance=>instance.id===instanceId)){setInstanceId(null);setSelectedPath(currentPlacement.path);}
      setView("scene");router.refresh();
      if(inspection?.scene===currentPlacement.path)await reloadInspection();
      else if(view==="scene")await loadScene(currentPlacement.path,false);
      return true;
    }finally{setSceneLoading(false);}
  }

  async function deleteProjectAsset(assetPath:string):Promise<boolean>{
    if(assetPath===activeScenePath)throw new Error("Load a different scene before deleting the open scene.");
    if(!canNavigate())return false;
    setSceneLoading(true);setSceneError("");
    try{
      const response=await fetch("/api/assets",{method:"DELETE",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,path:assetPath})});
      const result=await response.json();if(!response.ok)throw new Error(result.error??"Project asset could not be deleted.");
      if(selected.path===assetPath){setInstanceId(null);setSelectedPath(project.files.find(file=>file.path!==assetPath)?.path??"");}
      router.refresh();
      return true;
    }finally{setSceneLoading(false);}
  }

  async function reparentInstance(id:string,parent:string|null,revision=currentPlacement?.revision){
    if(!canNavigate()||!currentPlacement)return;
    setSceneLoading(true);setSceneError("");
    try{
      const response=await fetch("/api/scenes/placement",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,path:currentPlacement.path,id,parent,revision,action:"reparent",space})});
      const data=await response.json();if(!response.ok)throw new Error(data.error??"Reparent failed.");
      setPlacement(data);router.refresh();
      if(inspection&&inspection.scene===currentPlacement.path){const applied=await fetch("/api/inspection",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,id:inspection.id,command:{op:"reparent",id,parent}})});if(!applied.ok){const error=await applied.json();throw new Error(`Parent saved, but live update failed: ${error.error}. Reload the scene.`);}}
      else if(view==="scene")await loadScene(currentPlacement.path,false);
    }finally{setSceneLoading(false);}
  }

  async function loadScene(requestedPath = activeScenePath, selectSceneFile = true) {
    if (!requestedPath) return;
    setScenePath(requestedPath);
    if (selectSceneFile && !selectedInstance) setSelectedPath(requestedPath);
    await reloadInspection();
  }

  async function reloadInspection(){
    setSceneError("");
    setView("scene");
    restartInspection();
  }

  async function applyPlacement(value: TransformValues | null) {
    if (!currentPlacement || !selectedInstance) throw new Error("Select a scene instance first.");
    setSceneLoading(true);
    try {
      const response = await fetch("/api/scenes/placement",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,path:currentPlacement.path,id:selectedInstance.id,revision:currentPlacement.revision,transform:value,space,positionOnly:Boolean(selectedInstance.globalTranslationOnly)})});
      const data = await response.json();
      if (!response.ok) throw new Error(data.error ?? "Placement could not be saved.");
      setPlacement(data);
      router.refresh();
      if(inspection && inspection.scene===currentPlacement.path){
        const updated=(data as ScenePlacement).instances.find(item=>item.id===selectedInstance.id)!;
        const applied=await fetch("/api/inspection",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,id:inspection.id,command:{op:"transform",id:selectedInstance.id,...updated.transform}})});
        if(!applied.ok){const result=await applied.json();throw new Error(`Placement saved, but inspection update failed: ${result.error}. Reload the scene.`);}
        setView("scene");
      }else await loadScene(currentPlacement.path,false);
    } finally { setSceneLoading(false); }
  }

  async function applyPhysics(physics:PhysicsSettings|null) {
    if(!currentPlacement||!selectedInstance)throw new Error("Select a scene instance first.");
    setSceneLoading(true);
    try{
      const response=await fetch("/api/scenes/placement",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,path:currentPlacement.path,id:selectedInstance.id,revision:currentPlacement.revision,action:"physics",physics,space})});
      const data=await response.json();if(!response.ok)throw new Error(data.error??"Physics could not be saved.");
      setPlacement(data);router.refresh();
      if(inspection&&inspection.scene===currentPlacement.path)await reloadInspection();
      else await loadScene(currentPlacement.path,false);
    }finally{setSceneLoading(false);}
  }

  async function applyCamera(camera:CameraSettings|null) {
    if(!currentPlacement||!selectedInstance?.camera)throw new Error("Select a camera instance first.");
    setSceneLoading(true);
    try{
      const response=await fetch("/api/scenes/placement",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,path:currentPlacement.path,id:selectedInstance.id,revision:currentPlacement.revision,action:"camera",camera,space})});
      const data=await response.json();if(!response.ok)throw new Error(data.error??"Camera could not be saved.");
      setPlacement(data);router.refresh();if(camera===null)setTomlEpoch(value=>value+1);
      const effective=(data as ScenePlacement).instances.find(instance=>instance.id===selectedInstance.id)?.camera;
      if(inspection&&inspection.scene===currentPlacement.path&&effective){
        const applied=await fetch("/api/inspection",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,id:inspection.id,command:{op:"camera",id:selectedInstance.id,camera:effective}})});
        if(!applied.ok){restartInspection();throw new Error("Camera saved; restarting inspection after a live update failure.");}
      }
    }finally{setSceneLoading(false);}
  }

  async function applyCameraAsset(camera:CameraSettings|null){
    if(!camera||!selected.content)throw new Error("Camera settings are required.");
    const read=await fetch(`/api/project-toml?projectId=${encodeURIComponent(project.id)}&path=${encodeURIComponent(selected.path)}`,{cache:"no-store"}),document=await read.json();
    if(!read.ok)throw new Error(document.error??"Camera asset could not be read.");
    if(document.content!==selected.content)throw new Error("Camera asset changed externally. Reload before saving.");
    const response=await fetch("/api/project-toml",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,path:selected.path,revision:document.revision,content:replaceCameraAsset(document.content,camera)})});
    const result=await response.json();if(!response.ok)throw new Error(result.error??"Camera asset could not be saved.");router.refresh();
  }

  async function applyMaterial(material:MaterialSettings|null) {
    if(!currentPlacement||!selectedInstance)throw new Error("Select a renderable scene instance first.");
    setSceneLoading(true);
    try {
      const response=await fetch("/api/scenes/placement",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,path:currentPlacement.path,id:selectedInstance.id,revision:currentPlacement.revision,action:"material",material,space})});
      const data=await response.json();if(!response.ok)throw new Error(data.error??"Material could not be saved.");
      markTomlDirty(false);setPlacement(data);router.refresh();
      if(inspection&&inspection.scene===currentPlacement.path)await reloadInspection();
      else await loadScene(currentPlacement.path,false);
    }finally{setSceneLoading(false);}
  }

  async function persistGizmo(id:string,revision:string,transform:TransformValues){
    if(!currentPlacement||!inspection||inspection.scene!==currentPlacement.path)throw new Error("Inspection no longer matches the selected scene.");
    setSceneLoading(true);
    try{
      const response=await fetch("/api/scenes/placement",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId:project.id,path:currentPlacement.path,id,revision,transform,space:"local"})});
      if(!response.ok){const data=await response.json();throw new Error(data.error??"Gizmo placement could not be saved.");}
      setPlacement(await response.json());
      router.refresh();setInspectorReload(value=>value+1);
    }finally{setSceneLoading(false);}
  }

  return (
    <main className="studio-shell">
      <header className="topbar">
        <div className="brand"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></div>
        <div className="project-pill"><span>◫</span>{project.name}<span className="muted">LOCAL</span></div>
        <div className="scene-selector"><label htmlFor="scene-picker">Scene viewer</label><select id="scene-picker" value={scenes.some((scene) => scene.path === scenePath) ? scenePath : scenes[0]?.path ?? ""} onChange={(event) => { if(canNavigate()){setScenePath(event.target.value); setInstanceId(null);} }} disabled={!scenes.length || sceneLoading || tomlBusy || gizmoBusy}>{!scenes.length && <option value="">No scenes</option>}{scenes.map((scene) => <option key={scene.path} value={scene.path}>{scene.name}</option>)}</select><button onClick={() => {if(canNavigate())void loadScene();}} disabled={!scenes.length || sceneLoading || tomlBusy || gizmoBusy}>{sceneLoading ? "Loading…" : "Load"}</button><NewSceneModal projectId={project.id} disabled={sceneLoading || tomlBusy || gizmoBusy} onBeforeOpen={canNavigate} onCreated={(path) => { setScenePath(path); setSelectedPath(path); setInstanceId(null); setSceneError(""); setView("scene"); router.refresh(); }} /></div>
        <div className="space-switch" role="group" aria-label="Transform space">{(["local","global"] as const).map(mode=><button key={mode} aria-pressed={space===mode} disabled={sceneLoading||tomlBusy||gizmoBusy} onClick={()=>{if(mode!==space&&canNavigate())setSpace(mode);}}>{capitalize(mode)}</button>)}</div>
        <div className="top-actions"><ProjectRunControl projectId={project.id} scenePath={activeScenePath} disabled={sceneLoading || tomlBusy || gizmoBusy || tomlDirty} /></div>
      <FalBalance/></header>

      <EditorRail scene={activeScenePath??project.entryScene} view={view} onView={setView} beforeNavigate={canNavigate} onReload={()=>router.refresh()}/>

      <section className="workspace">
        <div className="left-column scene-first-column">
          <aside className="panel project-panel scene-project-panel">
            <div className="panel-head"><h2>Scene graph</h2><button className="square-button" title="Reload project from disk" onClick={() => {if(canNavigate()){router.refresh();setInspectorReload(value=>value+1);}}}>↻</button></div>
            <label className="search"><span>⌕</span><input ref={searchRef} value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Find…" /></label>
            <div className="scene-project-content">
              <div className="scene-graph-region">
              {placementError&&<p className="graph-error" role="alert">{placementError}</p>}
              {currentPlacement?<SceneGraph key={currentPlacement.path} placement={currentPlacement} selected={instanceId} disabled={sceneLoading||tomlBusy||gizmoBusy} query={query} onSelect={selectInstance} onReparent={reparentInstance} onRename={renameInstance} onDelete={deleteSceneInstance}/>:!placementError&&<p className="empty">Loading scene graph…</p>}
              </div>
              <details className="project-files" open><summary>Project files</summary><div className="asset-tree project-tree">
              {filtered.length > 0
                ? <details open className="tree-branch tree-project"><summary>{project.name}</summary><TreeContents node={fileTree} selected={selected.path} onSelect={selectFile} onDelete={deleteProjectAsset} disabled={sceneLoading||tomlBusy||gizmoBusy} assetRoots={project.assetRoots} /></details>
                : <p className="empty">No match for “{query}”.</p>}
              </div></details>
            </div>
            <AddAssetModal onBeforeOpen={canNavigate} projectId={project.id} scenePath={scenes.some((scene) => scene.path === scenePath) ? scenePath : scenes[0]?.path} onCreated={(path, warning) => {
              setSceneError(warning ?? "");
              setQuery("");
              setSelectedPath(path);
              setInstanceId(null);
              setView(path.endsWith(".noise.toml") ? "preview" : "source");
              router.refresh();
            }} />
          </aside>
        </div>

        <section className="canvas-panel">
          <div className="canvas-toolbar">
            {selected.path.endsWith(".entity.toml")&&selected.content&&/^kind\s*=\s*["']particle-effect["']/m.test(selected.content)&&<Link className="quiet-button" href={`/particles?asset=${encodeURIComponent(selected.path)}&scene=${encodeURIComponent(activeScenePath??project.entryScene)}`} onClick={event=>{if(!canNavigate())event.preventDefault();}}>Open Particle Generator ↗</Link>}
            {selected.kind==="tween"&&<Link className="quiet-button" href={`/tweens?asset=${encodeURIComponent(selected.path)}&scene=${encodeURIComponent(activeScenePath??project.entryScene)}`} onClick={event=>{if(!canNavigate())event.preventDefault();}}>Open Tween Editor ↗</Link>}
            <div className="view-switch">{(["scene", ...((selectedNoise || selected.kind==="model" || selected.name==="object.asset.json") ? ["preview" as const] : []), "source","toml"] as View[]).map((item) => <button disabled={item==="toml"&&!tomlEditable} key={item} className={view === item ? "selected" : ""} onClick={() => {if(item!==view&&canNavigate())setView(item);}}>{item==="toml"?"TOML":capitalize(item)}</button>)}</div>
          </div>
          {(sceneError || inspectionError) && <p className="scene-load-error" role="alert">{sceneError || inspectionError}</p>}
          {externalNotice && <p className="scene-external-change" role="status">{externalNotice}</p>}
          {inspection && <section className="scene-canvas-slot" aria-label="Rust-rendered scene preview">
            <InspectionViewport key={inspection.id} frame={currentPlacement?.frame} projectId={project.id} sessionId={inspection.id} sceneName={scenes.find(scene=>scene.path===inspection.scene)?.name??inspection.scene} selectedId={selectedInstance?.id??null} revision={currentPlacement?.revision??""} space={space} disabled={sceneLoading||tomlBusy||tomlDirty||!currentPlacement||activeScenePath!==inspection.scene} onBusy={markGizmoBusy} onCommit={persistGizmo} onSelect={id=>{if(!canNavigate())return;if(id){const instance=currentPlacement?.instances.find(item=>item.id===id);if(instance)selectInstance(instance);}else setInstanceId(null);}} onReady={inspectionReady} onFailure={inspectionFailed} />
          </section>}
          {view === "scene" && !inspection && <div className="scene-empty">{inspectionLoading ? "Starting scene inspection…" : inspectionError ? "Scene inspection could not start. Use Load to retry." : "Choose a scene to inspect it."}</div>}
          {view === "preview" && (selectedNoise?<div className="noise-overview" ref={setNoisePreviewTarget}/>:<AssetPreview project={project} file={selected} />)}
          {view === "source" && (/\.(entity|tween)\.toml$/.test(selected.path) ? <TomlEditor key={`source:${selected.path}:${tomlEpoch}`} projectId={project.id} path={selected.path} onDirty={markTomlDirty} onBusy={markTomlBusy} onSaved={()=>{router.refresh();setInspectorReload(value=>value+1);}}/> : <SourceView file={selected} />)}
          {view==="toml"&&tomlEditable&&tomlPath&&<>
            {selectedInstance&&<label className="toml-document-picker">TOML document<select aria-label="TOML document" value={tomlScope} disabled={tomlBusy} onChange={event=>{if(canNavigate())setTomlScope(event.target.value);}}><option value="scene">Scene overrides</option><option value="asset">Entity asset</option></select></label>}
            <TomlEditor key={`${tomlPath}:${instanceId}:${tomlEpoch}`} projectId={project.id} path={tomlPath} onDirty={markTomlDirty} onBusy={markTomlBusy} onSaved={()=>{router.refresh();setInspectorReload(value=>value+1);}}/>
          </>}
          <ProjectChat
            projectId={project.id}
            projectName={project.name}
            scene={activeScenePath ? { path: activeScenePath, name: scenes.find(scene=>scene.path===activeScenePath)?.name??"Scene" } : undefined}
            object={selectedInstance && activeScenePath ? { scope: `entity:${activeScenePath}#${selectedInstance.id}`, name: selectedInstance.name??selectedInstance.id } : ["entity","model"].includes(selected.kind) || selected.name==="object.asset.json" ? { scope: `file:${selected.path}`, name: selected.displayName??selected.name.replace(/\.(entity\.toml|glb|gltf)$/, "") } : undefined}
            aiSettings={aiSettings}
            disabled={sceneLoading || inspectorBusy || tomlDirty || gizmoBusy}
            onBusy={setChatBusy}
            onApplied={()=>{router.refresh();setInspectorReload(value=>value+1);setTomlEpoch(value=>value+1);if(view==="scene")restartInspection();}}
          />
        </section>

        <aside className="right-column">
          <section className="panel details-panel">
            {selectedNoise&&view!=="toml"?<>
              <div className="panel-head"><h2>Noise filter</h2><span className="kind-badge">FILTER</span></div>
              <NoiseInspector key={`${selected.path}:${tomlEpoch}`} projectId={project.id} path={selected.path} previewTarget={noisePreviewTarget} onDirty={markTomlDirty} onBusy={markTomlBusy} onSaved={()=>router.refresh()}/>
            </>:selectedInstance && currentPlacement ? <>
              {selectedInstance?.particles&&<div className="terrain-edit-action"><Link className="quiet-button" href={`/particles?asset=${encodeURIComponent(selectedInstance.asset)}&scene=${encodeURIComponent(activeScenePath??project.entryScene)}`} onNavigate={event=>{if(!canNavigate())event.preventDefault();}}>Edit particle effect ↗</Link><small>Effect preview is frozen at 2 s while editing. Run to animate.</small></div>}
              {selectedInstance?.structure&&<div className="terrain-edit-action"><Link className="quiet-button" href={`/object-builder?asset=${encodeURIComponent(selectedInstance.asset)}&scene=${encodeURIComponent(activeScenePath??project.entryScene)}&instance=${encodeURIComponent(selectedInstance.id)}`} onNavigate={event=>{if(!canNavigate())event.preventDefault();}}>Edit object ↗</Link><small>Open its local hierarchy and editable mesh.</small></div>}
              {terrainEditHref&&<div className="terrain-edit-action"><Link className="quiet-button" href={terrainEditHref} onNavigate={event=>{event.preventDefault();if(canNavigate())router.push(`${terrainEditHref}&open=${crypto.randomUUID()}`);}}>Edit terrain ↗</Link><small>Open this terrain in Terrain Builder.</small></div>}
              <div className="panel-head"><h2>Transform</h2><span className="kind-badge">INSTANCE</span></div>
              <TransformInspector key={`${currentPlacement.path}:${selectedInstance.id}:${inspectorReload}:${space}:${tomlEpoch}`} instance={selectedInstance} space={space} instances={currentPlacement.instances} onReparent={parent=>reparentInstance(selectedInstance.id,parent)} live={Boolean(inspection && inspection.scene===currentPlacement.path)} disabled={sceneLoading || tomlBusy || gizmoBusy || Boolean(inspection && readyInspection!==inspection.id)} onApply={applyPlacement} onDirty={markTomlDirty} onBusy={markTomlBusy} onReload={() => setInspectorReload(value => value + 1)} />
              {selectedInstance.raymarched?<p className="transform-note">Experimental ray-marched terrain is visual only. Overhang/cave collision is not implemented.</p>:<PhysicsInspector key={`physics:${currentPlacement.path}:${selectedInstance.id}:${tomlEpoch}`} instance={selectedInstance} disabled={sceneLoading || tomlBusy || gizmoBusy} onApply={applyPhysics} onDirty={markTomlDirty} onBusy={markTomlBusy}/>}
              {selectedInstance.camera&&<CameraInspector key={`camera:${currentPlacement.path}:${selectedInstance.id}:${tomlEpoch}`} settings={selectedInstance.camera} overridden={selectedInstance.cameraOverridden} disabled={sceneLoading||tomlBusy||gizmoBusy} onApply={applyCamera} onDirty={markTomlDirty} onBusy={markTomlBusy}/>}
              {selectedInstance.renderable&&!selectedInstance.raymarched&&<MaterialInspector textureAssets={project.files.filter(file=>/\.(png|jpe?g)$/i.test(file.path)&&project.assetRoots.some(root=>file.path.startsWith(`${root}/`)))} projectId={project.id} key={`material:${currentPlacement.path}:${selectedInstance.id}:${tomlEpoch}`} instance={selectedInstance} noiseAssets={project.files.filter(file=>file.path.endsWith(".noise.toml")&&project.assetRoots.some(root=>file.path.startsWith(`${root}/`)))} disabled={sceneLoading||tomlBusy||gizmoBusy} onApply={applyMaterial} onDirty={markTomlDirty} onBusy={markTomlBusy}/>}
            </> : selectedCamera ? <>
              <div className="panel-head"><h2>Camera asset</h2><span className="kind-badge">DEFAULTS</span></div>
              <CameraInspector key={`camera-asset:${selected.path}:${tomlEpoch}`} settings={selectedCamera} disabled={sceneLoading||tomlBusy||gizmoBusy} onApply={applyCameraAsset} onDirty={markTomlDirty} onBusy={markTomlBusy}/>
            </> : <>
            <div className="panel-head"><div><span className="eyebrow">SELECTED</span><h2>{selected.displayName??selected.name}</h2></div><span className="kind-badge">{selected.kind}</span></div>
            </>}
          </section>
          <StatementsPanel key={selected.path} projectId={project.id} subject={`file:${selected.path}`} subjectLabel={selectedInstance?.name??selected.displayName??selected.name} aiConfigured={aiSettings.configured} />
        </aside>

      </section>
    </main>
  );
}

function AssetPreview({ project, file }: { project: ProjectSnapshot; file: ProjectFile }) {
  let modelPath = file.path.endsWith(".glb") ? file.path : null;
  if (file.name === "object.asset.json" && file.content) {
    try { const descriptor = JSON.parse(file.content); if (typeof descriptor.model === "string") modelPath = descriptor.model; } catch { /* Invalid descriptors still have a source view. */ }
  }
  if (modelPath) return <div className="model-overview"><ModelPreview key={modelPath} name={file.name} src={`/api/project-asset?projectId=${encodeURIComponent(project.id)}&path=${encodeURIComponent(modelPath)}`} /></div>;
  return <SourceView file={file} />;
}

function SourceView({ file }: { file: ProjectFile }) {
  return <div className="source-view"><div className="source-head"><span>{file.path}</span><b>{file.content ? `${file.content.split("\n").length} lines` : "binary or unavailable"}</b></div>{file.content ? <pre><code>{file.content}</code></pre> : <div className="connection-empty"><b>No text preview</b><span>The IDE keeps the reference visible without claiming it can decode this asset type.</span></div>}</div>;
}

type FileTree = { folders: Map<string, FileTree>; files: ProjectFile[] };

function buildFileTree(files: ProjectFile[]): FileTree {
  const root: FileTree = { folders: new Map(), files: [] };
  for (const file of files) {
    const parts = file.path.split("/");
    let node = root;
    for (const folder of parts.slice(0, -1)) {
      const existing = node.folders.get(folder);
      if (existing) {
        node = existing;
      } else {
        const created: FileTree = { folders: new Map(), files: [] };
        node.folders.set(folder, created);
        node = created;
      }
    }
    node.files.push(file);
  }
  return root;
}

function TreeContents({ node, selected, onSelect, onDelete, disabled, assetRoots }: { node: FileTree; selected: string; onSelect: (file: ProjectFile) => void; onDelete:(path:string)=>Promise<boolean>; disabled:boolean; assetRoots:string[] }) {
  const [error,setError]=useState("");
  async function deleteAsset(file:ProjectFile){
    if(disabled)return;
    const name=file.displayName??file.name;
    if(!window.confirm(`Permanently delete "${name}" from the project? Referenced assets cannot be deleted.`))return;
    setError("");
    try{await onDelete(file.path);}
    catch(error){setError(error instanceof Error?error.message:"Asset deletion failed.");}
  }
  const folders = [...node.folders.entries()].sort(([left], [right]) => left.localeCompare(right));
  const files = [...node.files].sort((left, right) => (left.displayName??left.name).localeCompare(right.displayName??right.name));
  return <div className="tree-children">
    {folders.map(([name, child]) => <details open className="tree-branch" key={name}><summary>{name}</summary><TreeContents node={child} selected={selected} onSelect={onSelect} onDelete={onDelete} disabled={disabled} assetRoots={assetRoots} /></details>)}
    {error&&<p className="graph-error" role="alert">{error}</p>}
    {files.map((file) => <div className="tree-file-row" key={file.path}><button className={selected === file.path ? "tree-file selected" : "tree-file"} onClick={() => onSelect(file)} title={file.path}>{file.displayName??file.name}</button>{assetRoots.some(root=>file.path.startsWith(`${root}/`))&&<button type="button" className="tree-file-delete" aria-label={`Delete ${file.displayName??file.name}`} title="Delete project asset" disabled={disabled} onClick={()=>void deleteAsset(file)}>×</button>}</div>)}
  </div>;
}

function capitalize(value: string) { return `${value[0].toUpperCase()}${value.slice(1)}`; }
