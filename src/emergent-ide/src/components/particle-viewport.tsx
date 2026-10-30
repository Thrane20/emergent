"use client";
import {spriteFrameAt} from "@/lib/image-library";
import {useEffect,useRef,useState} from "react";
import * as THREE from "three";
import {OrbitControls} from "three/addons/controls/OrbitControls.js";
import {ParticleSimulation,type ParticleDefinition} from "@/lib/particles";
type Props={definition:ParticleDefinition;projectId:string;playing:boolean;restart:number;step:number;onTime:(seconds:number,count:number)=>void};
export function ParticleViewport(props:Props){
 const host=useRef<HTMLDivElement>(null),latest=useRef(props),runtime=useRef<((definition:ParticleDefinition)=>void)|null>(null),[error,setError]=useState("");useEffect(()=>{latest.current=props;},[props]);
 useEffect(()=>{
 const element=host.current!;let renderer:THREE.WebGLRenderer;
 // eslint-disable-next-line react-hooks/set-state-in-effect -- GPU availability is known only after mount.
 try{renderer=new THREE.WebGLRenderer({antialias:true});}catch{setError("WebGL is unavailable. Native capture remains available.");return;}
 renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.setClearColor(0x172129);renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.domElement.setAttribute("aria-label","Particle 3D preview");renderer.domElement.tabIndex=0;element.append(renderer.domElement);
 const scene=new THREE.Scene(),camera=new THREE.PerspectiveCamera(45,1,.05,200);camera.position.set(4,3,6);const controls=new OrbitControls(camera,renderer.domElement);controls.target.set(0,1.5,0);controls.enableDamping=true;
 scene.add(new THREE.GridHelper(12,24,0x52616c,0x303e49));const axes=new THREE.AxesHelper(.6);scene.add(axes);
 const canvas=document.createElement("canvas");canvas.width=64;canvas.height=64;const context=canvas.getContext("2d")!;const gradient=context.createRadialGradient(32,32,0,32,32,32);gradient.addColorStop(0,"rgba(255,255,255,1)");gradient.addColorStop(.4,"rgba(255,255,255,.5)");gradient.addColorStop(1,"rgba(255,255,255,0)");context.fillStyle=gradient;context.fillRect(0,0,64,64);const procedural=new THREE.CanvasTexture(canvas);procedural.colorSpace=THREE.SRGBColorSpace;
 const loader=new THREE.TextureLoader(),textures:THREE.Texture[]=[];
 const build=(definition:ParticleDefinition)=>definition.emitters.map(e=>{
 const sim=new ParticleSimulation(e),geometry=new THREE.InstancedBufferGeometry();geometry.setAttribute("position",new THREE.Float32BufferAttribute([-.5,-.5,0,.5,-.5,0,.5,.5,0,-.5,-.5,0,.5,.5,0,-.5,.5,0],3));
 const attrs={center:new THREE.InstancedBufferAttribute(new Float32Array(e.max_particles*3),3),tint:new THREE.InstancedBufferAttribute(new Float32Array(e.max_particles*4),4),params:new THREE.InstancedBufferAttribute(new Float32Array(e.max_particles*3),3)};
 for(const [key,value]of Object.entries(attrs))geometry.setAttribute(key,value);geometry.instanceCount=0;
 let texture:THREE.Texture=procedural;if(e.texture){texture=loader.load(`/api/project-asset?projectId=${encodeURIComponent(props.projectId)}&path=${encodeURIComponent(e.texture)}`,undefined,undefined,()=>setError("A particle texture could not be loaded. Check its asset path."));texture.colorSpace=THREE.SRGBColorSpace;textures.push(texture);}
 const material=new THREE.ShaderMaterial({transparent:true,depthWrite:false,depthTest:true,blending:THREE.CustomBlending,blendEquation:THREE.AddEquation,blendSrc:e.blend==="multiply"?THREE.DstColorFactor:THREE.OneFactor,blendDst:e.blend==="screen"?THREE.OneMinusSrcColorFactor:e.blend==="additive"?THREE.OneFactor:THREE.OneMinusSrcAlphaFactor,blendSrcAlpha:THREE.OneFactor,blendDstAlpha:THREE.OneMinusSrcAlphaFactor,uniforms:{sprite:{value:texture},grid:{value:new THREE.Vector2(e.columns,e.rows)}},vertexShader:`attribute vec3 center;attribute vec4 tint;attribute vec3 params;varying vec2 spriteUV;varying vec4 color;uniform vec2 grid;void main(){float c=cos(params.y),s=sin(params.y);vec2 q=mat2(c,s,-s,c)*position.xy*params.x;vec4 p=modelViewMatrix*vec4(center,1.);p.xy+=q;gl_Position=projectionMatrix*p;float f=params.z;spriteUV=(vec2(mod(f,grid.x),floor(f/grid.x))+vec2(position.x+.5,.5-position.y))/grid;color=tint;}`,fragmentShader:`uniform sampler2D sprite;varying vec2 spriteUV;varying vec4 color;void main(){vec4 t=texture2D(sprite,spriteUV);gl_FragColor=t*color;#include <tonemapping_fragment>\n#include <colorspace_fragment>\ngl_FragColor.rgb*=gl_FragColor.a;}`.replace(';#include',';\n#include')});
 const mesh=new THREE.Mesh(geometry,material);mesh.frustumCulled=false;mesh.renderOrder=e.blend==="additive"?2:1;scene.add(mesh);return {sim,geometry,attrs,material,mesh};
 });
 let emitters=build(latest.current.definition);
 let frame=0,previous=performance.now(),accumulator=0,elapsed=0,lastReport=0,step=latest.current.step,disposed=false;
 runtime.current=(definition)=>{emitters.forEach(v=>{scene.remove(v.mesh);v.geometry.dispose();v.material.dispose();});textures.forEach(t=>t.dispose());textures.length=0;emitters=build(definition);elapsed=0;accumulator=0;previous=performance.now();lastReport=0;};
 // shortcut: browser preview sorts per emitter; native capture supplies cross-emitter sorting and soft depth fading.
 const draw=(now:number)=>{const delta=Math.min(.1,(now-previous)/1000);previous=now;let steps=0;if(latest.current.playing){accumulator+=delta;while(accumulator>=1/60&&steps<6){accumulator-=1/60;steps++;}}else if(step!==latest.current.step){steps=1;step=latest.current.step;}if(steps){for(let i=0;i<steps;i++){emitters.forEach(v=>v.sim.step());elapsed+=1/60;}}
 let count=0;for(const {sim,geometry,attrs}of emitters){const e=sim.emitter;const distance=(p:number[])=> (p[0]-camera.position.x)**2+(p[1]-camera.position.y)**2+(p[2]-camera.position.z)**2;sim.particles.sort((a,b)=>distance(b.position)-distance(a.position));for(let i=0;i<sim.particles.length;i++){const p=sim.particles[i],t=p.age/p.life;attrs.center.setXYZ(i,...p.position as [number,number,number]);attrs.tint.setXYZW(i,...e.color_start.map((v,c)=>v*(1-t)+e.color_end[c]*t) as [number,number,number,number]);attrs.params.setXYZ(i,e.size[0]*(1-t)+e.size[1]*t,p.rotation,spriteFrameAt(p.frame+p.age*e.fps,e.frames,e.ping_pong));}Object.values(attrs).forEach(a=>a.needsUpdate=true);geometry.instanceCount=sim.particles.length;count+=sim.particles.length;}
 controls.update();renderer.render(scene,camera);if(now-lastReport>100){latest.current.onTime(elapsed,count);lastReport=now;}if(!disposed)frame=requestAnimationFrame(draw);};
 const resize=new ResizeObserver(()=>{const {width,height}=element.getBoundingClientRect();renderer.setSize(Math.max(width,1),Math.max(height,1));camera.aspect=Math.max(width,1)/Math.max(height,1);camera.updateProjectionMatrix();});resize.observe(element);frame=requestAnimationFrame(draw);
 return()=>{runtime.current=null;disposed=true;cancelAnimationFrame(frame);resize.disconnect();controls.dispose();emitters.forEach(v=>{v.geometry.dispose();v.material.dispose();});textures.forEach(t=>t.dispose());procedural.dispose();scene.traverse(o=>{if(o instanceof THREE.LineSegments){o.geometry.dispose();(o.material as THREE.Material).dispose();}});renderer.dispose();renderer.forceContextLoss();renderer.domElement.remove();};
 },[props.projectId]);
 useEffect(()=>{runtime.current?.(props.definition);},[props.definition,props.restart]);
 return <div ref={host} className="particle-viewport">{error&&<p role="alert">{error}</p>}</div>;
}
