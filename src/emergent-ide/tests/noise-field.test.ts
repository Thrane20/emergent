import assert from "node:assert/strict";
import test from "node:test";
import {parse} from "smol-toml";
import {assetContents,assetTemplate,sceneCompatible} from "../src/lib/asset-catalog";
import {DEFAULT_NOISE,decodeNoiseField,noiseSettings,noiseSource,replaceNoiseSettings} from "../src/lib/noise-field";
function field(width=2,depth=1){const data=new ArrayBuffer(16+width*width*depth*4),view=new DataView(data);view.setUint32(0,0x454e4631);view.setUint32(4,width,true);view.setUint32(8,width,true);view.setUint32(12,depth,true);for(let i=16;i<data.byteLength;i+=4)view.setFloat32(i,((i-16)/4)%4/3,true);return data;}
test("filter templates and settings are versioned, bounded, and safe to update without touching metadata",()=>{
 const template=assetTemplate("procedural.noise")!;assert.equal(template.category,"Filters");assert.equal(sceneCompatible(template),false);
 const source=assetContents(template,"Test noise").replace("[noise]","# retain metadata note\n[noise]\n# retain setting note");assert.equal(noiseSource(source).settings.algorithm,"perlin");
 const updated=replaceNoiseSettings(source,{...DEFAULT_NOISE,seed:42,algorithm:"worley",dimensions:3,size:32});assert.match(updated,/# retain metadata note/);assert.match(updated,/# retain setting note/);assert.equal(parse(updated).id,"test-noise");assert.equal(noiseSource(updated).settings.seed,42);
 for(const value of [{...DEFAULT_NOISE,seed:-1},{...DEFAULT_NOISE,seed:4294967296},{...DEFAULT_NOISE,dimensions:3,size:128},{...DEFAULT_NOISE,frequency:NaN},{...DEFAULT_NOISE,offset:[Infinity,0,0]},{...DEFAULT_NOISE,algorithm_version:2},{...DEFAULT_NOISE,algorithm:"unknown"},{...DEFAULT_NOISE,toString:1}])assert.throws(()=>noiseSettings(value));
});
test("dense noise transport validates shape, samples and all volume slices",()=>{
 const two=decodeNoiseField(field());assert.equal(two.depth,1);assert.equal(two.min,0);assert.equal(two.max,1);
 const three=decodeNoiseField(field(2,2));assert.equal(three.values.length,8);assert.equal(three.depth,2);
 assert.throws(()=>decodeNoiseField(new ArrayBuffer(0)),/length/);
 const bad=field();new DataView(bad).setFloat32(16,NaN,true);assert.throws(()=>decodeNoiseField(bad),/finite/);
 const shape=field();new DataView(shape).setUint32(12,123,true);assert.throws(()=>decodeNoiseField(shape),/dimensions/);
});
