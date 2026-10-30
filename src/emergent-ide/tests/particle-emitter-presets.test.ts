import assert from "node:assert/strict";
import test from "node:test";
import {parse,stringify} from "smol-toml";
import {PARTICLE_EMITTER_PRESETS,particleEmitterPreset} from "../src/lib/particle-emitter-presets";
import {DEFAULT_EMITTER,particleDefinition,ParticleSimulation} from "../src/lib/particles";

test("20 distinct emitter types validate, round-trip and simulate within bounded finite pools",()=>{
 assert.equal(PARTICLE_EMITTER_PRESETS.length,20);assert.equal(new Set(PARTICLE_EMITTER_PRESETS.map(p=>p.id)).size,20);
 for(const preset of PARTICLE_EMITTER_PRESETS){
  const emitter=particleEmitterPreset(preset.id),definition=particleDefinition({schema_version:1,id:"preset",name:preset.name,emitters:[emitter]});assert.equal(emitter.name,preset.name);assert.equal(emitter.texture,undefined);assert.deepEqual(particleDefinition(parse(stringify(definition))),definition);
  const a=new ParticleSimulation(emitter),b=new ParticleSimulation(particleEmitterPreset(preset.id));let peak=0;
  for(let step=0;step<600;step++){a.step();b.step();peak=Math.max(peak,a.particles.length);assert.ok(a.particles.length<=emitter.max_particles);assert.ok(a.particles.every(p=>p.position.every(Number.isFinite)&&p.velocity.every(Number.isFinite)));}
  assert.ok(peak>0,preset.name);assert.deepEqual(a.particles,b.particles);if(!emitter.looping)assert.equal(a.particles.length,0,preset.name);
 }
});
test("preset copies are independent and falling types fall while fountains rise",()=>{
 const defaults=structuredClone(DEFAULT_EMITTER),a=particleEmitterPreset("rain"),b=particleEmitterPreset("rain");a.gravity[1]=99;a.color_start[0]=0;assert.equal(b.gravity[1],-18);assert.notEqual(b.color_start[0],0);assert.deepEqual(DEFAULT_EMITTER,defaults);assert.throws(()=>particleEmitterPreset("missing"),/Unknown/);
 for(const id of ["rain","snow","leaves","ash"]){const simulation=new ParticleSimulation(particleEmitterPreset(id));for(let i=0;i<30;i++)simulation.step();assert.ok(simulation.particles.some(p=>p.velocity[1]<0),id);}
 const fountain=new ParticleSimulation(particleEmitterPreset("fountain"));fountain.step();assert.ok(fountain.particles.every(p=>p.velocity[1]>0));
});
