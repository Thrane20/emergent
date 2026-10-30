"use client";
import {useEffect,useRef,useState} from "react";
import * as THREE from "three";
import {OrbitControls} from "three/addons/controls/OrbitControls.js";
import {TransformControls} from "three/addons/controls/TransformControls.js";
import {extrudeFace,faceNormal,moveMeshSelection,structureDocument,triangulateFace,type StructureDocument,type StructurePart,type StructureSelection} from "@/lib/structure";
export type StructureMode="object"|"face"|"vertex";
export type StructureTool="move"|"extrude";
type Props={document:StructureDocument;selection:StructureSelection|null;mode:StructureMode;tool:StructureTool;transformMode:"translate"|"rotate"|"scale";disabled:boolean;onSelect:(selection:StructureSelection|null)=>void;onCommit:(document:StructureDocument)=>void;onError:(message:string)=>void;onBusy:(busy:boolean)=>void};
type Drag={base:StructureDocument;selection:StructureSelection;origin:THREE.Vector3;plane:THREE.Plane;normal:THREE.Vector3;start:THREE.Vector2;proxy:THREE.Vector3;part:THREE.Mesh;current:StructureDocument;changed:boolean;pointer?:number};
function geometry(mesh:StructurePart["mesh"]){const result=new THREE.BufferGeometry(),indices:number[]=[],faces:number[]=[];for(let face=0;face<mesh.faces.length;face++){const tris=triangulateFace(mesh,face);indices.push(...tris);for(let i=0;i<tris.length;i+=3)faces.push(face);}result.setAttribute("position",new THREE.Float32BufferAttribute(mesh.vertices.flat(),3));result.setIndex(indices);result.computeVertexNormals();return {geometry:result,faces};}
function disposeTree(root:THREE.Object3D){root.traverse(object=>{if(object instanceof THREE.Mesh||object instanceof THREE.Points||object instanceof THREE.LineSegments){object.geometry.dispose();for(const material of Array.isArray(object.material)?object.material:[object.material])material.dispose();}});root.clear();}
export function StructureViewport(props:Props){
 const host=useRef<HTMLDivElement>(null),latest=useRef(props),runtime=useRef<{rebuild:()=>void;focus:()=>void}|null>(null),[error,setError]=useState("");
 useEffect(()=>{latest.current=props;runtime.current?.rebuild();},[props]);
 useEffect(()=>{
  const element=host.current!;let renderer:THREE.WebGLRenderer;
  // eslint-disable-next-line react-hooks/set-state-in-effect -- Report GPU creation failure after the canvas mounts.
  try{renderer=new THREE.WebGLRenderer({antialias:true});}catch{setError("WebGL is unavailable. Use the numeric controls to edit this object.");return;}
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.setClearColor(0x101713);renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.domElement.tabIndex=0;renderer.domElement.setAttribute("aria-label","Object Builder 3D viewport");element.prepend(renderer.domElement);
  const scene=new THREE.Scene(),camera=new THREE.PerspectiveCamera(45,1,0.01,100000);camera.position.set(3,2.5,4);
  scene.add(new THREE.HemisphereLight(0xe3f2e6,0x29352c,2.2));const sun=new THREE.DirectionalLight(0xffffff,2.2);sun.position.set(4,8,5);scene.add(sun);
  const grid=new THREE.GridHelper(20,40,0x50705a,0x293a2d);grid.position.y=-0.501;scene.add(grid);
  const content=new THREE.Group();scene.add(content);
  const orbit=new OrbitControls(camera,renderer.domElement);orbit.enableDamping=true;
  const gizmo=new TransformControls(camera,renderer.domElement);scene.add(gizmo.getHelper());gizmo.setSpace("world");
  const proxy=new THREE.Object3D();scene.add(proxy);
  const meshes=new Map<string,THREE.Mesh>(),targets:THREE.Mesh[]=[],points:THREE.Points[]=[],ray=new THREE.Raycaster(),cursor=new THREE.Vector2();let drag:Drag|null=null,frame=0,first=true;
  function focus(){const box=new THREE.Box3().setFromObject(content);if(box.isEmpty())return;const center=box.getCenter(new THREE.Vector3()),radius=Math.max(0.5,box.getSize(new THREE.Vector3()).length()/2),distance=radius/Math.sin(camera.fov*Math.PI/360)*1.25;orbit.target.copy(center);camera.position.copy(center).add(new THREE.Vector3(1,0.7,1).normalize().multiplyScalar(distance));camera.near=Math.max(0.001,distance/10000);camera.far=Math.max(1000,distance*100);camera.updateProjectionMatrix();orbit.update();}
  function decorate(mesh:THREE.Mesh,part:StructurePart){
   const selected=latest.current.selection,mode=latest.current.mode;
   const wire=new THREE.LineSegments(new THREE.EdgesGeometry(mesh.geometry,1),new THREE.LineBasicMaterial({color:selected?.part===part.id?0xb5dfc1:0x365d43,transparent:true,opacity:0.8}));wire.userData.decoration=true;mesh.add(wire);
   if(mode==="vertex"){
    const geometry=new THREE.BufferGeometry().setAttribute("position",new THREE.Float32BufferAttribute(part.mesh.vertices.flat(),3));const cloud=new THREE.Points(geometry,new THREE.PointsMaterial({color:0xc8e7cf,size:6,sizeAttenuation:false}));cloud.userData={part:part.id,decoration:true};points.push(cloud);mesh.add(cloud);
   }
   if(selected?.part===part.id&&selected.face!==undefined&&part.mesh.faces[selected.face]){
    const highlight=new THREE.BufferGeometry().setAttribute("position",new THREE.Float32BufferAttribute(part.mesh.vertices.flat(),3));highlight.setIndex(triangulateFace(part.mesh,selected.face));const faceMesh=new THREE.Mesh(highlight,new THREE.MeshBasicMaterial({color:0xf0bd69,side:THREE.DoubleSide,transparent:true,opacity:0.5,depthWrite:false,polygonOffset:true,polygonOffsetFactor:-2}));faceMesh.userData.decoration=true;mesh.add(faceMesh);
   }
   if(selected?.part===part.id&&selected.vertex!==undefined&&part.mesh.vertices[selected.vertex]){const marker=new THREE.Points(new THREE.BufferGeometry().setAttribute("position",new THREE.Float32BufferAttribute(part.mesh.vertices[selected.vertex],3)),new THREE.PointsMaterial({color:0xffc269,size:12,sizeAttenuation:false,depthTest:false}));marker.renderOrder=2;marker.userData.decoration=true;mesh.add(marker);}
  }
  function rebuild(){
   if(drag||gizmo.dragging)return;const p=latest.current;gizmo.detach();disposeTree(content);meshes.clear();targets.length=0;points.length=0;
   try{
    for(const part of p.document.parts){const built=geometry(part.mesh),mesh=new THREE.Mesh(built.geometry,new THREE.MeshStandardMaterial({color:p.selection?.part===part.id?0x93b5a1:0x718d7e,flatShading:true,roughness:0.8,side:THREE.DoubleSide}));mesh.userData={part:part.id,faces:built.faces};mesh.position.fromArray(part.transform.translation);mesh.rotation.set(...part.transform.rotation_degrees.map(n=>n*Math.PI/180) as [number,number,number],"XYZ");mesh.scale.fromArray(part.transform.scale);meshes.set(part.id,mesh);targets.push(mesh);decorate(mesh,part);}
    for(const part of p.document.parts)(part.parent?meshes.get(part.parent)!:content).add(meshes.get(part.id)!);content.updateMatrixWorld(true);
    const selected=p.selection,mesh=selected?meshes.get(selected.part):undefined,part=p.document.parts.find(part=>part.id===selected?.part);
    if(mesh&&part&&!p.disabled){
     if(p.mode==="object"){gizmo.setMode(p.transformMode);gizmo.attach(mesh);}
     else if(selected?.face!==undefined||selected?.vertex!==undefined){const indices=selected.vertex!==undefined?[selected.vertex]:part.mesh.faces[selected.face!]??[];if(indices.length){const center=new THREE.Vector3();for(const i of indices)center.add(new THREE.Vector3().fromArray(part.mesh.vertices[i]));center.divideScalar(indices.length).applyMatrix4(mesh.matrixWorld);proxy.position.copy(center);proxy.rotation.set(0,0,0);proxy.scale.set(1,1,1);gizmo.setMode("translate");gizmo.attach(proxy);}}
    }
    if(first&&p.document.parts.length){focus();first=false;}
   }catch(error){latest.current.onError(error instanceof Error?error.message:"Object preview could not be built.");}
  }
  function setRay(event:PointerEvent){const box=renderer.domElement.getBoundingClientRect();cursor.set((event.clientX-box.left)/box.width*2-1,-(event.clientY-box.top)/box.height*2+1);ray.setFromCamera(cursor,camera);}
  function begin(selection:StructureSelection,origin:THREE.Vector3,start:THREE.Vector2,pointer?:number){
   const mesh=meshes.get(selection.part)!;content.updateMatrixWorld(true);const part=latest.current.document.parts.find(part=>part.id===selection.part)!;
   const normal=selection.face!==undefined?new THREE.Vector3().fromArray(faceNormal(part.mesh,selection.face)).applyNormalMatrix(new THREE.Matrix3().getNormalMatrix(mesh.matrixWorld)):new THREE.Vector3(0,1,0);
   drag={base:structuredClone(latest.current.document),selection,origin:origin.clone(),plane:new THREE.Plane().setFromNormalAndCoplanarPoint(camera.getWorldDirection(new THREE.Vector3()),origin),normal,start,proxy:proxy.position.clone(),part:mesh,current:latest.current.document,changed:false,pointer};orbit.enabled=false;latest.current.onBusy(true);
  }
  function updatePart(document:StructureDocument,selection:StructureSelection){
   const part=document.parts.find(part=>part.id===selection.part)!,mesh=meshes.get(part.id)!;mesh.position.fromArray(part.transform.translation);mesh.rotation.set(...part.transform.rotation_degrees.map(n=>n*Math.PI/180) as [number,number,number],"XYZ");mesh.scale.fromArray(part.transform.scale);
   if(latest.current.mode!=="object"){
    try{const built=geometry(part.mesh);mesh.geometry.dispose();mesh.geometry=built.geometry;mesh.userData.faces=built.faces;for(const child of [...mesh.children])if(child.userData.decoration){const index=points.indexOf(child as THREE.Points);if(index>=0)points.splice(index,1);disposeTree(child);mesh.remove(child);}decorate(mesh,part);}catch{/* Keep the last valid preview until the drag is committed or cancelled. */}
   }
   content.updateMatrixWorld(true);
  }
  function translate(delta:THREE.Vector3){
   if(!drag)return;const p=latest.current,d=drag,next=structuredClone(d.base),part=next.parts.find(part=>part.id===d.selection.part)!,base=d.base.parts.find(part=>part.id===d.selection.part)!;
   if(p.mode==="object"){
    const parent=d.part.parent!,inverse=parent.matrixWorld.clone().invert(),origin=new THREE.Vector3().applyMatrix4(inverse),local=delta.clone().applyMatrix4(inverse).sub(origin);part.transform.translation=base.transform.translation.map((n,i)=>n+local.getComponent(i));
   }else{
    const movement=delta.clone();if(p.mode==="face"&&p.tool==="extrude")movement.copy(d.normal).multiplyScalar(delta.dot(d.normal));
    const inverse=d.part.matrixWorld.clone().invert(),origin=new THREE.Vector3().applyMatrix4(inverse),local=movement.applyMatrix4(inverse).sub(origin);
    if(p.mode==="face"&&p.tool==="extrude"){
     const distance=local.length();if(distance<0.0001)return;part.mesh=extrudeFace(base.mesh,d.selection.face!,distance);
     const old=base.mesh.faces[d.selection.face!],cap=part.mesh.faces[d.selection.face!];for(let i=0;i<cap.length;i++)part.mesh.vertices[cap[i]]=base.mesh.vertices[old[i]].map((n,c)=>n+local.getComponent(c));
    }else part.mesh=moveMeshSelection(base.mesh,d.selection,local.toArray());
   }
   d.current=next;d.changed=true;updatePart(next,d.selection);
  }
  function finish(cancel=false){
   const d=drag;if(!d)return;drag=null;orbit.enabled=true;latest.current.onBusy(false);
   if(d.pointer!==undefined&&renderer.domElement.hasPointerCapture(d.pointer))renderer.domElement.releasePointerCapture(d.pointer);
   if(!cancel&&d.changed){try{structureDocument(d.current);latest.current.onCommit(d.current);}catch(error){latest.current.onError(error instanceof Error?error.message:"Invalid edit. Previous object restored.");}}
   rebuild();
  }
  function down(event:PointerEvent){
   if(event.button!==0||event.altKey||latest.current.disabled||gizmo.axis||drag)return;setRay(event);const hits=ray.intersectObjects(targets,false),hit=hits[0];
   let selection:StructureSelection|null=null,origin:THREE.Vector3|undefined;
   if(latest.current.mode==="vertex"){
    ray.params.Points.threshold=camera.position.distanceTo(orbit.target)*Math.tan(camera.fov*Math.PI/360)*18/renderer.domElement.clientHeight;
    const point=ray.intersectObjects(points,false).find(point=>!hit||point.distance<=hit.distance+ray.params.Points!.threshold*2);
    if(point&&point.index!==undefined){selection={part:point.object.userData.part,vertex:point.index};origin=point.point;}
   }else if(hit){selection={part:hit.object.userData.part,...(latest.current.mode==="face"?{face:hit.object.userData.faces[hit.faceIndex!]}:{})};origin=hit.point;}
   if(!selection||!origin)return;
   event.stopImmediatePropagation();renderer.domElement.focus();latest.current.onSelect(selection);begin(selection,origin,new THREE.Vector2(event.clientX,event.clientY),event.pointerId);renderer.domElement.setPointerCapture(event.pointerId);event.preventDefault();
  }
  function move(event:PointerEvent){
   if(!drag||drag.pointer!==event.pointerId)return;setRay(event);const d=drag,dx=event.clientX-d.start.x,dy=event.clientY-d.start.y;if(Math.hypot(dx,dy)<2&&!d.changed)return;
   if(latest.current.mode==="face"&&latest.current.tool==="extrude"){
    const box=renderer.domElement.getBoundingClientRect(),a=d.origin.clone().project(camera),b=d.origin.clone().add(d.normal).project(camera),direction=new THREE.Vector2((b.x-a.x)*box.width/2,-(b.y-a.y)*box.height/2),length=direction.lengthSq();
    const distance=length>4?(dx*direction.x+dy*direction.y)/length:-dy*camera.position.distanceTo(d.origin)/box.height*2;
    translate(d.normal.clone().multiplyScalar(distance));
   }else{const point=ray.ray.intersectPlane(d.plane,new THREE.Vector3());if(point)translate(point.sub(d.origin));}
  }
  function up(event:PointerEvent){if(drag?.pointer===event.pointerId)finish(event.type==="pointercancel");}
  function key(event:KeyboardEvent){if(event.key==="Escape"&&drag){event.preventDefault();gizmo.reset();finish(true);}else if(event.key.toLowerCase()==="f"&&!drag){event.preventDefault();focus();}}
  gizmo.addEventListener("mouseDown",()=>{const selection=latest.current.selection;if(!selection)return;const mesh=meshes.get(selection.part);if(!mesh)return;const origin=latest.current.mode==="object"?mesh.getWorldPosition(new THREE.Vector3()):proxy.position.clone();begin(selection,origin,new THREE.Vector2());});
  gizmo.addEventListener("objectChange",()=>{if(!drag||drag.pointer!==undefined)return;
   if(latest.current.mode==="object"){const next=structuredClone(drag.base),part=next.parts.find(part=>part.id===drag!.selection.part)!,mesh=drag.part;part.transform={translation:mesh.position.toArray(),rotation_degrees:[mesh.rotation.x,mesh.rotation.y,mesh.rotation.z].map(n=>n*180/Math.PI),scale:mesh.scale.toArray()};drag.current=next;drag.changed=true;}
   else translate(proxy.position.clone().sub(drag.proxy));
  });
  gizmo.addEventListener("mouseUp",()=>finish());gizmo.addEventListener("dragging-changed",event=>{orbit.enabled=event.value!==true;});
  renderer.domElement.addEventListener("pointerdown",down,true);renderer.domElement.addEventListener("pointermove",move,true);renderer.domElement.addEventListener("pointerup",up,true);renderer.domElement.addEventListener("pointercancel",up,true);renderer.domElement.addEventListener("keydown",key);
  const resize=new ResizeObserver(()=>{const width=element.clientWidth,height=element.clientHeight;renderer.setSize(width,height);camera.aspect=width/Math.max(1,height);camera.updateProjectionMatrix();});resize.observe(element);
  function render(){frame=requestAnimationFrame(render);orbit.update();renderer.render(scene,camera);}render();runtime.current={rebuild,focus};rebuild();
  return()=>{cancelAnimationFrame(frame);resize.disconnect();runtime.current=null;if(drag)latest.current.onBusy(false);gizmo.dispose();orbit.dispose();disposeTree(content);grid.geometry.dispose();(grid.material as THREE.Material).dispose();renderer.dispose();renderer.domElement.remove();};
 },[]);
 return <div className="structure-viewport" ref={host} data-parts={props.document.parts.length} data-part={props.selection?.part??""} data-face={props.selection?.face??""} data-vertex={props.selection?.vertex??""}>{error&&<p role="alert">{error}</p>}<div className="structure-view-help">Select and drag · XYZ handles · Empty space / Alt-drag to orbit · Scroll to zoom · F to frame<button type="button" className="quiet-button" onClick={()=>runtime.current?.focus()}>Frame object</button></div></div>;
}
