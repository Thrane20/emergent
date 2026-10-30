import assert from "node:assert/strict";
import test from "node:test";
import {circleOpacity,paintTerrain,paintTerrainStroke,terrainHeights,terrainGlbHeights,type TerrainBrush} from "../src/lib/terrain-brush";
import {DEFAULT_TERRAIN,terrainGlb} from "../src/lib/terrain";
const settings={...DEFAULT_TERRAIN,resolution:32,size:31,height:20},centre=16*32+16;
const brush:TerrainBrush={mode:"raise",radius:4,strength:1,height:12};
function paint(values:Float32Array,b:Partial<TerrainBrush>={},x=0.5,z=0.5){paintTerrain(values,new Float32Array(values.length),settings,{...brush,...b},x,z,0.1);}

test("circular grayscale opacity and strength scale height shifts; black/outside samples stay untouched",()=>{
 assert.equal(circleOpacity(0),1);assert.equal(circleOpacity(0.5),0.75);assert.equal(circleOpacity(1),0);assert.equal(circleOpacity(2),0);
 const full=new Float32Array(1024).fill(10),half=full.slice(),zero=full.slice();paint(full);paint(half,{strength:0.5});paint(zero,{strength:0});
 assert.equal(full[centre],10.5);assert.equal(half[centre],10.25);assert.equal(zero[centre],10);
 assert.ok(full[centre+1]<full[centre]);assert.equal(full[centre+4],10);assert.equal(full[0],10);
 paint(full,{mode:"lower"});assert.equal(full[centre],10);
 const top=new Float32Array(1024).fill(20);paint(top);assert.equal(top[centre],20);
 const bottom=new Float32Array(1024);paint(bottom,{mode:"lower"});assert.equal(bottom[centre],0);
});

test("smooth uses an unchanged neighbourhood, flatten approaches the requested height, and edges are safe",()=>{
 const values=new Float32Array(1024);values[centre]=18;paint(values,{mode:"smooth"});assert.ok(values[centre]<18);assert.ok(values[centre+1]>0);assert.equal(values[centre-1],values[centre+1]);assert.equal(values[0],0);
 const flat=new Float32Array(1024).fill(2);for(let i=0;i<100;i++)paint(flat,{mode:"flatten",height:12});assert.ok(Math.abs(flat[centre]-12)<0.001);assert.equal(flat[0],2);
 paint(flat,{mode:"smooth"},-15.5,-15.5);assert.ok(flat.every(Number.isFinite));
 const before=flat.slice();assert.throws(()=>paint(flat,{strength:NaN}));assert.deepEqual(flat,before);
});

test("editable fields roundtrip the actual GLB; save validation rejects malformed heights",()=>{
 const field=Float32Array.from({length:1024},(_,i)=>i/1024*20);const bytes=terrainGlb(settings,field);
 assert.deepEqual(terrainGlbHeights(bytes.buffer,settings),field);assert.deepEqual(terrainHeights(Array.from(field),settings),field);
 for(const value of [[],new Array(1024).fill(null),new Array(1024).fill("1"),new Array(1024).fill(-1),new Array(1024).fill(21),new Array(1024).fill(NaN)])assert.throws(()=>terrainHeights(value,settings));
});

test("a quick default-strength drag is visible at 128 resolution and covers its final segment",()=>{
 const s={...DEFAULT_TERRAIN,resolution:128},values=new Float32Array(128**2).fill(9),scratch=new Float32Array(values.length),b={...brush,radius:10,strength:0.5};
 paintTerrainStroke(values,scratch,s,b,-30,0,30,0,0.016);
 const change=(x:number)=>Math.max(...Array.from({length:128},(_,z)=>values[z*128+x]-9));
 assert.ok(change(64)>s.height*8/255,"normal fast drags must shift more than eight grayscale levels");
 assert.ok(change(100)>s.height*8/255,"the final segment must be painted before pointer release");
 assert.equal(values[0],9);
 const zero=new Float32Array(values.length).fill(9);paintTerrainStroke(zero,scratch,s,{...b,strength:0},-30,0,30,0,0.016);assert.ok(zero.every(v=>v===9));
 const split=new Float32Array(values.length).fill(9);
 for(let x=-30;x<30;x+=2.5)paintTerrainStroke(split,scratch,s,b,x,0,x+2.5,0,0.001);
 assert.ok(Math.abs(change(64)-Math.max(...Array.from({length:128},(_,z)=>split[z*128+64]-9)))<0.05,"event frequency should not weaken a straight drag");
});
