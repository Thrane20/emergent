/* eslint-disable @next/next/no-img-element -- Structure thumbnails are generated data URLs. */
"use client";
import {useEffect,useRef,useState} from "react";
import {createPortal} from "react-dom";
import * as THREE from "three";
import {OrbitControls} from "three/addons/controls/OrbitControls.js";
import {parseTrace,type BrainDocument,type BrainTrace} from "@/lib/brains";
import {structureDocument,structureWorldMatrices,triangulateFace,type StructureDocument} from "@/lib/structure";

export type PlaypenObject={path:string;name:string;thumbnail?:string};
type Stage={setTrace:(trace:BrainTrace|null)=>void;setObject:(document:StructureDocument|null)=>void;setSwitches:(switches:Switches)=>void;replay:()=>void;setPlaying:(playing:boolean)=>void};
type Switches={ground:boolean;light:boolean;spotlight:boolean};

/** One mesh per part, baked into the object's local space; shared by every agent clone. */
function objectTemplate(document:StructureDocument){
 const group=new THREE.Group(),world=structureWorldMatrices(document.parts),material=new THREE.MeshStandardMaterial({color:0x9fc7ad,flatShading:true,roughness:0.75,side:THREE.DoubleSide});
 for(const part of document.parts){
  const geometry=new THREE.BufferGeometry(),indices:number[]=[];
  for(let face=0;face<part.mesh.faces.length;face++)indices.push(...triangulateFace(part.mesh,face));
  geometry.setAttribute("position",new THREE.Float32BufferAttribute(part.mesh.vertices.flat(),3));geometry.setIndex(indices);geometry.computeVertexNormals();
  const mesh=new THREE.Mesh(geometry,material);mesh.applyMatrix4(world.get(part.id)!);mesh.castShadow=true;group.add(mesh);
 }
 // Sit the object on the ground: the engine moves its origin across y=0.
 const box=new THREE.Box3().setFromObject(group);if(!box.isEmpty())for(const mesh of group.children)mesh.position.y-=box.min.y;
 return group;
}

/** Ephemeral 3D playpen: runs the unsaved brain in the Rust preview and replays it with a library object as the agent body. */
export function BrainPlaypen({projectId,path,brain,disabled,objects,agentGroup,targetGroup}:{projectId:string;path?:string;brain:BrainDocument;disabled:boolean;objects:PlaypenObject[];agentGroup:string;targetGroup:string}){
 const host=useRef<HTMLDivElement>(null),stage=useRef<Stage|null>(null);
 // One object, no flock: the playpen shows this brain on a single body.
 const options={agents:1,agentGroup,targets:1,targetGroup,obstacles:0,targetSpeed:3,seconds:20,maxSpeed:3,seed:1};
 const [switches,setSwitches]=useState<Switches>({ground:true,light:true,spotlight:false});
 const [object,setObject]=useState<PlaypenObject|null>(null),[picking,setPicking]=useState(false);
 const [running,setRunning]=useState(false),[playing,setPlaying]=useState(true),[hasTrace,setHasTrace]=useState(false),[error,setError]=useState("");
 useEffect(()=>{stage.current?.setSwitches(switches);},[switches]);
 useEffect(()=>{stage.current?.setPlaying(playing);},[playing]);
 useEffect(()=>{
  const element=host.current!;let renderer:THREE.WebGLRenderer;
  // eslint-disable-next-line react-hooks/set-state-in-effect -- Report GPU creation failure after the canvas mounts.
  try{renderer=new THREE.WebGLRenderer({antialias:true});}catch{setError("WebGL is unavailable. Use the top-down Preview instead.");return;}
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.setClearColor(0x101713);renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.shadowMap.enabled=true;renderer.domElement.setAttribute("aria-label","Brain 3D playpen");element.prepend(renderer.domElement);
  const scene=new THREE.Scene(),camera=new THREE.PerspectiveCamera(45,1,0.05,2000);camera.position.set(14,12,18);
  const orbit=new OrbitControls(camera,renderer.domElement);orbit.enableDamping=true;
  scene.add(new THREE.HemisphereLight(0xe3f2e6,0x29352c,0.6));
  const sun=new THREE.DirectionalLight(0xfff2dc,2.4);sun.position.set(12,20,8);sun.castShadow=true;sun.shadow.mapSize.set(2048,2048);Object.assign(sun.shadow.camera,{left:-40,right:40,top:40,bottom:-40,far:80});scene.add(sun);
  const spot=new THREE.SpotLight(0xffffff,400,0,Math.PI/7,0.4,1.6);spot.position.set(0,18,0);spot.castShadow=true;scene.add(spot,spot.target);
  const ground=new THREE.Mesh(new THREE.PlaneGeometry(200,200),new THREE.MeshStandardMaterial({color:0x2a362e,roughness:1}));ground.rotation.x=-Math.PI/2;ground.receiveShadow=true;scene.add(ground);
  const grid=new THREE.GridHelper(80,40,0x50705a,0x293a2d);grid.position.y=0.002;scene.add(grid);
  const actors=new THREE.Group(),obstacles=new THREE.Group();scene.add(actors,obstacles);
  const cone=new THREE.ConeGeometry(0.35,1,10).rotateX(-Math.PI/2).translate(0,0.4,0),coneMaterial=new THREE.MeshStandardMaterial({color:0x9fe0b0});
  const sphere=new THREE.SphereGeometry(0.45,16,12).translate(0,0.45,0),sphereMaterial=new THREE.MeshStandardMaterial({color:0xf08a6c});
  const box=new THREE.BoxGeometry(1,1,1).translate(0,0.5,0),boxMaterial=new THREE.MeshStandardMaterial({color:0x56665b});
  let trace:BrainTrace|null=null,template:THREE.Group|null=null,clock=0,last=performance.now(),play=true,animation=0;
  const followed=new THREE.Vector3();
  function populate(){
   actors.clear();obstacles.clear();
   if(!trace){const body=template?template.clone():new THREE.Mesh(cone,coneMaterial);body.traverse(node=>{node.castShadow=true;});actors.add(body);return;}
   for(let i=0;i<trace.agents+trace.targets;i++){const body=i<trace.agents?template?template.clone():new THREE.Mesh(cone,coneMaterial):new THREE.Mesh(sphere,sphereMaterial);body.traverse(node=>{node.castShadow=true;});actors.add(body);}
   // The engine's obstacles are circles; a box of the same footprint reads as cover.
   for(const o of trace.obstacles){const mesh=new THREE.Mesh(box,boxMaterial);mesh.position.set(o.x,0,o.z);mesh.scale.set(o.radius*Math.SQRT2,1.6,o.radius*Math.SQRT2);mesh.castShadow=mesh.receiveShadow=true;obstacles.add(mesh);}
  }
  function pose(){
   if(!trace)return;const t=clock/trace.frameSeconds,index=Math.min(Math.floor(t),trace.frames.length-1),next=Math.min(index+1,trace.frames.length-1),blend=t-index;
   const a=trace.frames[index],b=trace.frames[next];
   actors.children.forEach((body,i)=>{
    const k=i*3;body.position.set(a[k]+(b[k]-a[k])*blend,a[k+1]+(b[k+1]-a[k+1])*blend,a[k+2]+(b[k+2]-a[k+2])*blend);
    const p=trace!.frames[Math.max(0,index-1)],dx=b[k]-p[k],dz=b[k+2]-p[k+2];
    // Engine forward is -Z.
    if(dx*dx+dz*dz>1e-6)body.rotation.y=Math.atan2(-dx,-dz);
   });
  }
  // Keep the user's orbit offset while the camera tracks the object.
  function follow(){const body=actors.children[0];if(!body)return;const delta=body.position.clone().sub(followed);camera.position.add(delta);orbit.target.add(delta);followed.copy(body.position);}
  function frame(now:number){
   animation=requestAnimationFrame(frame);
   if(trace&&play){clock=Math.min(clock+(now-last)/1000,(trace.frames.length-1)*trace.frameSeconds);pose();}
   if(trace)follow();
   last=now;
   const width=Math.max(1,element.clientWidth),height=Math.max(1,element.clientHeight);
   if(renderer.domElement.width!==Math.round(width*renderer.getPixelRatio())||renderer.domElement.height!==Math.round(height*renderer.getPixelRatio())){renderer.setSize(width,height,false);camera.aspect=width/height;camera.updateProjectionMatrix();}
   orbit.update();renderer.render(scene,camera);
  }
  animation=requestAnimationFrame(frame);
  stage.current={
   setTrace:next=>{trace=next;clock=0;populate();pose();follow();},
   setObject:document=>{template?.traverse(node=>{if(node instanceof THREE.Mesh){node.geometry.dispose();(node.material as THREE.Material).dispose();}});template=document?objectTemplate(document):null;trace=null;clock=0;populate();camera.position.sub(orbit.target);orbit.target.set(0,0,0);followed.set(0,0,0);},
   setSwitches:value=>{ground.visible=value.ground;sun.visible=value.light;spot.visible=value.spotlight;},
   replay:()=>{clock=0;pose();},
   setPlaying:value=>{play=value;},
  };
  stage.current.setSwitches(switches);populate();
  return()=>{cancelAnimationFrame(animation);stage.current=null;orbit.dispose();scene.traverse(node=>{if(node instanceof THREE.Mesh){node.geometry.dispose();(Array.isArray(node.material)?node.material:[node.material]).forEach(material=>material.dispose());}});renderer.dispose();renderer.domElement.remove();};
  // eslint-disable-next-line react-hooks/exhaustive-deps -- One renderer per mount; later switch changes go through the stage.
 },[]);
 async function choose(item:PlaypenObject|null){
  setPicking(false);setError("");
  if(!item){setObject(null);stage.current?.setObject(null);setHasTrace(false);return;}
  try{
   const response=await fetch(`/api/structures?projectId=${encodeURIComponent(projectId)}&asset=${encodeURIComponent(item.path)}`,{cache:"no-store"});const data=await response.json();
   if(!response.ok)throw new Error(data.error??"Object could not be loaded.");
   stage.current?.setObject(structureDocument(data.document));setObject(item);setHasTrace(false);
  }catch(failure){setError(failure instanceof Error?failure.message:"Object could not be loaded.");}
 }
 async function run(){
  setRunning(true);setError("");
  try{const response=await fetch("/api/brains/preview",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,path,brain,options})});const text=await response.text();if(!response.ok)throw new Error(JSON.parse(text).error??"Simulation failed.");stage.current?.setTrace(parseTrace(text));setHasTrace(true);setPlaying(true);}
  catch(failure){setError(failure instanceof Error?failure.message:"Simulation failed.");}finally{setRunning(false);}
 }
 const toggle=(key:keyof Switches,label:string)=><label className="brain-switch" key={key}><input type="checkbox" role="switch" checked={switches[key]} onChange={event=>setSwitches({...switches,[key]:event.target.checked})}/><span>{label}</span></label>;
 return <div className="brain-preview">
  <div className="brain-preview-controls brain-playpen-controls">
   {toggle("ground","Ground")}{toggle("light","Light")}{toggle("spotlight","Spotlight")}
   <button type="button" className="quiet-button" aria-haspopup="dialog" onClick={()=>setPicking(true)}>{object?`Object: ${object.name}`:"Choose object…"}</button>
   <button type="button" className="settings-save" disabled={disabled||running} onClick={()=>void run()}>{running?"Simulating…":"Run in engine"}</button>
   {hasTrace&&<button type="button" className="quiet-button" onClick={()=>{stage.current?.replay();setPlaying(true);}}>Replay</button>}
   {hasTrace&&<button type="button" className="quiet-button" onClick={()=>setPlaying(value=>!value)}>{playing?"Pause":"Play"}</button>}
  </div>
  {error&&<p className="brain-error" role="alert">{error}</p>}
  <div ref={host} className="brain-playpen-stage"/>
  <p className="transform-note">Playpen only: nothing here is saved. {object?<b>{object.name}</b>:"The green cone"} runs this brain on its own and the camera follows it; the red sphere is the <b>{options.targetGroup}</b> group.</p>
  {picking&&createPortal(<div className="asset-modal-backdrop" onMouseDown={event=>{if(event.target===event.currentTarget)setPicking(false);}} onKeyDown={event=>{if(event.key==="Escape")setPicking(false);}}>
   <section className="new-scene-modal reference-picker" role="dialog" aria-modal="true" aria-labelledby="playpen-object-title">
    <header className="asset-modal-head"><div><span className="eyebrow">OBJECT LIBRARY</span><h2 id="playpen-object-title">Give this brain to…</h2></div><button type="button" aria-label="Close object library" autoFocus onClick={()=>setPicking(false)}>×</button></header>
    <div className="reference-picker-body">
     {objects.length?<div className="reference-picker-grid" aria-label="Library objects">
      <button type="button" aria-pressed={!object} onClick={()=>void choose(null)}><span className="brain-playpen-cone" aria-hidden="true">▲</span><span>Default cone</span></button>
      {objects.map(item=><button type="button" key={item.path} title={item.path} aria-pressed={object?.path===item.path} onClick={()=>void choose(item)}>{item.thumbnail?<img src={item.thumbnail} alt=""/>:<span className="brain-playpen-cone" aria-hidden="true">?</span>}<span>{item.name}</span></button>)}
     </div>:<p className="object-muted">No objects yet. Build one in the Object Builder.</p>}
    </div>
   </section>
  </div>,document.body)}
 </div>;
}
