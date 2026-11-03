"use client";
import {useEffect,useRef,useState} from "react";
import * as THREE from "three";
import {OrbitControls} from "three/examples/jsm/controls/OrbitControls.js";
import {shaderGLSL,shaderValues,type ShaderGraph} from "@/lib/shader-graph";
export function graphMaterial(graph:ShaderGraph,overrides:Record<string,number>={}){
 const code=shaderGLSL(graph);
 const output=graph.nodes.find(n=>n.id===graph.outputs.displacement),displaced=output?.op!=="constant"||output?.value!==0;
 return new THREE.ShaderMaterial({transparent:graph.transparent,depthWrite:!graph.transparent,side:THREE.DoubleSide,uniforms:{shaderTime:{value:0},shaderParameters:{value:shaderValues(graph,overrides)}},vertexShader:`${code}
 varying vec3 localPosition;varying vec3 worldPosition;varying vec3 worldNormal;varying vec2 texCoord;
 void main(){SurfaceGraph g=surfaceGraph(position,uv,1.0);vec3 p=position+normal*g.displacement;vec4 world=modelMatrix*vec4(p,1.0);localPosition=position;worldPosition=world.xyz;worldNormal=normalize(mat3(modelMatrix)*normal);texCoord=uv;gl_Position=projectionMatrix*viewMatrix*world;}`,
 fragmentShader:`${code}
 varying vec3 localPosition;varying vec3 worldPosition;varying vec3 worldNormal;varying vec2 texCoord;
 void main(){vec3 normal=normalize(worldNormal);${displaced?"normal=normalize(cross(dFdx(worldPosition),dFdy(worldPosition)));if(dot(normal,worldNormal)<0.0)normal=-normal;":""}float facing=dot(normal,normalize(cameraPosition-worldPosition));SurfaceGraph g=surfaceGraph(localPosition,texCoord,facing);if(g.opacity<0.001)discard;gl_FragColor=vec4(g.color*(0.6+0.4*abs(dot(normal,normalize(vec3(1.0,2.0,3.0)))))+g.color*g.emission,g.opacity);
 #include <tonemapping_fragment>
 #include <colorspace_fragment>
 }`});
}
export function ShaderPreview({graph,shape,playing,time,restart,onError}:{graph:ShaderGraph;shape:"sphere"|"plane"|"cube";playing:boolean;time:number;restart:number;onError:(message:string)=>void}){
 const host=useRef<HTMLDivElement>(null),meshRef=useRef<THREE.Mesh|null>(null),clockState=useRef({playing,time,restart}),[gpuError,setGpuError]=useState("");
 useEffect(()=>{clockState.current={playing,time,restart};},[playing,time,restart]);
 useEffect(()=>{
  const element=host.current!;let renderer:THREE.WebGLRenderer;
  // eslint-disable-next-line react-hooks/set-state-in-effect -- Report device availability after mount.
  try{renderer=new THREE.WebGLRenderer({antialias:true});}catch{setGpuError("WebGL is unavailable.");return;}
  renderer.debug.onShaderError=(_gl,_program,vertex,fragment)=>onError(`Shader compilation failed: ${_gl.getShaderInfoLog(vertex)??""} ${_gl.getShaderInfoLog(fragment)??""}`);
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.setClearColor(0x171f2a);renderer.outputColorSpace=THREE.SRGBColorSpace;
  renderer.domElement.setAttribute("aria-label","Live shader preview");renderer.domElement.setAttribute("role","img");element.append(renderer.domElement);
  const scene=new THREE.Scene(),camera=new THREE.PerspectiveCamera(45,1,.05,100);camera.position.set(2,1.5,3);const controls=new OrbitControls(camera,renderer.domElement);controls.enableDamping=true;
  const geometry=shape==="sphere"?new THREE.SphereGeometry(.8,64,48):shape==="plane"?new THREE.PlaneGeometry(3,3,96,96).rotateX(-Math.PI/2):new THREE.BoxGeometry(1.3,1.3,1.3,24,24,24);
  const mesh=new THREE.Mesh(geometry,new THREE.MeshBasicMaterial());meshRef.current=mesh;scene.add(mesh);
  const grid=new THREE.GridHelper(8,16,0x586578,0x303c4b);grid.position.y=-1;scene.add(grid);
  const resize=()=>{const {width,height}=element.getBoundingClientRect();if(!width||!height)return;renderer.setSize(width,height);camera.aspect=width/height;camera.updateProjectionMatrix();};const observer=new ResizeObserver(resize);observer.observe(element);resize();
  let seconds=clockState.current.time,last=performance.now(),lastScrub=clockState.current.time,lastRestart=clockState.current.restart;
  renderer.setAnimationLoop(()=>{const now=performance.now();if(lastScrub!==clockState.current.time||lastRestart!==clockState.current.restart){seconds=clockState.current.time;lastScrub=clockState.current.time;lastRestart=clockState.current.restart;}if(clockState.current.playing)seconds+=Math.min((now-last)/1000,.1);last=now;if(mesh.material instanceof THREE.ShaderMaterial)mesh.material.uniforms.shaderTime.value=seconds%1000000;controls.update();renderer.render(scene,camera);renderer.domElement.dataset.time=String(seconds);});
  return()=>{renderer.setAnimationLoop(null);observer.disconnect();controls.dispose();geometry.dispose();mesh.material.dispose();meshRef.current=null;grid.geometry.dispose();(grid.material as THREE.Material).dispose();renderer.dispose();renderer.forceContextLoss();renderer.domElement.remove();};
 },[shape,onError]);
 useEffect(()=>{const mesh=meshRef.current;if(!mesh)return;const material=graphMaterial(graph);(mesh.material as THREE.Material).dispose();mesh.material=material;},[graph,shape]);
 return <div className="shader-preview" ref={host}>{gpuError&&<p role="alert">{gpuError}</p>}</div>;
}
