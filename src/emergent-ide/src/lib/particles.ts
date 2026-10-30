import {parse} from "smol-toml";
export type ParticleEmitter={name:string;seed:number;max_particles:number;rate:number;burst:number;duration:number;looping:boolean;local_space:boolean;shape:"point"|"cone"|"box"|"sphere";position:number[];extent:number[];speed:number;spread:number;lifetime:number[];gravity:number[];wind:number[];drag:number;size:number[];color_start:number[];color_end:number[];rotation_speed:number;blend:"alpha"|"additive"|"screen"|"multiply";soft_distance:number;texture?:string;columns:number;rows:number;frames:number;fps:number;random_frame:boolean;ping_pong?:boolean};
export type ParticleDefinition={schema_version:1;id:string;name:string;emitters:ParticleEmitter[]};
export const DEFAULT_EMITTER:ParticleEmitter={name:"Smoke",seed:1,max_particles:1000,rate:40,burst:0,duration:5,looping:true,local_space:false,shape:"cone",position:[0,0,0],extent:[.2,.2,.2],speed:1,spread:.3,lifetime:[2,4],gravity:[0,.1,0],wind:[0,0,0],drag:.2,size:[.25,1.5],color_start:[.7,.7,.7,.5],color_end:[.3,.3,.3,0],rotation_speed:.3,blend:"alpha",soft_distance:.3,columns:1,rows:1,frames:1,fps:12,random_frame:false,ping_pong:false};
export function blankParticleSystem():ParticleDefinition{
 return {schema_version:1,id:"draft",name:"Untitled particle system",emitters:[{...structuredClone(DEFAULT_EMITTER),name:"Emitter 1",rate:0,burst:0}]};
}
export function particlePreset(preset:string):ParticleDefinition{
 const smoke=structuredClone(DEFAULT_EMITTER);const fire:ParticleEmitter={...structuredClone(smoke),name:"Flame",seed:2,rate:100,speed:1.4,extent:[.18,.1,.18],lifetime:[.3,.8],size:[.4,.05],color_start:[1,.45,.06,.9],color_end:[1,.05,0,0],blend:"additive",gravity:[0,1,0],drag:.1};
 const embers:ParticleEmitter={...structuredClone(fire),name:"Embers",seed:3,rate:12,speed:2,spread:.6,lifetime:[1,2.5],size:[.035,.01],gravity:[0,.1,0],wind:[.2,0,0]};
 const dust:ParticleEmitter={...structuredClone(smoke),name:"Dust",shape:"box",extent:[1,.1,1],rate:30,speed:.2,lifetime:[1,3],color_start:[.7,.5,.3,.35],size:[.15,.7]};
 return {schema_version:1,id:"draft",name:preset==="fire"?"Campfire":preset==="embers"?"Embers":preset==="dust"?"Drifting dust":"Rising smoke",emitters:preset==="fire"?[fire,{...smoke,position:[0,.4,0]},embers]:preset==="embers"?[embers]:preset==="dust"?[dust]:[smoke]};
}
const printable=(s:unknown)=>typeof s==="string"&&s.trim().length>0&&Array.from(s).length<=64&&!/[\p{Cc}\p{Cf}]/u.test(s);
export function particleDefinition(value:unknown):ParticleDefinition{
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Particle effect is required.");const d=value as ParticleDefinition;
 if(Object.keys(d).some(k=>!["schema_version","id","name","emitters"].includes(k))||d.schema_version!==1||typeof d.id!=="string"||!/^[a-z0-9_.-]{1,128}$/i.test(d.id)||!printable(d.name)||!Array.isArray(d.emitters)||!d.emitters.length||d.emitters.length>16)throw new Error("Use a schema-1 effect with a name and 1–16 emitters.");
 const num=(v:unknown,min:number,max:number,int=false)=>typeof v==="number"&&Number.isFinite(v)&&v>=min&&v<=max&&(!int||Number.isInteger(v));
 const vec=(v:unknown,n:number,min:number,max:number)=>Array.isArray(v)&&v.length===n&&v.every(x=>num(x,min,max));
 const emitters=d.emitters.map(raw=>{
 if(!raw||typeof raw!=="object"||Array.isArray(raw)||Object.keys(raw).some(k=>!Object.hasOwn(DEFAULT_EMITTER,k)&&k!=="texture"))throw new Error("Unknown emitter setting.");
 const e={...structuredClone(DEFAULT_EMITTER),...raw};
 if(!printable(e.name)||!num(e.seed,0,4294967295,true)||!num(e.max_particles,1,100000,true)||!num(e.burst,0,e.max_particles,true)||!num(e.rate,0,100000)||!num(e.duration,1/60,3600)||typeof e.looping!=="boolean"||typeof e.local_space!=="boolean"||!["point","cone","box","sphere"].includes(e.shape)||!["alpha","additive","screen","multiply"].includes(e.blend)||!vec(e.position,3,-10000,10000)||!vec(e.extent,3,0,1000)||!num(e.speed,0,1000)||!num(e.spread,0,Math.PI)||!vec(e.lifetime,2,1/60,120)||e.lifetime[0]>e.lifetime[1]||!vec(e.gravity,3,-1000,1000)||!vec(e.wind,3,-1000,1000)||!num(e.drag,0,100)||!vec(e.size,2,0,1000)||!vec(e.color_start,4,0,1)||!vec(e.color_end,4,0,1)||!num(e.rotation_speed,-100,100)||!num(e.soft_distance,0,10)||!num(e.columns,1,16,true)||!num(e.rows,1,16,true)||!num(e.frames,1,e.columns*e.rows,true)||!num(e.fps,0,120)||typeof e.random_frame!=="boolean"||typeof e.ping_pong!=="boolean")throw new Error("Check emitter bounds, lifetime, colors and flipbook layout.");
 if(e.texture!==undefined&&(typeof e.texture!=="string"||e.texture.length>512||!/^([^/\\:.]+\/)*[^/\\:.]+\.(png|jpe?g)$/i.test(e.texture)))throw new Error("Choose a project PNG/JPEG sprite or atlas.");
 return structuredClone(e);
 });if(emitters.reduce((n,e)=>n+e.max_particles,0)>100000)throw new Error("An effect can reserve at most 100,000 particles.");return {...d,name:d.name.trim(),emitters};
}
export function particleAsset(source:string){return particleDefinition(parse(source));}
export type PreviewParticle={position:number[];velocity:number[];age:number;life:number;rotation:number;frame:number};
export class ParticleSimulation{
 particles:PreviewParticle[]=[];private rng:number;private credit=0;private elapsed=0;private started=false;
 constructor(public emitter:ParticleEmitter){this.rng=emitter.seed;}
 private random(){this.rng=(Math.imul(this.rng,1664525)+1013904223)>>>0;return (this.rng>>>8)/16777216;}
 step(dt=1/60){const e=this.emitter;this.particles=this.particles.filter(p=>{p.age+=dt;if(p.age>=p.life)return false;for(let i=0;i<3;i++){p.velocity[i]=(p.velocity[i]+(e.gravity[i]+e.wind[i])*dt)*Math.exp(-e.drag*dt);p.position[i]+=p.velocity[i]*dt;}p.rotation+=e.rotation_speed*dt;return true;});
 if(e.looping&&this.elapsed>=e.duration){this.elapsed%=e.duration;this.started=false;}let spawn=0;if(e.looping||this.elapsed<e.duration){this.credit+=e.rate*(e.looping?dt:Math.min(dt,Math.max(0,e.duration-this.elapsed)));spawn=Math.floor(this.credit);this.credit-=spawn;if(!this.started){spawn+=e.burst;this.started=true;}}this.elapsed=e.looping?this.elapsed+dt:Math.min(e.duration,this.elapsed+dt);
 spawn=Math.min(spawn,e.max_particles-this.particles.length);for(let n=0;n<spawn;n++){const a=this.random()*Math.PI*2,z=this.random()*2-1,r=Math.sqrt(1-z*z),unit=[r*Math.cos(a),z,r*Math.sin(a)];let offset=[0,0,0];if(e.shape==="box")offset=unit.map(()=>this.random()*2-1);else if(e.shape!=="point"){const radius=Math.cbrt(this.random());offset=unit.map(v=>v*radius);if(e.shape==="cone")offset[1]=radius;}const cos=1-this.random()*(1-Math.cos(e.spread)),sin=Math.sqrt(Math.max(0,1-cos*cos)),direction=[sin*Math.cos(a),cos,sin*Math.sin(a)];this.particles.push({position:offset.map((v,i)=>e.position[i]+v*e.extent[i]),velocity:direction.map(v=>v*e.speed),age:0,life:e.lifetime[0]+this.random()*(e.lifetime[1]-e.lifetime[0]),rotation:this.random()*Math.PI*2,frame:e.random_frame?Math.floor(this.random()*e.frames):0});}
 }
}
