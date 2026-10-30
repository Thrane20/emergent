export const TERRAIN_RESOLUTIONS=[32,64,128,256,512,1024] as const;
export const TERRAIN_DATA_MAX_BYTES=32*1024*1024;
export const TERRAIN_SAVE_MAX_BYTES=64*1024*1024;
export const TERRAIN_COMPARE_MAX_RESOLUTION=256;
export const TERRAIN_PRESETS = [
 {id:"plain",name:"Field / Plain",description:"Open gently undulating ground.",height:12,frequency:0.5,octaves:3,roughness:0.35},
 {id:"canyon",name:"Canyon",description:"A steep winding valley through a raised plateau.",height:70,frequency:0.65,octaves:4,roughness:0.35},
 {id:"underwater",name:"Underwater / Seabed",description:"Low rolling seabed; water rendering comes separately.",height:24,frequency:0.5,octaves:3,roughness:0.4},
 {id:"hilly",name:"Hilly",description:"Broad, gently rolling hills.",height:18,frequency:0.65,octaves:4,roughness:0.45},
 {id:"mountainous",name:"Mountainous",description:"Irregular mountain groups with shoulders and multiple summits.",height:38,frequency:0.65,octaves:4,roughness:0.35},
 {id:"rivers",name:"Rivers",description:"A winding river valley carved through the landscape.",height:16,frequency:0.75,octaves:4,roughness:0.45},
 {id:"island",name:"Island",description:"An elevated interior falling to a flat coastline.",height:26,frequency:0.8,octaves:5,roughness:0.5},
] as const;
export type TerrainSettings={algorithmVersion:number;generator:"noise"|"diamond-square";domainWarp:number;ridgeStrength:number;erosionIterations:number;preset:typeof TERRAIN_PRESETS[number]["id"];seed:number;resolution:number;size:number;height:number;frequency:number;octaves:number;roughness:number;mountainCount:number;sunStrength:number;sunAzimuth:number;sunElevation:number};
export const DEFAULT_TERRAIN:TerrainSettings={algorithmVersion:2,generator:"noise",domainWarp:0.25,ridgeStrength:0.5,erosionIterations:16,preset:"hilly",seed:42,resolution:128,size:100,height:18,frequency:0.65,octaves:4,roughness:0.45,mountainCount:3,sunStrength:0.65,sunAzimuth:315,sunElevation:35};
export function terrainSettings(value:unknown):TerrainSettings {
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Terrain settings are required.");
 const input=value as Record<string,unknown>;
 if(Object.keys(input).some(key=>!Object.hasOwn(DEFAULT_TERRAIN,key)))throw new Error("Unknown terrain setting.");
 const s={...DEFAULT_TERRAIN,...input} as TerrainSettings;
 if(![1,2].includes(s.algorithmVersion))throw new Error("Unsupported terrain algorithm version.");
 if(!["noise","diamond-square"].includes(s.generator))throw new Error("Choose Noise or Diamond-square.");
 if(!TERRAIN_PRESETS.some(preset=>preset.id===s.preset))throw new Error("Choose a supported heightfield biome.");
 if(!TERRAIN_RESOLUTIONS.some(resolution=>resolution===s.resolution))throw new Error("Resolution must be 32, 64, 128, 256, 512 or 1024.");
 for(const [key,min,max] of [["erosionIterations",0,64],["seed",0,4294967295],["octaves",1,8],["mountainCount",1,12]] as const)if(!Number.isInteger(s[key])||s[key]<min||s[key]>max)throw new Error(`${key} must be an integer in ${min}..${max}.`);
 for(const [key,min,max] of [["domainWarp",0,1],["ridgeStrength",0,1],["size",1,10000],["height",0,1000],["frequency",0.1,8],["roughness",0,1],["sunStrength",0,1],["sunAzimuth",0,360],["sunElevation",5,85]] as const)if(typeof s[key]!=="number"||!Number.isFinite(s[key])||s[key]<min||s[key]>max)throw new Error(`${key} must be finite and in ${min}..${max}.`);
 return s;
}

export function terrainComparison(settings:TerrainSettings):TerrainSettings[] {
 const baseline={...settings,algorithmVersion:1,generator:"noise" as const,domainWarp:0,ridgeStrength:0,erosionIterations:0};
 return [baseline,{...baseline,generator:"diamond-square"},settings];
}
export const TERRAIN_COMPARISON_LABELS=["Original noise","Diamond-square","Enhanced pipeline"] as const;
export type TerrainMetrics={generationMs:number;meshMs:number;fieldBytes:number;glbBytes:number};

/** Bounded Rust ETF1 output: count, then elapsed nanoseconds, resolution and f32 heights. */
export function decodeTerrainFields(buffer:ArrayBuffer,settings:TerrainSettings,compare:boolean){
 const view=new DataView(buffer),recipes=compare?terrainComparison(settings):[settings];
 if(buffer.byteLength<8||view.getUint32(0,true)!==0x31465445||view.getUint32(4,true)!==recipes.length)throw new Error("Invalid terrain field header.");
 let offset=8;
 const fields=recipes.map(recipe=>{
  const count=recipe.resolution**2;
  if(offset+12+count*4>buffer.byteLength)throw new Error("Truncated terrain field.");
  const generationMs=Number(view.getBigUint64(offset,true))/1e6;
  if(view.getUint32(offset+8,true)!==recipe.resolution)throw new Error("Terrain resolution mismatch.");offset+=12;
  const values=new Float32Array(count);
  for(let i=0;i<count;i++){
   const value=view.getFloat32(offset+i*4,true);
   if(!Number.isFinite(value)||value<0||value>recipe.height+0.0001)throw new Error("Invalid terrain height.");values[i]=value;
  }
  offset+=count*4;return {settings:recipe,values,generationMs};
 });
 if(offset!==buffer.byteLength)throw new Error("Unexpected terrain field bytes.");
 return fields;
}

/** One untextured grayscale mesh; geometry is prepared once, never in the orbit loop. */
export function terrainGlb(settings:TerrainSettings,heights:Float32Array):Uint8Array<ArrayBuffer> {
 const s=terrainSettings(settings),n=s.resolution,count=n*n,triangles=(n-1)*(n-1)*2,attributeBytes=count*12,indexBytes=triangles*12;
 if(heights.length!==count)throw new Error("Invalid terrain height field.");
 const binaryLength=attributeBytes*3+indexBytes,step=s.size/(n-1);
 let min=Infinity,max=-Infinity;
 for(let i=0;i<count;i++){const value=heights[i];if(!Number.isFinite(value)||value<0||value>s.height+0.0001)throw new Error("Invalid terrain height field.");min=Math.min(min,value);max=Math.max(max,value);}
 // glTF directional lights emit along local -Z; orient from the preview sun controls.
 const pitch=-s.sunElevation*Math.PI/360,yaw=s.sunAzimuth*Math.PI/360;
 const sunRotation=[Math.sin(pitch)*Math.cos(yaw),Math.cos(pitch)*Math.sin(yaw),-Math.sin(pitch)*Math.sin(yaw),Math.cos(pitch)*Math.cos(yaw)];
 const gltf={extensionsUsed:["KHR_lights_punctual"],extensions:{KHR_lights_punctual:{lights:[{name:"Terrain sunlight",type:"directional",color:[1,1,1],intensity:s.sunStrength*5}]}},asset:{version:"2.0",generator:"Emergent Terrain Builder",extras:{terrainSettings:s}},scene:0,scenes:[{nodes:[0,1]}],nodes:[{mesh:0},{name:"Terrain sunlight",rotation:sunRotation,extensions:{KHR_lights_punctual:{light:0}}}],meshes:[{primitives:[{attributes:{POSITION:0,NORMAL:1,COLOR_0:2},indices:3,material:0}]}],materials:[{pbrMetallicRoughness:{metallicFactor:0,roughnessFactor:1}}],buffers:[{byteLength:binaryLength}],bufferViews:[0,1,2].map(index=>({buffer:0,byteOffset:index*attributeBytes,byteLength:attributeBytes,target:34962})).concat([{buffer:0,byteOffset:attributeBytes*3,byteLength:indexBytes,target:34963}]),accessors:[{bufferView:0,componentType:5126,count,type:"VEC3",min:[Math.fround(-s.size/2),min,Math.fround(-s.size/2)],max:[Math.fround(s.size/2),max,Math.fround(s.size/2)]},{bufferView:1,componentType:5126,count,type:"VEC3"},{bufferView:2,componentType:5126,count,type:"VEC3"},{bufferView:3,componentType:5125,count:triangles*3,type:"SCALAR"}]};
 const json=new TextEncoder().encode(JSON.stringify(gltf)),jsonLength=Math.ceil(json.length/4)*4,result=new Uint8Array(28+jsonLength+binaryLength),header=new DataView(result.buffer);
 header.setUint32(0,0x46546c67,true);header.setUint32(4,2,true);header.setUint32(8,result.length,true);header.setUint32(12,jsonLength,true);header.setUint32(16,0x4e4f534a,true);result.fill(32,20,20+jsonLength);result.set(json,20);header.setUint32(20+jsonLength,binaryLength,true);header.setUint32(24+jsonLength,0x004e4942,true);
 const view=new DataView(result.buffer,28+jsonLength,binaryLength);
 for(let z=0;z<n;z++)for(let x=0;x<n;x++){
  const i=z*n+x,y=heights[i],left=Math.max(0,x-1),right=Math.min(n-1,x+1),up=Math.max(0,z-1),down=Math.min(n-1,z+1);
  const dx=(heights[z*n+right]-heights[z*n+left])/((right-left)*step),dz=(heights[down*n+x]-heights[up*n+x])/((down-up)*step),length=Math.hypot(dx,1,dz),gray=0.3+0.6*(s.height?y/s.height:0),offset=i*12;
  view.setFloat32(offset,x*step-s.size/2,true);view.setFloat32(offset+4,y,true);view.setFloat32(offset+8,z*step-s.size/2,true);
  view.setFloat32(attributeBytes+offset,-dx/length,true);view.setFloat32(attributeBytes+offset+4,1/length,true);view.setFloat32(attributeBytes+offset+8,-dz/length,true);
  for(let c=0;c<3;c++)view.setFloat32(attributeBytes*2+offset+c*4,gray,true);
 }
 let offset=attributeBytes*3;
 for(let z=0;z<n-1;z++)for(let x=0;x<n-1;x++){
  const i=z*n+x;
  view.setUint32(offset,i,true);view.setUint32(offset+4,i+n,true);view.setUint32(offset+8,i+1,true);view.setUint32(offset+12,i+1,true);view.setUint32(offset+16,i+n,true);view.setUint32(offset+20,i+n+1,true);offset+=24;
 }
 return result;
}
