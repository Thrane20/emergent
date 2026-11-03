"use client";
import {useEffect,useRef,useState} from "react";
import * as THREE from "three";
import {OrbitControls} from "three/addons/controls/OrbitControls.js";
import {TransformControls} from "three/addons/controls/TransformControls.js";
import {extrudeFace,faceNormal,moveMeshSelection,structureDocument,structureWorldMatrices,selectedPartRoots,transformStructureParts,triangulateFace,type StructureDocument,type StructurePart,type StructureSelection} from "@/lib/structure";
export type StructureMode="object"|"face"|"edge"|"vertex";
export type StructureTool="move"|"extrude";
type Props={document:StructureDocument;selection:StructureSelection|null;selectedParts:string[];mode:StructureMode;tool:StructureTool;transformMode:"translate"|"rotate"|"scale";transformSpace:"local"|"global";disabled:boolean;onSelect:(selection:StructureSelection|null,additive?:boolean)=>void;onCommit:(document:StructureDocument)=>void;onError:(message:string)=>void;onBusy:(busy:boolean)=>void;onContextMenu?:(partId:string|null)=>void};
type Drag={base:StructureDocument;selection:StructureSelection;origin:THREE.Vector3;plane:THREE.Plane;normal:THREE.Vector3;start:THREE.Vector2;proxy:THREE.Vector3;part:THREE.Mesh;current:StructureDocument;changed:boolean;pointer?:number};
function geometry(mesh:StructurePart["mesh"]){const result=new THREE.BufferGeometry(),indices:number[]=[],faces:number[]=[];for(let face=0;face<mesh.faces.length;face++){const tris=triangulateFace(mesh,face);indices.push(...tris);for(let i=0;i<tris.length;i+=3)faces.push(face);}result.setAttribute("position",new THREE.Float32BufferAttribute(mesh.vertices.flat(),3));result.setIndex(indices);if(mesh.vertices.length)result.computeVertexNormals();return {geometry:result,faces};}
function disposeTree(root:THREE.Object3D){root.traverse(object=>{if(object instanceof THREE.Mesh||object instanceof THREE.Points||object instanceof THREE.LineSegments){object.geometry.dispose();for(const material of Array.isArray(object.material)?object.material:[object.material])material.dispose();}});root.clear();}
export function StructureViewport(props:Props){
 const host=useRef<HTMLDivElement>(null),latest=useRef(props),runtime=useRef<{rebuild:()=>void;focus:()=>void;axis:(axis:"X"|"Y"|"Z")=>void}|null>(null),[error,setError]=useState("");
 useEffect(()=>{latest.current=props;runtime.current?.rebuild();},[props]);
 useEffect(()=>{
  const element=host.current!;let renderer:THREE.WebGLRenderer;
  // eslint-disable-next-line react-hooks/set-state-in-effect -- Report GPU creation failure after the canvas mounts.
  try{renderer=new THREE.WebGLRenderer({antialias:true});}catch{setError("WebGL is unavailable. Use the numeric controls to edit this object.");return;}
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.setClearColor(0x101713);renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.domElement.tabIndex=0;renderer.domElement.setAttribute("aria-label","Object Builder 3D viewport");element.prepend(renderer.domElement);
  const scene=new THREE.Scene(),perspective=new THREE.PerspectiveCamera(45,1,0.01,100000),orthographic=new THREE.OrthographicCamera(-1,1,1,-1,0.01,100000);let camera:THREE.PerspectiveCamera|THREE.OrthographicCamera=perspective;perspective.position.set(3,2.5,4);
  scene.add(new THREE.HemisphereLight(0xe3f2e6,0x29352c,2.2));const sun=new THREE.DirectionalLight(0xffffff,2.2);sun.position.set(4,8,5);scene.add(sun);
  const grid=new THREE.GridHelper(20,40,0x50705a,0x293a2d);grid.position.y=-0.501;scene.add(grid);
  const content=new THREE.Group();scene.add(content);
  const orbit=new OrbitControls(camera,renderer.domElement);orbit.enableDamping=true;
  const gizmo=new TransformControls(camera,renderer.domElement);scene.add(gizmo.getHelper());gizmo.setSpace("local");
  const proxy=new THREE.Object3D();scene.add(proxy);
  const meshes=new Map<string,THREE.Mesh>(),targets:THREE.Mesh[]=[],points:THREE.Points[]=[],ray=new THREE.Raycaster(),cursor=new THREE.Vector2();let drag:Drag|null=null,frame=0,first=true,lastAxis:"X"|"Y"|"Z"|null=null,side=1;
  function focus(){const box=new THREE.Box3().setFromObject(content);if(box.isEmpty())return;camera=perspective;orbit.object=camera;gizmo.camera=camera;lastAxis=null;const center=box.getCenter(new THREE.Vector3()),radius=Math.max(0.5,box.getSize(new THREE.Vector3()).length()/2),distance=radius/Math.sin(perspective.fov*Math.PI/360)*1.25;orbit.target.copy(center);camera.up.set(0,1,0);camera.position.copy(center).add(new THREE.Vector3(1,0.7,1).normalize().multiplyScalar(distance));camera.near=Math.max(0.001,distance/10000);camera.far=Math.max(1000,distance*100);camera.updateProjectionMatrix();orbit.update();}
  function axisView(axis:"X"|"Y"|"Z"){side=lastAxis===axis?-side:1;lastAxis=axis;frameAxis(axis);}
  function frameAxis(axis:"X"|"Y"|"Z"){
   const box=new THREE.Box3().setFromObject(content);if(box.isEmpty())return;
   camera=orthographic;orbit.object=camera;gizmo.camera=camera;
   const center=box.getCenter(new THREE.Vector3()),size=box.getSize(new THREE.Vector3()),aspect=Math.max(1,element.clientWidth)/Math.max(1,element.clientHeight);
   const horizontal=axis==="X"?size.z:size.x,vertical=axis==="Y"?size.z:size.y,half=Math.max(0.5,vertical/2,horizontal/(2*aspect))*1.2;
   camera.left=-half*aspect;camera.right=half*aspect;camera.top=half;camera.bottom=-half;
   camera.position.copy(center).add(new THREE.Vector3(axis==="X"?side:0,axis==="Y"?side:0,axis==="Z"?side:0).multiplyScalar(Math.max(10,size.length()*2)));
   camera.up.set(0,axis==="Y"?0:1,axis==="Y"?(side===1?-1:1):0);orbit.target.copy(center);camera.lookAt(center);camera.updateProjectionMatrix();orbit.update();
  }
  function decorate(mesh:THREE.Mesh,part:StructurePart){
   const selected=latest.current.selection,mode=latest.current.mode;
   const wire=new THREE.LineSegments(new THREE.EdgesGeometry(mesh.geometry,1),new THREE.LineBasicMaterial({color:latest.current.selectedParts.includes(part.id)?0xb5dfc1:0x365d43,transparent:true,opacity:0.8}));wire.userData.decoration=true;mesh.add(wire);
   if(selected?.part===part.id&&selected.edge){const geometry=new THREE.BufferGeometry().setAttribute("position",new THREE.Float32BufferAttribute(selected.edge.flatMap(i=>part.mesh.vertices[i]),3));const line=new THREE.LineSegments(geometry,new THREE.LineBasicMaterial({color:0xffc269,depthTest:false}));line.userData.decoration=true;mesh.add(line);}
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
    for(const part of p.document.parts){const built=geometry(part.mesh),mesh=new THREE.Mesh(built.geometry,new THREE.MeshStandardMaterial({color:p.selectedParts.includes(part.id)?0x93b5a1:0x718d7e,flatShading:true,roughness:0.8,side:THREE.DoubleSide}));mesh.userData={part:part.id,faces:built.faces};mesh.position.fromArray(part.transform.translation);mesh.rotation.set(...part.transform.rotation_degrees.map(n=>n*Math.PI/180) as [number,number,number],"XYZ");mesh.scale.fromArray(part.transform.scale);meshes.set(part.id,mesh);targets.push(mesh);decorate(mesh,part);}
    for(const part of p.document.parts)(part.parent?meshes.get(part.parent)!:content).add(meshes.get(part.id)!);content.updateMatrixWorld(true);
    const selected=p.selection,mesh=selected?meshes.get(selected.part):undefined,part=p.document.parts.find(part=>part.id===selected?.part);
    if(mesh&&part&&!p.disabled){
     if(p.mode==="object"){
      gizmo.setMode(p.transformMode);
      if(p.selectedParts.length>1){const world=structureWorldMatrices(p.document.parts),roots=selectedPartRoots(p.document.parts,p.selectedParts),center=new THREE.Vector3();for(const id of roots)center.set(center.x+world.get(id)!.elements[12],center.y+world.get(id)!.elements[13],center.z+world.get(id)!.elements[14]);center.divideScalar(roots.length);proxy.position.copy(center);proxy.rotation.set(0,0,0);proxy.scale.set(1,1,1);gizmo.setSpace("world");gizmo.attach(proxy);}
      else{gizmo.setSpace(p.transformSpace==="global"?"world":"local");gizmo.attach(mesh);}
     }
     else if(selected?.face!==undefined||selected?.vertex!==undefined||selected?.edge!==undefined){const indices=selected.vertex!==undefined?[selected.vertex]:selected.edge??part.mesh.faces[selected.face!]??[];if(indices.length){const center=new THREE.Vector3();for(const i of indices)center.add(new THREE.Vector3().fromArray(part.mesh.vertices[i]));center.divideScalar(indices.length).applyMatrix4(mesh.matrixWorld);proxy.position.copy(center);proxy.rotation.set(0,0,0);proxy.scale.set(1,1,1);gizmo.setMode("translate");gizmo.attach(proxy);}}
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
    if(p.selectedParts.length>1){try{const moved=transformStructureParts(d.base,p.selectedParts,"translate",delta.toArray());d.current=moved;d.changed=true;proxy.position.copy(d.proxy).add(delta);for(const id of selectedPartRoots(moved.parts,p.selectedParts))updatePart(moved,{part:id});}catch(error){p.onError(error instanceof Error?error.message:"Group move failed.");}return;}
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
    ray.params.Points.threshold=camera===orthographic?(orthographic.top-orthographic.bottom)*9/Math.max(1,element.clientHeight):camera.position.distanceTo(orbit.target)*Math.tan(perspective.fov*Math.PI/360)*18/Math.max(1,element.clientHeight);
    const point=ray.intersectObjects(points,false).find(point=>!hit||point.distance<=hit.distance+ray.params.Points!.threshold*2);
    if(point&&point.index!==undefined){selection={part:point.object.userData.part,vertex:point.index};origin=point.point;}
   }else if(hit){selection={part:hit.object.userData.part,...(latest.current.mode==="face"?{face:hit.object.userData.faces[hit.faceIndex!]}:{})};origin=hit.point;
    if(latest.current.mode==="edge"){const part=latest.current.document.parts.find(p=>p.id===selection!.part)!,face=part.mesh.faces[hit.object.userData.faces[hit.faceIndex!]],box=renderer.domElement.getBoundingClientRect();let best=Infinity,pair:[number,number]|undefined;
     for(let i=0;i<face.length;i++){const a=face[i],b=face[(i+1)%face.length],start=new THREE.Vector3().fromArray(part.mesh.vertices[a]).applyMatrix4(hit.object.matrixWorld).project(camera),end=new THREE.Vector3().fromArray(part.mesh.vertices[b]).applyMatrix4(hit.object.matrixWorld).project(camera);const x=(event.clientX-box.left)/box.width*2-1,y=1-(event.clientY-box.top)/box.height*2,dx=end.x-start.x,dy=end.y-start.y,t=Math.max(0,Math.min(1,((x-start.x)*dx+(y-start.y)*dy)/(dx*dx+dy*dy||1))),distance=Math.hypot((x-start.x-t*dx)*box.width/2,(y-start.y-t*dy)*box.height/2);if(distance<best){best=distance;pair=[a,b];}}
     if(best>14||!pair)return;selection.edge=pair;origin=new THREE.Vector3().fromArray(part.mesh.vertices[pair[0]]).add(new THREE.Vector3().fromArray(part.mesh.vertices[pair[1]])).multiplyScalar(0.5).applyMatrix4(hit.object.matrixWorld);
    }}
   if(!selection||!origin)return;
   event.stopImmediatePropagation();renderer.domElement.focus();if(event.shiftKey||latest.current.mode!=="object"||!latest.current.selectedParts.includes(selection.part)||latest.current.selectedParts.length<2)latest.current.onSelect(selection,event.shiftKey&&latest.current.mode==="object");if(event.shiftKey&&latest.current.mode==="object"){event.preventDefault();return;}begin(selection,origin,new THREE.Vector2(event.clientX,event.clientY),event.pointerId);renderer.domElement.setPointerCapture(event.pointerId);event.preventDefault();
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
  gizmo.addEventListener("mouseDown",()=>{const selection=latest.current.selection;if(!selection)return;const mesh=meshes.get(selection.part);if(!mesh)return;const origin=latest.current.mode==="object"&&latest.current.selectedParts.length===1?mesh.getWorldPosition(new THREE.Vector3()):proxy.position.clone();begin(selection,origin,new THREE.Vector2());});
  gizmo.addEventListener("objectChange",()=>{if(!drag||drag.pointer!==undefined)return;
   if(latest.current.mode==="object"){
    if(latest.current.selectedParts.length>1){const action=latest.current.transformMode,values=action==="translate"?proxy.position.clone().sub(drag.proxy).toArray():action==="rotate"?[proxy.rotation.x,proxy.rotation.y,proxy.rotation.z].map(n=>n*180/Math.PI):proxy.scale.toArray();try{const next=transformStructureParts(drag.base,latest.current.selectedParts,action,values);drag.current=next;drag.changed=true;for(const id of selectedPartRoots(next.parts,latest.current.selectedParts))updatePart(next,{part:id});}catch(error){latest.current.onError(error instanceof Error?error.message:"Group transform failed.");}}
    else{const next=structuredClone(drag.base),part=next.parts.find(part=>part.id===drag!.selection.part)!,mesh=drag.part;part.transform={translation:mesh.position.toArray(),rotation_degrees:[mesh.rotation.x,mesh.rotation.y,mesh.rotation.z].map(n=>n*180/Math.PI),scale:mesh.scale.toArray()};drag.current=next;drag.changed=true;}
   }
   else translate(proxy.position.clone().sub(drag.proxy));
  });
  gizmo.addEventListener("mouseUp",()=>finish());gizmo.addEventListener("dragging-changed",event=>{orbit.enabled=event.value!==true;});
  function context(event:MouseEvent){if(latest.current.mode!=="object"||latest.current.selectedParts.length<2)return;setRay(event as PointerEvent);const hit=ray.intersectObjects(targets,false)[0],id=hit?.object.userData.part;if(!id||latest.current.selectedParts.includes(id)){event.preventDefault();latest.current.onContextMenu?.(id??null);}}
  renderer.domElement.addEventListener("contextmenu",context);renderer.domElement.addEventListener("pointerdown",down,true);renderer.domElement.addEventListener("pointermove",move,true);renderer.domElement.addEventListener("pointerup",up,true);renderer.domElement.addEventListener("pointercancel",up,true);renderer.domElement.addEventListener("keydown",key);
  const resize=new ResizeObserver(()=>{const width=element.clientWidth,height=element.clientHeight;renderer.setSize(width,height);perspective.aspect=width/Math.max(1,height);perspective.updateProjectionMatrix();if(lastAxis)frameAxis(lastAxis);});resize.observe(element);
  function render(){frame=requestAnimationFrame(render);orbit.update();renderer.render(scene,camera);}render();runtime.current={rebuild,focus,axis:axisView};rebuild();
  return()=>{cancelAnimationFrame(frame);resize.disconnect();runtime.current=null;if(drag)latest.current.onBusy(false);renderer.domElement.removeEventListener("contextmenu",context);gizmo.dispose();orbit.dispose();disposeTree(content);grid.geometry.dispose();(grid.material as THREE.Material).dispose();renderer.dispose();renderer.domElement.remove();};
 },[]);
 return <div className="structure-viewport" ref={host} data-parts={props.document.parts.length} data-part={props.selection?.part??""} data-face={props.selection?.face??""} data-vertex={props.selection?.vertex??""} data-edge={props.selection?.edge?.join(",")??""}>{error&&<p role="alert">{error}</p>}<div className="structure-axis-views" role="group" aria-label="Orthographic views">{(["X","Y","Z"] as const).map(axis=><button key={axis} type="button" aria-label={`View along ${axis} axis; click again for opposite side`} onClick={()=>runtime.current?.axis(axis)}>{axis}</button>)}</div><div className="structure-view-help">Select and drag · XYZ handles · Empty space / Alt-drag to orbit · Scroll to zoom · F to frame<button type="button" className="quiet-button" onClick={()=>runtime.current?.focus()}>Frame object</button></div></div>;
}
