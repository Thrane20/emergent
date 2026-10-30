import assert from "node:assert/strict";
import test from "node:test";
import { DEFAULT_TERRAIN, terrainSettings, decodeTerrainFields, terrainComparison, terrainGlb } from "../src/lib/terrain";
import { validateGlb } from "../src/lib/object-files";
const settings={...DEFAULT_TERRAIN,resolution:32,seed:0};
const field=new Float32Array(32*32).fill(0.5);

test("terrain inputs bound workload and reject invalid numeric settings",()=>{
 assert.deepEqual(terrainSettings({}),DEFAULT_TERRAIN);assert.equal(terrainSettings({algorithmVersion:1}).algorithmVersion,1);
 for(const invalid of [{seed:-1},{seed:4294967296},{resolution:2048},{resolution:"64"},{octaves:9},{height:Infinity},{height:-1},{size:0},{frequency:0},{roughness:2},{preset:"erosion"},{mountainCount:0},{mountainCount:13},{mountainCount:2.5},{sunStrength:2},{sunStrength:-1},{sunElevation:0},{sunElevation:90},{sunAzimuth:361},{extra:1}])assert.throws(()=>terrainSettings(invalid));
 assert.throws(()=>terrainGlb(settings,new Float32Array(3)),/height field/);
 const corrupt=field.slice();corrupt[3]=NaN;assert.throws(()=>terrainGlb(settings,corrupt),/height field/);
 for(const invalid of [{algorithmVersion:3},{generator:"other"},{domainWarp:-1},{ridgeStrength:2},{erosionIterations:65},{erosionIterations:1.2}])assert.throws(()=>terrainSettings(invalid));
});

test("GLB has upward triangles, unit smooth normals, grayscale vertices and finite bounds",()=>{
 const ramp=Float32Array.from({length:1024},(_,i)=>((i%32)+Math.floor(i/32))/62);
 const bytes=terrainGlb(settings,ramp);validateGlb(Buffer.from(bytes));assert.deepEqual(bytes,terrainGlb(settings,ramp));
 const header=new DataView(bytes.buffer),length=header.getUint32(12,true),gltf=JSON.parse(new TextDecoder().decode(bytes.slice(20,20+length))),data=new DataView(bytes.buffer,28+length);
 assert.deepEqual(gltf.asset.extras.terrainSettings,settings);assert.equal(gltf.accessors[0].count,1024);assert.equal(gltf.accessors[3].count,31*31*6);
 const attributeBytes=1024*12;
 for(let i=0;i<1024;i++){
  const nx=data.getFloat32(attributeBytes+i*12,true),ny=data.getFloat32(attributeBytes+i*12+4,true),nz=data.getFloat32(attributeBytes+i*12+8,true);
  assert.ok(Math.abs(Math.hypot(nx,ny,nz)-1)<1e-5);assert.ok(ny>0);
  const gray=data.getFloat32(attributeBytes*2+i*12,true);assert.ok(gray>=0&&gray<=1);assert.equal(gray,data.getFloat32(attributeBytes*2+i*12+4,true));assert.equal(gray,data.getFloat32(attributeBytes*2+i*12+8,true));
 }
 const vertex=(index:number)=>[data.getFloat32(index*12,true),data.getFloat32(index*12+4,true),data.getFloat32(index*12+8,true)];
 for(let i=0;i<31*31*6;i+=3){
  const indices=[0,1,2].map(offset=>data.getUint32(attributeBytes*3+(i+offset)*4,true));assert.ok(indices.every(index=>index<1024));const [a,b,c]=indices.map(vertex);
  assert.ok((b[2]-a[2])*(c[0]-a[0])-(b[0]-a[0])*(c[2]-a[2])>0);
 }
 assert.equal(gltf.extensions.KHR_lights_punctual.lights[0].type,"directional");
 assert.equal(gltf.nodes[1].extensions.KHR_lights_punctual.light,0);assert.ok(gltf.scenes[0].nodes.includes(1));
 const [qx,qy,qz,qw]=gltf.nodes[1].rotation;assert.ok(Math.abs(Math.hypot(qx,qy,qz,qw)-1)<1e-6);
 assert.ok(2*(qw*qx-qy*qz)<0,"sunlight points downwards");
 assert.equal(gltf.images,undefined);assert.equal(gltf.textures,undefined);
 const flat=terrainGlb({...settings,height:0},new Float32Array(1024));validateGlb(Buffer.from(flat));
});


test("preview sun controls set the directional light and preserve terrain geometry",()=>{
 const decode=(bytes:Uint8Array)=>{const header=new DataView(bytes.buffer),length=header.getUint32(12,true);return {json:JSON.parse(new TextDecoder().decode(bytes.slice(20,20+length))),geometry:bytes.slice(28+length)};};
 const lit=decode(terrainGlb({...settings,sunStrength:1,sunAzimuth:315,sunElevation:35},field)),off=decode(terrainGlb({...settings,sunStrength:0,sunAzimuth:90,sunElevation:70},field));
 assert.equal(lit.json.extensions.KHR_lights_punctual.lights[0].intensity,5);assert.equal(off.json.extensions.KHR_lights_punctual.lights[0].intensity,0);
 assert.notDeepEqual(lit.json.nodes[1].rotation,off.json.nodes[1].rotation);assert.deepEqual(lit.geometry,off.geometry);
});

test("Rust terrain protocol checks counts, resolution, byte length and finite heights",()=>{
 const bytes=new ArrayBuffer(8+12+1024*4),view=new DataView(bytes);view.setUint32(0,0x31465445,true);view.setUint32(4,1,true);view.setBigUint64(8,BigInt(2500000),true);view.setUint32(16,32,true);
 const [field]=decodeTerrainFields(bytes,settings,false);assert.equal(field.values.length,1024);assert.equal(field.generationMs,2.5);
 assert.throws(()=>decodeTerrainFields(bytes.slice(0,-1),settings,false),/Truncated/);
 view.setFloat32(20,NaN,true);assert.throws(()=>decodeTerrainFields(bytes,settings,false),/height/);view.setFloat32(20,0,true);
 view.setUint32(16,64,true);assert.throws(()=>decodeTerrainFields(bytes,settings,false),/resolution/);
 assert.throws(()=>decodeTerrainFields(bytes,settings,true),/header/);
 const recipes=terrainComparison(settings);assert.deepEqual(recipes[2],settings);assert.equal(recipes[0].algorithmVersion,1);assert.equal(recipes[1].algorithmVersion,1);assert.equal(recipes[2].algorithmVersion,2);assert.equal(recipes[0].erosionIterations,0);assert.equal(recipes[1].generator,"diamond-square");
 for(const recipe of recipes)for(const key of ["seed","resolution","size","height","sunStrength","sunAzimuth","sunElevation"] as const)assert.equal(recipe[key],settings[key]);
});

test("512 and 1024 terrains retain every height and 32-bit mesh index",()=>{
 for(const resolution of [512,1024]){
  const settings=terrainSettings({resolution}),count=resolution**2,bytes=terrainGlb(settings,new Float32Array(count));
  validateGlb(Buffer.from(bytes));
  const header=new DataView(bytes.buffer),length=header.getUint32(12,true),gltf=JSON.parse(new TextDecoder().decode(bytes.subarray(20,20+length))),data=new DataView(bytes.buffer,28+length);
  assert.equal(gltf.accessors[0].count,count);assert.equal(gltf.accessors[3].componentType,5125);assert.equal(gltf.accessors[3].count,(resolution-1)**2*6);
  const indices=gltf.bufferViews[gltf.accessors[3].bufferView];
  let maximum=0;for(let offset=0;offset<indices.byteLength;offset+=4)maximum=Math.max(maximum,data.getUint32(indices.byteOffset+offset,true));
  assert.equal(maximum,count-1);
 }
});
