"use client";
import {useEffect,useRef,useState} from "react";
import * as THREE from "three";
import type {MaterialSettings} from "@/lib/material-settings";
// shortcut: browser Blinn-Phong approximation; noise channel maps, bump and triplanar projection render only in native inspection.
const linear=(rgb:number[]|undefined,fallback=[.5,.5,.5])=>{const [r,g,b]=rgb??fallback;return new THREE.Color().setRGB(r,g,b,THREE.LinearSRGBColorSpace);};
const textures=new Map<string,Promise<THREE.Texture|null>>();
function loadTexture(projectId:string,asset:string){
 const url=`/api/project-asset?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(asset)}`;
 if(!textures.has(url))textures.set(url,new THREE.TextureLoader().loadAsync(url).then(texture=>{texture.colorSpace=THREE.SRGBColorSpace;texture.wrapS=texture.wrapT=THREE.RepeatWrapping;return texture;},()=>null));
 return textures.get(url)!;
}
// The engine samples diffuse images at local position × scale; repeat approximates that over each shape's UV span in metres.
async function threeMaterial(projectId:string,settings:MaterialSettings,span:[number,number]){
 const source=settings.diffuse_map?await loadTexture(projectId,settings.diffuse_map.asset):null,map=source?.clone()??null;
 map?.repeat.set(span[0]*settings.diffuse_map!.scale,span[1]*settings.diffuse_map!.scale);
 if(settings.shading==="unlit")return new THREE.MeshBasicMaterial({color:linear(settings.albedo),map});
 return new THREE.MeshPhongMaterial({color:linear(settings.albedo),map,specular:linear(settings.specular),shininess:settings.shininess,emissive:linear(settings.emission),emissiveIntensity:settings.emission_intensity,flatShading:settings.shading==="flat"});
}
function disposeMaterial(material:THREE.Material){if("map" in material&&material.map instanceof THREE.Texture)material.map.dispose();material.dispose();}
// A point light above-right and in front of the shapes gives tight specular highlights; the soft key keeps the shadow side readable.
function lights(scene:THREE.Scene){const point=new THREE.PointLight(0xffffff,14,0,2);point.position.set(1.2,1,1.8);const key=new THREE.DirectionalLight(0xffffff,1);key.position.set(3,4,5);const rim=new THREE.DirectionalLight(0xa8c4ff,.6);rim.position.set(-4,2,-3);scene.add(new THREE.AmbientLight(0xffffff,.35),point,key,rim);return point;}
const shapes:{geometry:()=>THREE.BufferGeometry;x:number;span:[number,number]}[]=[
 {geometry:()=>new THREE.SphereGeometry(.55,64,32),x:-1.55,span:[Math.PI*1.1,Math.PI*.55]},
 {geometry:()=>new THREE.BoxGeometry(.85,.85,.85),x:0,span:[.85,.85]},
 {geometry:()=>new THREE.TorusGeometry(.42,.17,32,96),x:1.55,span:[Math.PI*.84,Math.PI*.34]},
];
export function MaterialStage({projectId,material}:{projectId:string;material:MaterialSettings}){
 const host=useRef<HTMLDivElement>(null),meshes=useRef<THREE.Mesh[]>([]),[error,setError]=useState("");
 useEffect(()=>{
  const element=host.current!;let renderer:THREE.WebGLRenderer;
  // eslint-disable-next-line react-hooks/set-state-in-effect -- GPU availability is known only after mount.
  try{renderer=new THREE.WebGLRenderer({antialias:true});}catch{setError("WebGL is unavailable.");return;}
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.setClearColor(0x161d24);renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.domElement.setAttribute("aria-label","Material preview on sphere, cube and torus");renderer.domElement.setAttribute("role","img");element.append(renderer.domElement);
  const scene=new THREE.Scene(),camera=new THREE.PerspectiveCamera(32,1,.1,50);camera.position.set(0,.9,5.4);camera.lookAt(0,0,0);const bulb=new THREE.Mesh(new THREE.SphereGeometry(.05,16,8),new THREE.MeshBasicMaterial({color:0xfff6dd}));lights(scene).add(bulb);
  meshes.current=shapes.map(shape=>{const mesh=new THREE.Mesh(shape.geometry(),new THREE.MeshPhongMaterial({color:0x808080}));mesh.position.x=shape.x;mesh.rotation.x=.35;scene.add(mesh);return mesh;});
  const resize=()=>{const {width,height}=element.getBoundingClientRect();if(!width||!height)return;renderer.setSize(width,height);camera.aspect=width/height;camera.position.z=Math.max(4.2,8.5/camera.aspect);camera.updateProjectionMatrix();};
  const observer=new ResizeObserver(resize);observer.observe(element);resize();
  const clock=new THREE.Clock();renderer.setAnimationLoop(()=>{const dt=clock.getDelta();for(const mesh of meshes.current)mesh.rotation.y+=dt*.35;renderer.render(scene,camera);});
  return ()=>{renderer.setAnimationLoop(null);observer.disconnect();bulb.geometry.dispose();bulb.material.dispose();for(const mesh of meshes.current){mesh.geometry.dispose();disposeMaterial(mesh.material as THREE.Material);}meshes.current=[];renderer.dispose();renderer.domElement.remove();};
 },[]);
 useEffect(()=>{
  let cancelled=false;
  void Promise.all(shapes.map(shape=>threeMaterial(projectId,material,shape.span))).then(materials=>{
   if(cancelled||!meshes.current.length){materials.forEach(disposeMaterial);return;}
   meshes.current.forEach((mesh,index)=>{disposeMaterial(mesh.material as THREE.Material);mesh.material=materials[index];});
  });
  return ()=>{cancelled=true;};
 },[projectId,material]);
 return <div className="material-stage-canvas" ref={host}>{error&&<p role="alert">{error}</p>}</div>;
}
// One short-lived offscreen renderer draws every library swatch, so the carousel never holds a WebGL context per tile.
export function useMaterialThumbnails(projectId:string,items:{path:string;revision:string;material:MaterialSettings}[]){
 const [thumbnails,setThumbnails]=useState<Record<string,string>>({}),done=useRef(new Map<string,string>());
 useEffect(()=>{
  const pending=items.filter(item=>done.current.get(item.path)!==item.revision);if(!pending.length)return;
  let cancelled=false,renderer:THREE.WebGLRenderer;
  try{renderer=new THREE.WebGLRenderer({antialias:true,alpha:true,preserveDrawingBuffer:true});}catch{return;}
  renderer.setSize(160,104,false);renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.setClearColor(0x000000,0);
  const scene=new THREE.Scene(),camera=new THREE.PerspectiveCamera(30,160/104,.1,20),sphere=new THREE.Mesh(shapes[0].geometry());camera.position.set(0,.4,2.6);camera.lookAt(0,0,0);lights(scene);scene.add(sphere);
  void (async()=>{for(const item of pending){const material=await threeMaterial(projectId,item.material,shapes[0].span);if(cancelled){disposeMaterial(material);break;}sphere.material=material;renderer.render(scene,camera);const url=renderer.domElement.toDataURL("image/png");disposeMaterial(material);done.current.set(item.path,item.revision);setThumbnails(previous=>({...previous,[item.path]:url}));}sphere.geometry.dispose();renderer.dispose();renderer.forceContextLoss();})();
  return ()=>{cancelled=true;};
 },[projectId,items]);
 return thumbnails;
}
