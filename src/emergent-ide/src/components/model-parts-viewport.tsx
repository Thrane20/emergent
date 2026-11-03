"use client";
import {useEffect,useRef,useState} from "react";
import * as THREE from "three";
import {OrbitControls} from "three/addons/controls/OrbitControls.js";
import {cutBoxCorners,cutBoxFrame,drawnCutBox,resizeBoxCorner,type CutBox} from "@/lib/model-box-cut";
import type {Axis,EditorPart} from "@/lib/model-part-geometry";

type Props={parts:EditorPart[];selected:Set<string>;active:string|null;cutPlane:{axis:Axis;offset:number}|null;boxMode:boolean;cutBox:CutBox|null;onCutBox:(box:CutBox)=>void;onCancelBox:()=>void;onPick:(id:string|null,add:boolean)=>void};

/** Presentation only: meshes here are disposable views of editor parts, never identity. */
export function ModelPartsViewport(props:Props){
 const host=useRef<HTMLDivElement>(null),rectangle=useRef<HTMLDivElement>(null),handles=useRef<HTMLDivElement>(null),latest=useRef(props),runtime=useRef<{rebuild:(refocus:boolean)=>void;updateBox:()=>void}|null>(null),shown=useRef<EditorPart[]>([]),[error,setError]=useState("");
 useEffect(()=>{const before=latest.current;latest.current=props;
 const refocus=!props.parts.some(part=>shown.current.some(old=>old.id===part.id));shown.current=props.parts;
 if(before.parts!==props.parts||before.selected!==props.selected||before.active!==props.active||before.cutPlane!==props.cutPlane)runtime.current?.rebuild(refocus);
 else runtime.current?.updateBox();
 },[props]);
 useEffect(()=>{
  const element=host.current!;let renderer:THREE.WebGLRenderer;
  // eslint-disable-next-line react-hooks/set-state-in-effect -- GPU availability is known only after mount.
  try{renderer=new THREE.WebGLRenderer({antialias:true});}catch{setError("WebGL is unavailable.");return;}
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.setClearColor(0x161d24);renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.domElement.setAttribute("aria-label","Model parts 3D viewport");renderer.domElement.tabIndex=0;element.prepend(renderer.domElement);
  const scene=new THREE.Scene(),camera=new THREE.PerspectiveCamera(40,1,0.01,1000);camera.position.set(2.5,1.8,3);
  scene.add(new THREE.HemisphereLight(0xffffff,0x3a4450,2.4));const sun=new THREE.DirectionalLight(0xffffff,1.8);sun.position.set(3,6,4);scene.add(sun);
  const content=new THREE.Group(),overlay=new THREE.Group(),cutter=new THREE.Group();scene.add(content,overlay,cutter);
  const orbit=new OrbitControls(camera,renderer.domElement);orbit.enableDamping=true;
  const owned:(THREE.Material|THREE.BufferGeometry)[]=[];
  function clear(){content.clear();overlay.clear();for(const item of owned.splice(0))item.dispose();}
  const boxOwned:(THREE.Material|THREE.BufferGeometry)[]=[];let worldCorners:THREE.Vector3[]=[];
  function updateBox(){
   cutter.clear();for(const item of boxOwned.splice(0))item.dispose();worldCorners=[];
   const box=latest.current.cutBox;if(!box)return;
   worldCorners=cutBoxCorners(box);
   const size=new THREE.Vector3(...box.max).sub(new THREE.Vector3(...box.min)),middle=new THREE.Vector3(...box.min).add(new THREE.Vector3(...box.max)).multiplyScalar(0.5);
   const geometry=new THREE.BoxGeometry(...size.toArray()),material=new THREE.MeshBasicMaterial({color:0xffb347,transparent:true,opacity:0.12,depthWrite:false});
   const mesh=new THREE.Mesh(geometry,material);mesh.position.copy(middle.applyMatrix4(cutBoxFrame(box)));mesh.quaternion.fromArray(box.rotation);mesh.renderOrder=3;cutter.add(mesh);
   const edges=new THREE.EdgesGeometry(geometry),lineMaterial=new THREE.LineBasicMaterial({color:0xffc86b,depthTest:false,transparent:true,opacity:0.95}),lines=new THREE.LineSegments(edges,lineMaterial);
   lines.position.copy(mesh.position);lines.quaternion.copy(mesh.quaternion);lines.renderOrder=4;cutter.add(lines);boxOwned.push(geometry,material,edges,lineMaterial);
  }
  function focus(){const box=new THREE.Box3().setFromObject(content);if(box.isEmpty())return;const center=box.getCenter(new THREE.Vector3()),radius=Math.max(0.2,box.getSize(new THREE.Vector3()).length()/2);orbit.target.copy(center);camera.position.copy(center).add(new THREE.Vector3(0.9,0.6,1.1).normalize().multiplyScalar(radius/Math.sin(camera.fov*Math.PI/360)*1.1));camera.near=radius/100;camera.far=radius*100;camera.updateProjectionMatrix();}
  function rebuild(refocus:boolean){
   clear();const {parts,selected,active,cutPlane}=latest.current;
   for(const part of parts){
    const highlighted=selected.has(part.id);
    let material=part.material;
    if(highlighted){material=part.material.clone();(material as THREE.MeshStandardMaterial).emissive?.set(part.id===active?0x3d6ea8:0x24476e);owned.push(material);}
    const mesh=new THREE.Mesh(part.geometry,material);mesh.userData.part=part.id;content.add(mesh);
   }
   const activePart=parts.find(part=>part.id===active);
   if(activePart){
    const box=new THREE.Box3().setFromBufferAttribute(activePart.geometry.getAttribute("position") as THREE.BufferAttribute),size=box.getSize(new THREE.Vector3());
    const axes=new THREE.AxesHelper(Math.max(size.length()*0.35,0.05));axes.position.fromArray(activePart.pivot);(axes.material as THREE.Material).depthTest=false;axes.renderOrder=2;owned.push(axes.geometry,axes.material as THREE.Material);overlay.add(axes);
    if(cutPlane){
     const extent=size.clone().multiplyScalar(1.3).max(new THREE.Vector3(0.05,0.05,0.05)),dims=[0,1,2].filter(axis=>axis!==cutPlane.axis).map(axis=>extent.getComponent(axis));
     const geometry=new THREE.PlaneGeometry(dims[0],dims[1]),plane=new THREE.MeshBasicMaterial({color:0xffb347,transparent:true,opacity:0.35,side:THREE.DoubleSide,depthWrite:false});owned.push(geometry,plane);
     const mesh=new THREE.Mesh(geometry,plane),center=box.getCenter(new THREE.Vector3());center.setComponent(cutPlane.axis,cutPlane.offset);mesh.position.copy(center);
     if(cutPlane.axis===0)mesh.rotation.y=Math.PI/2;else if(cutPlane.axis===1)mesh.rotation.x=-Math.PI/2;
     overlay.add(mesh);
    }
   }
   if(refocus)focus();updateBox();
  }
  runtime.current={rebuild,updateBox};rebuild(true);
  const ray=new THREE.Raycaster(),pointer=new THREE.Vector2();let down:{x:number;y:number}|null=null;
  type Drawing={origin:THREE.Vector3;rotation:THREE.Quaternion;plane:THREE.Plane;start:THREE.Vector3;x:number;y:number};
  type Dragging={corner:number;box:CutBox;plane:THREE.Plane;start:THREE.Vector3;point:THREE.Vector3};
  let drawing:Drawing|null=null,dragging:Dragging|null=null,captured:number|null=null;
  function setRay(event:PointerEvent){const rect=renderer.domElement.getBoundingClientRect();pointer.set((event.clientX-rect.left)/rect.width*2-1,-(event.clientY-rect.top)/rect.height*2+1);ray.setFromCamera(pointer,camera);}
  function intercept(event:PointerEvent){event.preventDefault();event.stopImmediatePropagation();orbit.enabled=false;(event.target instanceof HTMLButtonElement?event.target:renderer.domElement).focus({preventScroll:true});captured=event.pointerId;element.setPointerCapture(event.pointerId);}
  function finish(){drawing=null;dragging=null;down=null;orbit.enabled=true;if(rectangle.current)rectangle.current.style.display="none";if(captured!==null&&element.hasPointerCapture(captured))element.releasePointerCapture(captured);captured=null;}
  const onDown=(event:PointerEvent)=>{
   if(event.button!==0)return;
   const {boxMode,cutBox}=latest.current;
   const target=(event.target as Element).closest<HTMLElement>("[data-box-corner]");
   if(boxMode&&cutBox&&target){
    setRay(event);const corner=Number(target.dataset.boxCorner),point=cutBoxCorners(cutBox)[corner],plane=new THREE.Plane().setFromNormalAndCoplanarPoint(camera.getWorldDirection(new THREE.Vector3()),point),start=ray.ray.intersectPlane(plane,new THREE.Vector3());
    if(start){dragging={corner,box:cutBox,plane,start,point};intercept(event);}return;
   }
   if(boxMode&&!cutBox){
    setRay(event);const bounds=new THREE.Box3().setFromObject(content);if(bounds.isEmpty())return;
    const origin=bounds.getCenter(new THREE.Vector3()),rotation=camera.quaternion.clone(),plane=new THREE.Plane().setFromNormalAndCoplanarPoint(camera.getWorldDirection(new THREE.Vector3()),origin),start=ray.ray.intersectPlane(plane,new THREE.Vector3());
    if(start){drawing={origin,rotation,plane,start,x:event.clientX,y:event.clientY};intercept(event);}return;
   }
   down={x:event.clientX,y:event.clientY};
  };
  const onMove=(event:PointerEvent)=>{
   if(captured!==event.pointerId)return;
   if(drawing&&rectangle.current){
    const rect=renderer.domElement.getBoundingClientRect();Object.assign(rectangle.current.style,{display:"block",left:`${Math.min(drawing.x,event.clientX)-rect.left}px`,top:`${Math.min(drawing.y,event.clientY)-rect.top}px`,width:`${Math.abs(event.clientX-drawing.x)}px`,height:`${Math.abs(event.clientY-drawing.y)}px`});
   }else if(dragging){
    setRay(event);const point=ray.ray.intersectPlane(dragging.plane,new THREE.Vector3());
    if(point)latest.current.onCutBox(resizeBoxCorner(dragging.box,dragging.corner,point.sub(dragging.start).add(dragging.point)));
   }
  };
  const onUp=(event:PointerEvent)=>{
   if(captured===event.pointerId){
    if(drawing&&Math.abs(event.clientX-drawing.x)>=4&&Math.abs(event.clientY-drawing.y)>=4){setRay(event);const end=ray.ray.intersectPlane(drawing.plane,new THREE.Vector3());if(end){const box=drawnCutBox(drawing.origin,drawing.rotation,drawing.start,end);if(box)latest.current.onCutBox(box);}}
    finish();return;
   }
   const start=down;down=null;
   if(latest.current.boxMode||!start||Math.hypot(event.clientX-start.x,event.clientY-start.y)>4)return;
   setRay(event);const hit=ray.intersectObjects(content.children,false)[0];latest.current.onPick(hit?hit.object.userData.part as string:null,event.shiftKey||event.metaKey||event.ctrlKey);
  };
  const onCancel=()=>finish();
  const onKey=(event:KeyboardEvent)=>{
   if(event.key==="Escape"&&latest.current.boxMode){finish();latest.current.onCancelBox();event.preventDefault();return;}
   const handle=(event.target as Element).closest<HTMLElement>("[data-box-corner]"),box=latest.current.cutBox;
   if(!handle||!box||!["ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(event.key))return;
   event.preventDefault();const corner=Number(handle.dataset.boxCorner),point=cutBoxCorners(box)[corner],step=Math.max(...box.max.map((value,axis)=>value-box.min[axis]))*(event.shiftKey?0.01:0.05);
   const direction=new THREE.Vector3(event.key==="ArrowLeft"?-1:event.key==="ArrowRight"?1:0,event.key==="ArrowUp"?1:event.key==="ArrowDown"?-1:0,0).applyQuaternion(camera.quaternion);
   latest.current.onCutBox(resizeBoxCorner(box,corner,point.addScaledVector(direction,step)));
  };
  element.addEventListener("pointerdown",onDown,true);element.addEventListener("pointermove",onMove,true);element.addEventListener("pointerup",onUp,true);element.addEventListener("pointercancel",onCancel);element.addEventListener("lostpointercapture",onCancel);element.addEventListener("keydown",onKey);
  const resize=()=>{const {width,height}=element.getBoundingClientRect();if(!width||!height)return;renderer.setSize(width,height);camera.aspect=width/height;camera.updateProjectionMatrix();};
  const observer=new ResizeObserver(resize);observer.observe(element);resize();
  const projected=new THREE.Vector3();
  renderer.setAnimationLoop(()=>{orbit.update();renderer.render(scene,camera);
   const width=element.clientWidth,height=element.clientHeight;
   for(let i=0;i<worldCorners.length;i++){const handle=handles.current?.children[i] as HTMLElement|undefined;if(!handle)continue;projected.copy(worldCorners[i]).project(camera);handle.style.left=`${(projected.x+1)*width/2}px`;handle.style.top=`${(1-projected.y)*height/2}px`;handle.hidden=projected.z < -1||projected.z > 1;}
  });
  return ()=>{renderer.setAnimationLoop(null);observer.disconnect();element.removeEventListener("pointerdown",onDown,true);element.removeEventListener("pointermove",onMove,true);element.removeEventListener("pointerup",onUp,true);element.removeEventListener("pointercancel",onCancel);element.removeEventListener("lostpointercapture",onCancel);element.removeEventListener("keydown",onKey);orbit.dispose();clear();for(const item of boxOwned)item.dispose();renderer.dispose();renderer.domElement.remove();runtime.current=null;};
 },[]);
 return <div className={`model-parts-canvas${props.boxMode&&!props.cutBox?" drawing-box":""}`} ref={host}><div ref={rectangle} className="model-cut-rectangle"/>{props.cutBox&&<div ref={handles} className="model-cut-handles">{Array.from({length:8},(_,corner)=><button key={corner} type="button" data-box-corner={corner} aria-label={`Resize cut box corner ${corner+1}`} title="Drag to resize in this view; orbit to adjust depth. Arrow keys also resize."/>)}</div>}{props.boxMode&&<p className="model-cut-hint">{props.cutBox?"Drag a corner to resize · orbit to adjust depth · Esc cancels":"Drag a rectangle to draw the cut box · Esc cancels"}</p>}{error&&<p role="alert">{error}</p>}</div>;
}
