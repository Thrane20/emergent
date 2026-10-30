import type {TerrainSettings} from "./terrain";
import type {TerrainPaint} from "./terrain-paint";
export function terrainThumbnailSvg(settings:TerrainSettings,heights:Float32Array,paint?:TerrainPaint,colors:number[][]=[],base=[.65,.65,.65]){
 const n=20,res=settings.resolution,points:number[][]=[];let minX=Infinity,maxX=-Infinity,minY=Infinity,maxY=-Infinity;
 for(let z=0;z<=n;z++)for(let x=0;x<=n;x++){const index=Math.round(z/n*(res-1))*res+Math.round(x/n*(res-1)),h=heights[index]/settings.size,p=[(x/n-z/n)*.866,(x/n+z/n)*.42-h,index];points.push(p);minX=Math.min(minX,p[0]);maxX=Math.max(maxX,p[0]);minY=Math.min(minY,p[1]);maxY=Math.max(maxY,p[1]);}
 const scale=Math.min(148/(maxX-minX||1),82/(maxY-minY||1)),offsetX=(160-(maxX-minX)*scale)/2,offsetY=(96-(maxY-minY)*scale)/2;
 const point=(i:number)=>`${((points[i][0]-minX)*scale+offsetX).toFixed(2)},${((points[i][1]-minY)*scale+offsetY).toFixed(2)}`;
 const paths:string[]=[];
 for(let depth=0;depth<2*n-1;depth++)for(let z=Math.max(0,depth-n+1);z<=Math.min(n-1,depth);z++){const x=depth-z,a=z*(n+1)+x,b=a+1,c=a+n+1,d=c+1;
  for(const triangle of [[a,c,b],[b,c,d]]){const index=points[triangle[0]][2],shade=Math.max(.35,Math.min(1.1,.8+(heights[points[a][2]]-heights[points[b][2]])/settings.size*n*.7+(heights[points[a][2]]-heights[points[c][2]])/settings.size*n*.4));
   const color=paint&&colors.length?base.map((_,channel)=>paint.materials.reduce((sum,_material,layer)=>sum+(colors[layer]?.[channel]??base[channel])*paint.weights[index*4+layer]/255,0)):base;
   const rgb=color.map(value=>Math.round(255*Math.pow(Math.max(0,Math.min(1,value*shade)),1/2.2)));
   paths.push(`<path d="M${triangle.map(point).join("L")}Z" fill="rgb(${rgb.join(",")})" stroke="rgb(${rgb.join(",")})" stroke-width=".3"/>`);
  }
 }
 return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 96"><rect width="160" height="96" fill="#172129"/>${paths.join("")}</svg>`;
}
