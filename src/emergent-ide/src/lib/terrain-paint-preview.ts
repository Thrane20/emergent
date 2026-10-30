import {terrainGlb,type TerrainSettings} from "./terrain";
import type {TerrainPaint} from "./terrain-paint";
import type {MaterialSettings} from "./material-settings";
type Pixels={width:number;height:number;data:Uint8ClampedArray};
const images=new Map<string,Promise<Pixels>>();
async function imagePixels(projectId:string,path:string,revision:string){
 const url=`/api/project-asset?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(path)}&v=${encodeURIComponent(revision)}`;
 let pending=images.get(url);if(!pending){pending=new Promise<Pixels>((resolve,reject)=>{const image=new Image();image.onload=()=>{const canvas=document.createElement("canvas");canvas.width=image.naturalWidth;canvas.height=image.naturalHeight;const ctx=canvas.getContext("2d",{willReadFrequently:true})!;ctx.drawImage(image,0,0);resolve({width:canvas.width,height:canvas.height,data:ctx.getImageData(0,0,canvas.width,canvas.height).data});};image.onerror=()=>reject(new Error(`Could not preview texture ${path}.`));image.src=url;});if(images.size>=8)images.delete(images.keys().next().value!);images.set(url,pending);pending.catch(()=>images.delete(url));}
 return pending;
}
const linear=(n:number)=>n<=0.04045?n/12.92:((n+0.055)/1.055)**2.4;
const srgb=(n:number)=>Math.round(Math.max(0,Math.min(1,n<=0.0031308?n*12.92:1.055*n**(1/2.4)-0.055))*255);
/** Diffuse-only editing preview. Native inspection renders the complete library material. */
export async function terrainPaintPreview(projectId:string,settings:TerrainSettings,heights:Float32Array,paint:TerrainPaint|null,library:{path:string;revision:string;material:MaterialSettings}[]):Promise<Uint8Array<ArrayBuffer>>{
 const base=terrainGlb(settings,heights);if(!paint)return base;
 const layers=await Promise.all(paint.materials.map(async path=>{const item=library.find(item=>item.path===path);if(!item)throw new Error("A painted material is missing from the library. Refresh the library.");const material=item.material,map=material.active_layer?material.terrain_layers.find(layer=>layer.id===material.active_layer)?.diffuse_map:material.diffuse_map;return {material,map,image:map?await imagePixels(projectId,map.asset,item.revision):undefined};}));
 const n=settings.resolution,side=512,canvas=document.createElement("canvas");canvas.width=side;canvas.height=side;const ctx=canvas.getContext("2d")!,image=ctx.createImageData(side,side);
 const sample=(p:Pixels,u:number,v:number,c:number)=>linear(p.data[((Math.floor(((v%1+1)%1)*p.height)*p.width)+Math.floor(((u%1+1)%1)*p.width))*4+c]/255);
 const weight=(x:number,z:number,c:number)=>paint.weights[(z*n+x)*4+c]/255;
 for(let row=0;row<side;row++)for(let col=0;col<side;col++){
  const fx=col/(side-1)*(n-1),fz=row/(side-1)*(n-1),x=Math.min(n-2,Math.floor(fx)),z=Math.min(n-2,Math.floor(fz)),tx=fx-x,tz=fz-z;
  const interpolate=(get:(x:number,z:number)=>number)=>(get(x,z)*(1-tx)+get(x+1,z)*tx)*(1-tz)+(get(x,z+1)*(1-tx)+get(x+1,z+1)*tx)*tz;
  const height=interpolate((x,z)=>heights[z*n+x]),step=settings.size/(n-1),dx=(heights[z*n+x+1]-heights[z*n+x])/step,dz=(heights[(z+1)*n+x]-heights[z*n+x])/step,axes=[Math.abs(dx)**4,1,Math.abs(dz)**4],sum=axes[0]+axes[1]+axes[2];
  const rgb=[0,0,0];
  for(let layer=0;layer<layers.length;layer++){
   const opacity=interpolate((x,z)=>weight(x,z,layer));if(opacity<0.00001)continue;
   const {material,map,image}=layers[layer],scale=map?.scale??1,px=(col/(side-1)-0.5)*settings.size*scale,pz=(row/(side-1)-0.5)*settings.size*scale,py=height*scale;
   for(let c=0;c<3;c++){const diffuse=image?(sample(image,py,pz,c)*axes[0]+sample(image,px,pz,c)*axes[1]+sample(image,px,py,c)*axes[2])/sum:1;rgb[c]+=(material.albedo?.[c]??(0.3+0.6*(settings.height?height/settings.height:0)))*diffuse*opacity;}
  }
  const i=(row*side+col)*4;for(let c=0;c<3;c++)image.data[i+c]=srgb(rgb[c]);image.data[i+3]=255;
 }
 ctx.putImageData(image,0,0);const blob=await new Promise<Blob>((resolve,reject)=>canvas.toBlob(blob=>blob?resolve(blob):reject(new Error("Could not create terrain preview.")),"image/png")),png=new Uint8Array(await blob.arrayBuffer());
 const header=new DataView(base.buffer),jsonLength=header.getUint32(12,true),gltf=JSON.parse(new TextDecoder().decode(base.subarray(20,20+jsonLength))),geometry=base.subarray(28+jsonLength),uv=new Uint8Array(n*n*8),view=new DataView(uv.buffer);
 for(let z=0;z<n;z++)for(let x=0;x<n;x++){const i=(z*n+x)*8;view.setFloat32(i,x/(n-1),true);view.setFloat32(i+4,z/(n-1),true);}
 // The atlas is already tinted; remove the grayscale vertex multiplier.
 delete gltf.meshes[0].primitives[0].attributes.COLOR_0;
 const uvView=gltf.bufferViews.length;gltf.bufferViews.push({buffer:0,byteOffset:geometry.length,byteLength:uv.length,target:34962});const uvAccessor=gltf.accessors.length;gltf.accessors.push({bufferView:uvView,componentType:5126,count:n*n,type:"VEC2"});gltf.meshes[0].primitives[0].attributes.TEXCOORD_0=uvAccessor;
 const imageView=gltf.bufferViews.length;gltf.bufferViews.push({buffer:0,byteOffset:geometry.length+uv.length,byteLength:png.length});gltf.images=[{bufferView:imageView,mimeType:"image/png"}];gltf.textures=[{source:0}];gltf.materials[0].pbrMetallicRoughness.baseColorTexture={index:0};
 const length=Math.ceil((geometry.length+uv.length+png.length)/4)*4;gltf.buffers[0].byteLength=length;const json=new TextEncoder().encode(JSON.stringify(gltf)),padded=Math.ceil(json.length/4)*4,result=new Uint8Array(28+padded+length),out=new DataView(result.buffer);
 out.setUint32(0,0x46546c67,true);out.setUint32(4,2,true);out.setUint32(8,result.length,true);out.setUint32(12,padded,true);out.setUint32(16,0x4e4f534a,true);result.fill(32,20,20+padded);result.set(json,20);out.setUint32(20+padded,length,true);out.setUint32(24+padded,0x004e4942,true);result.set(geometry,28+padded);result.set(uv,28+padded+geometry.length);result.set(png,28+padded+geometry.length+uv.length);return result;
}
