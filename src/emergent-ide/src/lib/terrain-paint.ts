import {circleOpacity} from "./terrain-brush";
import {TERRAIN_RESOLUTIONS,type TerrainSettings} from "./terrain";
export type TerrainPaint={schema_version:1;resolution:number;materials:string[];transitions:number[];blend:number;weights:number[]};
export const TERRAIN_MATERIAL_NAMES=["River bed","Ground","Mountain","Summit"];
export function terrainPaint(value:unknown,resolution:number):TerrainPaint{
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Terrain paint settings are required.");
 const p=value as TerrainPaint;
 if(Object.keys(p).some(key=>!["schema_version","resolution","materials","transitions","blend","weights"].includes(key))||p.schema_version!==1||p.resolution!==resolution||!TERRAIN_RESOLUTIONS.some(value=>value===resolution)||!Array.isArray(p.materials)||p.materials.length<1||p.materials.length>4||!p.materials.every(path=>typeof path==="string"&&path.endsWith(".material.toml")&&path.length<=512)||!Array.isArray(p.transitions)||p.transitions.length!==3||!p.transitions.every((n,i)=>typeof n==="number"&&Number.isFinite(n)&&n>=0&&n<=1000&&(i===0||n>p.transitions[i-1]))||!Number.isFinite(p.blend)||p.blend<0||p.blend>1000||!Array.isArray(p.weights)||p.weights.length!==resolution**2*4)throw new Error("Invalid terrain material palette, height transitions or paint resolution.");
 for(let i=0;i<p.weights.length;i+=4){let sum=0;for(let c=0;c<4;c++){const n=p.weights[i+c];if(!Number.isInteger(n)||n<0||n>255||(c>=p.materials.length&&n!==0))throw new Error("Invalid terrain material weights.");sum+=n;}if(sum!==255)throw new Error("Terrain material weights must sum to 255.");}
 return {...p};
}
export function quantizeWeights(values:Float32Array){
 const result=new Array<number>(values.length);
 for(let i=0;i<values.length;i+=4){let total=0,largest=0;for(let c=0;c<4;c++){const n=values[i+c];if(!Number.isFinite(n)||n<0)throw new Error("Invalid paint weight.");if(n>values[i+largest])largest=c;result[i+c]=Math.round(n*255);total+=result[i+c];}result[i+largest]+=255-total;}
 return result;
}
export function heightMaterialWeights(heights:Float32Array,transitions:number[],blend:number,count:number){
 if(count<1||count>4||transitions.length!==3||!transitions.every((n,i)=>Number.isFinite(n)&&n>=0&&n<=1000&&(i===0||n>transitions[i-1]))||!Number.isFinite(blend)||blend<0||blend>1000)throw new Error("Use three increasing height transitions and a nonnegative blend width.");
 const values=new Float32Array(heights.length*4);
 for(let i=0;i<heights.length;i++){
  let remaining=1;
  for(let c=0;c<count;c++){
   let upper=0;if(c<count-1){const distance=heights[i]-transitions[c];upper=blend===0?(distance>=0?1:0):Math.max(0,Math.min(1,distance/blend+0.5));upper=upper*upper*(3-2*upper);}
   values[i*4+c]=remaining*(1-upper);remaining*=upper;
  }
 }
 return quantizeWeights(values);
}
/** Paint normalized float weights, quantizing only at commit so weak strokes accumulate. */
export function paintMaterialStroke(values:Float32Array,settings:TerrainSettings,slot:number,radius:number,strength:number,fromX:number,fromZ:number,x:number,z:number,seconds:number){
 const n=settings.resolution,step=settings.size/(n-1),spacing=Math.max(step*0.5,radius/4),distance=Math.hypot(x-fromX,z-fromZ);
 if(values.length!==n*n*4||!Number.isInteger(slot)||slot<0||slot>3||![radius,strength,fromX,fromZ,x,z,seconds].every(Number.isFinite)||radius<=0||radius>settings.size||strength<0||strength>1||seconds<0||seconds>0.1||Math.max(Math.abs(fromX),Math.abs(fromZ),Math.abs(x),Math.abs(z))>settings.size/2+0.001)throw new Error("Invalid terrain material brush.");
 const stamps=Math.max(1,Math.ceil(distance/spacing)),exposure=Math.min(0.1,Math.max(seconds,distance/spacing*0.1)/stamps);
 for(let stamp=1;stamp<=stamps;stamp++){
  const cx=((fromX+(x-fromX)*stamp/stamps)/settings.size+0.5)*(n-1),cz=((fromZ+(z-fromZ)*stamp/stamps)/settings.size+0.5)*(n-1),r=radius/step;
  for(let row=Math.max(0,Math.ceil(cz-r));row<=Math.min(n-1,Math.floor(cz+r));row++)for(let col=Math.max(0,Math.ceil(cx-r));col<=Math.min(n-1,Math.floor(cx+r));col++){
   const amount=1-Math.exp(-circleOpacity(Math.hypot(col-cx,row-cz)/r)*strength*exposure*10),i=(row*n+col)*4;
   for(let c=0;c<4;c++)values[i+c]+=(Number(c===slot)-values[i+c])*amount;
  }
 }
}
