import assert from "node:assert/strict";
import test from "node:test";
import {DEFAULT_TERRAIN} from "../src/lib/terrain";
import {heightMaterialWeights,paintMaterialStroke,quantizeWeights,terrainPaint} from "../src/lib/terrain-paint";
test("height painting selects beds, ground, mountain and summit with normalized soft transitions",()=>{
 const heights=Float32Array.from([0,5,15,25,35]);
 const weights=heightMaterialWeights(heights,[5,15,25],2,4);
 assert.deepEqual(weights.slice(0,4),[255,0,0,0]);assert.deepEqual(weights.slice(-4),[0,0,0,255]);
 assert.ok(Math.abs(weights[4]-weights[5])<=1);
 for(let i=0;i<weights.length;i+=4)assert.equal(weights.slice(i,i+4).reduce((a,b)=>a+b),255);
 assert.deepEqual(heightMaterialWeights(Float32Array.from([0,50]),[5,15,25],0,1),[255,0,0,0,255,0,0,0]);
 assert.throws(()=>heightMaterialWeights(heights,[10,5,20],2,4));assert.throws(()=>heightMaterialWeights(heights,[5,15,25],-1,4));
});
test("material brush accumulates weak strokes, keeps a fast drag continuous, and preserves normalized weights",()=>{
 const settings={...DEFAULT_TERRAIN,resolution:32,size:31};
 const values=Float32Array.from({length:32*32*4},(_,i)=>i%4===0?1:0);
 paintMaterialStroke(values,settings,1,3,0.5,-10,0,10,0,0.001);
 for(let col=8;col<24;col++)assert.ok(values[(16*32+col)*4+1]>0.05,"gap in fast drag");
 const before=values.slice();paintMaterialStroke(values,settings,2,3,0,0,0,0,0,0.1);assert.deepEqual(values,before);
 for(let i=0;i<200;i++)paintMaterialStroke(values,settings,2,3,0.001,0,0,0,0,0.1);
 assert.ok(values[(16*32+16)*4+2]>0.01);
 const bytes=quantizeWeights(values);terrainPaint({schema_version:1,resolution:32,materials:["a.material.toml","b.material.toml","c.material.toml"],transitions:[5,15,25],blend:2,weights:bytes},32);
 assert.throws(()=>paintMaterialStroke(values,settings,9,3,1,0,0,0,0,0.1));assert.throws(()=>paintMaterialStroke(values,settings,1,0,1,0,0,0,0,0.1));
 const paint={schema_version:1,resolution:32,materials:["a.material.toml"],transitions:[5,15,25],blend:2,weights:bytes};assert.throws(()=>terrainPaint(paint,32));assert.throws(()=>terrainPaint({...paint,materials:["../bad.png"]},32));
});
