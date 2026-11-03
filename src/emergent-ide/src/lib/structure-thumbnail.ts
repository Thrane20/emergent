import {Vector3} from "three";
import {structureWorldMatrices,type StructureDocument} from "./structure";

/** Static geometry swatch; avoids a WebGL context and animation loop per library tile. */
export function structureThumbnail(document:StructureDocument):string{
 const matrices=structureWorldMatrices(document.parts),view=new Vector3(1,0.8,1).normalize(),right=new Vector3(1,0,-1).normalize(),up=new Vector3().crossVectors(view,right);
 const faces=document.parts.flatMap(part=>{const points=part.mesh.vertices.map(vertex=>new Vector3().fromArray(vertex).applyMatrix4(matrices.get(part.id)!));return part.mesh.faces.map(face=>{const vertices=face.map(i=>points[i]),normal=new Vector3().subVectors(vertices[1],vertices[0]).cross(new Vector3().subVectors(vertices[2],vertices[0])).normalize();return {points:vertices.map(p=>[p.dot(right),-p.dot(up)]),depth:vertices.reduce((sum,p)=>sum+p.dot(view),0)/vertices.length,shade:Math.round(48+Math.abs(normal.dot(view))*24)};});}).sort((a,b)=>a.depth-b.depth);
 let minX=Infinity,minY=Infinity,maxX=-Infinity,maxY=-Infinity;
 for(const face of faces)for(const [x,y] of face.points){minX=Math.min(minX,x);maxX=Math.max(maxX,x);minY=Math.min(minY,y);maxY=Math.max(maxY,y);}
 const scale=88/Math.max(maxX-minX,maxY-minY,0.01),cx=(minX+maxX)/2,cy=(minY+maxY)/2;
 const polygons=faces.map(face=>`<polygon points="${face.points.map(([x,y])=>`${((x-cx)*scale+60).toFixed(2)},${((y-cy)*scale+52).toFixed(2)}`).join(" ")}" fill="hsl(263 26% ${face.shade}%)" stroke="#302c40" stroke-width="0.5"/>`).join("");
 return `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 104"><rect width="120" height="104" fill="#252630"/>${polygons}</svg>`)}`;
}
