import {terrainSettings,type TerrainSettings} from "./terrain";

export type TerrainBrush={mode:"raise"|"lower"|"smooth"|"flatten";radius:number;strength:number;height:number};

/** White centre, black edge. The grayscale value is the sample's opacity. */
export function circleOpacity(distance:number){return Math.max(0,1-distance*distance);}

export function terrainHeights(value:unknown,settings:TerrainSettings):Float32Array {
 const s=terrainSettings(settings);
 if(!Array.isArray(value)||value.length!==s.resolution**2||!value.every(v=>typeof v==="number"&&Number.isFinite(v)&&v>=0&&v<=s.height+0.0001))throw new Error("Invalid terrain height field.");
 return Float32Array.from(value);
}

/** Mutate only the brush rectangle; scratch preserves order-independent smoothing. */
export function paintTerrain(values:Float32Array,scratch:Float32Array,settings:TerrainSettings,brush:TerrainBrush,x:number,z:number,seconds:number){
 const n=settings.resolution,step=settings.size/(n-1);
 if(values.length!==n*n||scratch.length!==values.length||scratch===values||![x,z,seconds,brush.radius,brush.strength,brush.height].every(Number.isFinite)||seconds<0||seconds>0.1||brush.radius<=0||brush.radius>settings.size||brush.strength<0||brush.strength>1||brush.height<0||brush.height>settings.height||!["raise","lower","smooth","flatten"].includes(brush.mode))throw new Error("Invalid terrain brush.");
 if(brush.strength===0||seconds===0)return;
 if(brush.mode==="smooth")scratch.set(values);
 const cx=(x/settings.size+0.5)*(n-1),cz=(z/settings.size+0.5)*(n-1),radius=brush.radius/step;
 for(let row=Math.max(0,Math.ceil(cz-radius));row<=Math.min(n-1,Math.floor(cz+radius));row++)for(let col=Math.max(0,Math.ceil(cx-radius));col<=Math.min(n-1,Math.floor(cx+radius));col++){
  const opacity=circleOpacity(Math.hypot(col-cx,row-cz)/radius);if(!opacity)continue;
  const i=row*n+col,amount=opacity*brush.strength;
  if(brush.mode==="raise"||brush.mode==="lower")values[i]=Math.max(0,Math.min(settings.height,values[i]+(brush.mode==="raise"?1:-1)*amount*seconds*Math.max(1,settings.height*0.25)));
  else {
   let target=brush.height;
   if(brush.mode==="smooth"){
    let sum=0,count=0;
    for(let dz=Math.max(0,row-1);dz<=Math.min(n-1,row+1);dz++)for(let dx=Math.max(0,col-1);dx<=Math.min(n-1,col+1);dx++){sum+=scratch[dz*n+dx];count++;}
    target=sum/count;
   }
   values[i]+=(target-values[i])*(1-Math.exp(-amount*seconds*8));
  }
 }
}

/** Keep a fast drag visible: deposit by distance as well as time, with overlapping stamps. */
export function paintTerrainStroke(values:Float32Array,scratch:Float32Array,settings:TerrainSettings,brush:TerrainBrush,fromX:number,fromZ:number,x:number,z:number,seconds:number){
 const distance=Math.hypot(x-fromX,z-fromZ),spacing=Math.max(settings.size/(settings.resolution-1)*0.5,brush.radius/4);
 if(![distance,seconds,spacing].every(Number.isFinite)||seconds<0||seconds>0.1||spacing<=0)throw new Error("Invalid terrain stroke.");
 const steps=Math.max(1,Math.ceil(distance/spacing)),exposure=Math.min(0.1,Math.max(seconds,distance/spacing*0.1)/steps);
 for(let i=1;i<=steps;i++)paintTerrain(values,scratch,settings,brush,fromX+(x-fromX)*i/steps,fromZ+(z-fromZ)*i/steps,exposure);
}

/** Extract the editable field from our own packed GLB position accessor. */
export function terrainGlbHeights(buffer:ArrayBuffer,settings:TerrainSettings){
 const view=new DataView(buffer),length=view.getUint32(12,true),json=JSON.parse(new TextDecoder().decode(new Uint8Array(buffer,20,length)));
 const accessor=json.accessors[json.meshes[0].primitives[0].attributes.POSITION],section=json.bufferViews[accessor.bufferView];
 if(accessor.count!==settings.resolution**2||accessor.type!=="VEC3"||accessor.componentType!==5126)throw new Error("Invalid terrain mesh.");
 return terrainHeights(Array.from({length:accessor.count},(_,i)=>view.getFloat32(28+length+(section.byteOffset??0)+i*12+4,true)),settings);
}
