"use client";
/* eslint-disable @next/next/no-img-element -- local project images and cached generation artifacts */
import {FalBalance} from "@/components/fal-balance";
import {FalModelSelector} from "./fal-model-selector";
import {InfoTip} from "./info-tip";
import {DEFAULT_FAL_MODELS} from "@/lib/fal-model-types";
import Link from "next/link";
import {useEffect,useRef,useState,type DragEvent,type FormEvent} from "react";
import {SpriteActionsDialog,type SpriteActionSource} from "./sprite-actions-dialog";
import {SpriteFamilies} from "./sprite-families";
import {ProjectVideoLibrary} from "./project-video-library";
import {type ImageLibrarySettings,type LibraryImage} from "@/lib/image-library";
import {textureRequest,type TextureJob,type TextureRequest} from "@/lib/texture-job-types";

const candidateType="application/x-emergent-image-candidate";
async function json<T>(response:Response):Promise<T>{const data=await response.json();if(!response.ok)throw new Error(data.error??"Image request failed.");return data;}

export function ImageGenerator({projectId,projectName,initialImages,initialJobs,configured,initialKind="image"}:{projectId:string;projectName:string;initialImages:LibraryImage[];initialJobs:TextureJob[];configured:boolean;initialKind?:"image"|"sprite"|"skybox"}){
 const [model,setModel]=useState<string>(DEFAULT_FAL_MODELS.image.id),[modelReady,setModelReady]=useState(true);
 const [images,setImages]=useState(initialImages),[jobs,setJobs]=useState(initialJobs),[selectedId,setSelectedId]=useState(initialJobs[0]?.id??"");
 const [name,setName]=useState(""),[prompt,setPrompt]=useState(""),[busy,setBusy]=useState(false),[saving,setSaving]=useState(false),[dragging,setDragging]=useState(false),[error,setError]=useState(""),[status,setStatus]=useState("");
 const [preview,setPreview]=useState<{name:string;src:string}|null>(null),dialog=useRef<HTMLDialogElement>(null),file=useRef<HTMLInputElement>(null);
 const submitting=useRef(false),adding=useRef(false),pending=useRef<{id:string;fingerprint:string;input:TextureRequest}|null>(null);
 const [spriteActions,setSpriteActions]=useState<(SpriteActionSource&{candidate?:{jobId:string;resultId:string}})|null>(null),[videoRefresh,setVideoRefresh]=useState(0),videoSubmitting=useRef(false);
 const [spritePending,setSpritePending]=useState<{source:string;family?:string;prompt:string}|null>(null);
 async function generateVideo(sprite:SpriteActionSource,motion:string,spriteSettings?:ImageLibrarySettings,animationModel?:string,animationSeed=sprite.seed){
  if(videoSubmitting.current)return;videoSubmitting.current=true;setError("");setStatus(spriteSettings?"Submitting sprite animation to fal…":"Submitting video to fal…");
  if(spriteSettings){setLibraryType("sprite");setSpritePending({source:sprite.source,family:images.find(image=>image.path===sprite.source)?.family,prompt:motion});}
  try{let source=sprite.source;
   if(spriteSettings){if(source.startsWith("texture-job:")){const [,jobId,resultId]=source.split(":"),job=jobs.find(j=>j.id===jobId);if(!job)throw new Error("Sprite candidate is no longer available.");const saved=await json<TextureJob>(await fetch(`/api/texture-jobs/${jobId}`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,action:"save",resultId,name:job.input.name})}));update(saved);source=saved.outputs.find(output=>output.id===resultId)?.savedPath??"";if(!source)throw new Error("Sprite could not be saved.");}
    await json(await fetch("/api/textures",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path:source,settings:spriteSettings})}));}
   await json(await fetch("/api/particle-media",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,id:crypto.randomUUID(),input:{stage:"animate",source,prompt:motion,seed:animationSeed,...(animationModel?{model:animationModel}:{}),...(spriteSettings?{output:"sprite"}:{})}})}));await reloadLibrary();setVideoRefresh(n=>n+1);setLibraryType(spriteSettings?"sprite":"video");setStatus(spriteSettings?"Sprite animation queued. Its atlas will appear beside the static version.":"Video request added to the Video library.");
  }catch(e){setError(e instanceof Error?e.message:"Unable to generate animation.");setStatus("");}finally{videoSubmitting.current=false;setSpritePending(null);}
 }
 const [libraryType,setLibraryType]=useState<"image"|"sprite"|"skybox"|"video">(initialKind),[generationKind,setGenerationKind]=useState<"image"|"sprite"|"skybox">(initialKind);
 const visibleImages=images.filter(image=>(image.kind??"image")===libraryType);
 async function configureImage(sprite:SpriteActionSource&{candidate?:{jobId:string;resultId:string}},settings:ImageLibrarySettings){
  if(adding.current)return;adding.current=true;setSaving(true);setError("");setStatus("");
  try{let imagePath=sprite.source;
   if(sprite.candidate){const job=jobs.find(j=>j.id===sprite.candidate!.jobId);if(!job)throw new Error("Image request is no longer available.");const saved=await json<TextureJob>(await fetch(`/api/texture-jobs/${job.id}`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,action:"save",resultId:sprite.candidate.resultId,name:job.input.name})}));update(saved);imagePath=saved.outputs.find(output=>output.id===sprite.candidate!.resultId)?.savedPath??"";if(!imagePath)throw new Error("Image could not be added to the library.");}
   await json(await fetch("/api/textures",{method:"PUT",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path:imagePath,settings})}));await reloadLibrary();setLibraryType(settings.kind);setStatus("Image label and sprite-sheet settings saved.");
  }catch(e){setError(e instanceof Error?e.message:"Could not save image settings.");}finally{adding.current=false;setSaving(false);}
 }
 const selected=jobs.find(job=>job.id===selectedId);
 const libraryUrl=(image:LibraryImage)=>`/api/project-asset?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(image.path)}&v=${encodeURIComponent(image.modifiedAt)}`;
 const artifact=(job:TextureJob,id:string)=>`/api/texture-jobs/${job.id}/artifact?projectId=${encodeURIComponent(projectId)}&result=${encodeURIComponent(id)}`;
 function update(job:TextureJob){setJobs(previous=>previous.some(item=>item.id===job.id&&item.updatedAt>job.updatedAt)?previous:[job,...previous.filter(item=>item.id!==job.id)].filter(item=>item.state!=="ready"||item.outputs.length>0).sort((a,b)=>b.createdAt.localeCompare(a.createdAt)));}
 async function reloadLibrary(){setImages(await json<LibraryImage[]>(await fetch(`/api/textures?projectId=${encodeURIComponent(projectId)}`,{cache:"no-store"})));}
 useEffect(()=>{if(preview)dialog.current?.showModal();else dialog.current?.close();},[preview]);
 useEffect(()=>{
  if(!selected||selected.error||!["submitting","queued","generating","downloading"].includes(selected.state))return;
  let active=true;const controller=new AbortController();
  const timer=setTimeout(()=>{void fetch(`/api/texture-jobs/${selected.id}`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,action:"refresh"}),signal:controller.signal}).then(response=>json<TextureJob>(response)).then(job=>{if(active)update(job);}).catch(error=>{if(active)setError(error.message);});},2500);
  return()=>{active=false;clearTimeout(timer);controller.abort();};
 },[projectId,selected]);
 async function generate(event:FormEvent){
  event.preventDefault();if(submitting.current||!modelReady)return;submitting.current=true;setBusy(true);setError("");setStatus("");
  try{
   const input=textureRequest({name,prompt,seed:crypto.getRandomValues(new Uint32Array(1))[0]&0x7fffffff,count:1,tileFriendly:false,kind:generationKind,...(model===DEFAULT_FAL_MODELS.image.id?{}:{model})});
   const fingerprint=JSON.stringify({name:input.name,prompt:input.prompt,kind:input.kind,model:input.model});
   // Keep the same request and seed after a lost response to avoid a second paid submission.
   if(!pending.current||pending.current.fingerprint!==fingerprint)pending.current={id:crypto.randomUUID(),fingerprint,input};
   const response=await fetch("/api/texture-jobs",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,id:pending.current.id,input:pending.current.input})});
   const job=await json<TextureJob>(response);update(job);setSelectedId(job.id);pending.current=null;
  }catch(error){setError(error instanceof Error?error.message:"Unable to generate image.");}finally{submitting.current=false;setBusy(false);}
 }
 async function refresh(){if(!selected)return;setBusy(true);setError("");try{update(await json<TextureJob>(await fetch(`/api/texture-jobs/${selected.id}`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,action:"refresh"})})));}catch(error){setError(error instanceof Error?error.message:"Unable to check generation.");}finally{setBusy(false);}}
 async function saveCandidate(jobId:string,resultId:string){
  if(adding.current)return;
  const job=jobs.find(item=>item.id===jobId),result=job?.outputs.find(item=>item.id===resultId);
  if(!job||job.state!=="ready"||!result){setError("Choose a completed generated image.");return;}
  adding.current=true;setSaving(true);setError("");setStatus("");
  try{const saved=await json<TextureJob>(await fetch(`/api/texture-jobs/${job.id}`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,action:"save",resultId,name:job.outputs.length===1?job.input.name:`${job.input.name.slice(0,60)} ${result.index+1}`})}));update(saved);await reloadLibrary();setLibraryType(saved.input.kind??"image");setStatus("Image added to the project library.");}
  catch(error){setError(error instanceof Error?error.message:"Unable to add image.");}finally{adding.current=false;setSaving(false);}
 }
 async function deleteCandidate(jobId:string,resultId:string){
  if(adding.current)return;adding.current=true;setSaving(true);setError("");setStatus("");
  try{const updated=await json<TextureJob>(await fetch(`/api/texture-jobs/${jobId}`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,action:"delete",resultId})}));update(updated);if(!updated.outputs.length)setSelectedId(current=>current===jobId?jobs.find(job=>job.id!==jobId)?.id??"":current);setStatus("Generated preview deleted. Any saved library copy was kept.");}
  catch(error){setError(error instanceof Error?error.message:"Unable to delete generated image.");}finally{adding.current=false;setSaving(false);}
 }
 async function deleteLibraryTile(relative:string){
  if(adding.current)return;adding.current=true;setSaving(true);setError("");setStatus("");
  try{await json(await fetch("/api/textures",{method:"DELETE",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path:relative})}));setImages(all=>all.filter(image=>image.path!==relative));setJobs(all=>all.map(job=>({...job,outputs:job.outputs.map(output=>output.savedPath===relative?{...output,savedPath:null}:output)})));setVideoRefresh(n=>n+1);setStatus("Image deleted from the project library.");}
  catch(error){setError(error instanceof Error?error.message:"Unable to delete library image.");}finally{adding.current=false;setSaving(false);}
 }
 async function upload(image:File){
  if(adding.current)return;adding.current=true;setSaving(true);setError("");setStatus("");
  try{if(image.size>16*1024*1024)throw new Error("Choose a PNG/JPEG of at most 16 MiB.");await json(await fetch(`/api/textures?projectId=${encodeURIComponent(projectId)}&name=${encodeURIComponent(image.name)}&kind=${libraryType==="video"?"image":libraryType}`,{method:"POST",headers:{"Content-Type":"application/octet-stream"},body:image}));await reloadLibrary();setStatus("Image imported into the project library.");}
  catch(error){setError(error instanceof Error?error.message:"Unable to import image.");}finally{adding.current=false;setSaving(false);if(file.current)file.current.value="";}
 }
 function drop(event:DragEvent){event.preventDefault();setDragging(false);if(saving)return;try{const value=JSON.parse(event.dataTransfer.getData(candidateType));if(typeof value.jobId!=="string"||typeof value.resultId!=="string")throw new Error();void saveCandidate(value.jobId,value.resultId);}catch{setError("Drag a generated result into the library, or use Import image for a local file.");}}
 return <main className="image-shell">
  <header className="settings-topbar"><Link className="brand" href="/"><span className="brand-mark">E</span><span>Emergent</span><b>IDE</b></Link><span className="settings-breadcrumb">{projectName} / <b>Image Generator</b></span><Link className="quiet-button" href="/textures">Texture Creator</Link><Link className="quiet-button" href="/">Project map ↗</Link><FalBalance/></header>
  <div className="image-heading"><div><span className="eyebrow">YOUR PROJECT · IMAGE LIBRARY</span><h1>Imagine it. Keep it. Build with it.</h1><p>Generate an image, then drag the result into your library.</p></div><span className={configured?"settings-state connected":"settings-state"}>{configured?"fal connected":"Connect fal in Settings"}</span></div>
  <div className="image-workspace">
   <section className={`panel image-library${dragging?" drag-over":""}`} aria-label="Project image library" onDragOver={event=>{if(Array.from(event.dataTransfer.types).includes(candidateType)&&!saving){event.preventDefault();event.dataTransfer.dropEffect="copy";setDragging(true);}}} onDragLeave={event=>{if(!event.currentTarget.contains(event.relatedTarget as Node|null))setDragging(false);}} onDrop={drop}>
    <div className="panel-head"><div><h2>Image library <span className="kind-badge">{images.length}</span></h2><p>Shared across this project’s tools.</p></div><div className="image-library-actions"><button className="quiet-button" disabled={saving} onClick={()=>{setError("");void reloadLibrary().catch(error=>setError(error.message));}}>Refresh</button><button className="quiet-button" disabled={saving} onClick={()=>file.current?.click()}>{saving?"Adding…":"Import image"}</button></div></div>
    <input ref={file} type="file" hidden accept=".png,.jpg,.jpeg,image/png,image/jpeg" aria-label="Import library image" onChange={event=>{const image=event.target.files?.[0];if(image)void upload(image);}}/>
    <div className="image-drop-hint">{dragging?"Drop to keep this image":"Drop generated images here to add them to your library"}</div>
    <div className="image-type-tabs" role="tablist" aria-label="Library media types">{(["image","sprite","video","skybox"] as const).map(type=><button type="button" className="quiet-button" role="tab" aria-selected={libraryType===type} key={type} onClick={()=>{setLibraryType(type);if(type!=="video")setGenerationKind(type);}}>{type==="image"?"Image":type==="sprite"?"Sprite":type==="skybox"?"Skybox":"Video"}</button>)}</div>{libraryType==="video"&&<ProjectVideoLibrary projectId={projectId} refreshKey={videoRefresh} onImagesChanged={()=>void reloadLibrary()} onShowSprites={()=>{void reloadLibrary();setLibraryType("sprite");}}/>}{libraryType!=="video"&&<p className="image-video-hint object-muted">Right-click a tile to change its type, configure sprites, generate video or delete.</p>}
    {libraryType==="sprite"&&<SpriteFamilies projectId={projectId} images={visibleImages} refreshKey={videoRefresh} submitting={spritePending} onChanged={()=>void reloadLibrary()} onOptions={image=>setSpriteActions({name:image.name,preview:libraryUrl(image),source:image.path,kind:"sprite",sprite:image.sprite,seed:crypto.getRandomValues(new Uint32Array(1))[0]&0x7fffffff})} onDelete={image=>void deleteLibraryTile(image.path)}/>}
    {(libraryType==="image"||libraryType==="skybox")&&(visibleImages.length?<div className="image-library-grid">{visibleImages.map(image=><button className="image-library-tile" key={image.path} title={image.path} onContextMenu={event=>{event.preventDefault();setSpriteActions({name:image.name,preview:libraryUrl(image),source:image.path,kind:image.kind??"image",sprite:image.sprite,seed:crypto.getRandomValues(new Uint32Array(1))[0]&0x7fffffff});}} onClick={()=>setPreview({name:image.name,src:libraryUrl(image)})}><img src={libraryUrl(image)} alt="" loading="lazy"/><span>{image.name}</span><small className="image-kind-badge">{image.kind==="sprite"?`Sprite · ${image.sprite?.columns??1} × ${image.sprite?.rows??1}`:image.kind==="skybox"?"Skybox · Panorama":"Image"}</small></button>)}</div>:<div className="image-empty"><span>▧</span><h3>A home for your images.</h3><p>Import an image or generate your first one.</p></div>)}
    
   </section>
   <aside className="image-generation">
    <section className="panel image-generation-panel" aria-label="AI image generator">
     <header className="chat-header"><span className="chat-spark">✦</span><div><h2>Create with AI</h2><small>Text to image · fal</small></div><Link href="/settings#object-generation" className="provider-status">Settings ↗</Link></header>
     <form className="image-prompt" onSubmit={generate}><FalModelSelector projectId={projectId} kind="image" value={model} disabled={busy} onChange={(id,ready)=>{setModel(id);setModelReady(ready);}}/><label className="settings-field"><span>Generate as {generationKind==="skybox"&&<InfoTip label="Skybox generation help">Creates a 2:1 equirectangular panorama. Choose it on the Lighting page after adding it to the library. AI generation may leave a visible seam.</InfoTip>}</span><select aria-label="Generate as" value={generationKind} disabled={busy} onChange={event=>setGenerationKind(event.target.value as "image"|"sprite"|"skybox")}><option value="image">Image</option><option value="sprite">Sprite</option><option value="skybox">Skybox</option></select></label><label className="settings-field"><span>Image name</span><input aria-label="Image name" required maxLength={64} value={name} placeholder="Moonlit forest" onChange={event=>setName(event.target.value)} disabled={busy}/></label><div className="chat-composer"><textarea aria-label="Image prompt" required minLength={3} maxLength={2000} rows={4} value={prompt} placeholder="Describe the image you want to create…" disabled={busy} onChange={event=>setPrompt(event.target.value)} onKeyDown={event=>{if(event.key==="Enter"&&!event.shiftKey&&!event.nativeEvent.isComposing){event.preventDefault();event.currentTarget.form?.requestSubmit();}}}/><button type="submit" aria-label="Generate image" title="Generate image" disabled={!configured||busy||!modelReady||!name.trim()||prompt.trim().length<3}>{busy?"…":"↑"}</button></div><InfoTip label="Image generation help">Uses your fal account credit. Skyboxes use a 2:1 panoramic size. Other images use the model’s defaults. Enter to generate; Shift+Enter for a new line.</InfoTip>{!configured&&<Link href="/settings#object-generation">Connect fal to generate images →</Link>}</form>
     <div className="image-result-head"><h3>Generated preview</h3><span className="kind-badge" role="status">{selected?.state??"READY FOR A PROMPT"}</span></div>
     {selected?.state==="ready"?selected.outputs.map(result=><article className="image-candidate" key={result.id}><button className="image-candidate-preview" draggable={!saving&&!result.savedPath} aria-label={`Preview generated ${selected.input.name}`} onDragStart={event=>{if(result.savedPath||saving){event.preventDefault();return;}event.dataTransfer.setData(candidateType,JSON.stringify({jobId:selected.id,resultId:result.id}));event.dataTransfer.effectAllowed="copy";}} onContextMenu={event=>{event.preventDefault();const linked=images.find(image=>image.path===result.savedPath);setSpriteActions({name:selected.input.name,preview:artifact(selected,result.id),source:`texture-job:${selected.id}:${result.id}`,seed:selected.input.seed,kind:linked?.kind??selected.input.kind??"image",sprite:linked?.sprite,candidate:{jobId:selected.id,resultId:result.id}});}} onClick={()=>setPreview({name:selected.input.name,src:artifact(selected,result.id)})}><img src={artifact(selected,result.id)} alt={selected.input.name} draggable={false}/></button><div><b>{selected.input.name}</b><span className="image-candidate-actions"><button className="settings-save" disabled={saving||!!result.savedPath} onClick={()=>void saveCandidate(selected.id,result.id)}>{result.savedPath?"In library ✓":saving?"Adding…":"Add to library"}</button><button type="button" className="quiet-button" aria-label="Delete generated image" title="Delete this generated preview; saved library copies stay" disabled={busy||saving} onClick={()=>void deleteCandidate(selected.id,result.id)}>Delete</button></span></div><p className="object-muted">{result.savedPath?"Saved in project assets.":"Drag the preview into the library, or use Add to library."}</p></article>):<div className="image-empty image-result-empty"><span>✦</span><p>{selected?selected.error??"Your image is being prepared. You can return to this request later.":"Your next idea appears here."}</p></div>}
     {selected&&<div className="image-request"><p>{selected.input.prompt}</p><small className="object-muted">{selected.endpoint}</small>{selected.error&&selected.state==="ready"&&<p>{selected.error}</p>}<button className="quiet-button" disabled={busy||saving} onClick={()=>void refresh()}>Check request</button></div>}
    </section>
    <section className="panel image-history"><div className="panel-head"><h2>Recent generations</h2></div>{jobs.length?jobs.map(job=><button key={job.id} disabled={saving} className={selectedId===job.id?"object-history-item selected":"object-history-item"} onClick={()=>{setSelectedId(job.id);setError("");}}><span className="texture-history-icon">▧</span><span><b>{job.input.name}</b><small>{job.state}{job.outputs.some(result=>result.savedPath)?" · in library":""}</small></span></button>):<p className="object-muted">Your generation history stays with this project.</p>}</section>
   </aside>
  </div>
  <div className="image-feedback" aria-live="polite">{error&&<p role="alert" className="object-error">{error}</p>}{status&&<p role="status">{status}</p>}</div>
  {spriteActions&&<SpriteActionsDialog projectId={projectId} sprite={spriteActions} initialPrompt="Slow continuous motion, keeping the sprite centered and fully in frame" onClose={()=>setSpriteActions(null)} onConfigure={settings=>void configureImage(spriteActions,settings)} onGenerate={(motion,settings,animationModel,seed)=>void generateVideo(spriteActions,motion,settings,animationModel,seed)} deleteLabel={spriteActions.candidate?"Delete result":"Delete image"} onDelete={!saving&&!busy?()=>{if(spriteActions.candidate)void deleteCandidate(spriteActions.candidate.jobId,spriteActions.candidate.resultId);else void deleteLibraryTile(spriteActions.source);}:undefined}/>}
  <dialog ref={dialog} className="image-preview-modal" aria-labelledby="image-preview-title" onClose={()=>setPreview(null)} onClick={event=>{if(event.target===event.currentTarget)setPreview(null);}}>{preview&&<><header><h2 id="image-preview-title">{preview.name}</h2><button className="quiet-button" aria-label="Close image preview" autoFocus onClick={()=>setPreview(null)}>Close ×</button></header><img src={preview.src} alt={preview.name}/></>}</dialog>
 </main>;
}
