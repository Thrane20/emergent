import {SphereGeometry} from "three";
import type {PolygonMesh} from "./structure";
export const STRUCTURE_PRIMITIVES=["Cube","Plane","Pyramid","Cylinder","Cone","Sphere"] as const;
export function structurePrimitive(kind:typeof STRUCTURE_PRIMITIVES[number]):PolygonMesh{
 const bottom=[[-0.5,-0.5,-0.5],[0.5,-0.5,-0.5],[0.5,-0.5,0.5],[-0.5,-0.5,0.5]];
 if(kind==="Cube")return {vertices:[...bottom,...bottom.map(v=>[v[0],0.5,v[2]])],faces:[[0,1,2,3],[4,7,6,5],[0,4,5,1],[1,5,6,2],[2,6,7,3],[3,7,4,0]]};
 if(kind==="Plane")return {vertices:bottom.map(v=>[v[0],0,v[2]]),faces:[[0,3,2,1]]};
 if(kind==="Pyramid")return {vertices:[...bottom,[0,0.5,0]],faces:[[0,1,2,3],[0,4,1],[1,4,2],[2,4,3],[3,4,0]]};
 if(kind==="Cylinder"||kind==="Cone"){
  const count=16,ring=Array.from({length:count},(_,i)=>[Math.cos(i/count*Math.PI*2)*0.5,-0.5,Math.sin(i/count*Math.PI*2)*0.5]);
  if(kind==="Cone")return {vertices:[...ring,[0,0.5,0]],faces:[ring.map((_,i)=>i),...ring.map((_,i)=>[i,count,(i+1)%count])]};
  return {vertices:[...ring,...ring.map(v=>[v[0],0.5,v[2]])],faces:[ring.map((_,i)=>i),ring.map((_,i)=>count*2-1-i),...ring.map((_,i)=>[i,i+count,(i+1)%count+count,(i+1)%count])]};
 }
 const geometry=new SphereGeometry(0.5,16,8),positions=geometry.getAttribute("position"),lookup=new Map<string,number>(),vertices:number[][]=[],remap:number[]=[];
 for(let i=0;i<positions.count;i++){const point=[positions.getX(i),positions.getY(i),positions.getZ(i)].map(n=>Number(n.toFixed(7))),key=point.join(",");let index=lookup.get(key);if(index===undefined){index=vertices.length;vertices.push(point);lookup.set(key,index);}remap.push(index);}
 const indices=geometry.index!,faces:number[][]=[];for(let i=0;i<indices.count;i+=3){const face=[0,1,2].map(c=>remap[indices.getX(i+c)]);if(new Set(face).size===3)faces.push(face);}geometry.dispose();return {vertices,faces};
}
