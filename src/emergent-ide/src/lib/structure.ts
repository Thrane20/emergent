import {Euler,Matrix4,Quaternion,Vector3} from "three";
import {sceneHierarchy,validParents} from "./scene-hierarchy";
export type StructureTransform={translation:number[];rotation_degrees:number[];scale:number[]};
export type PolygonMesh={vertices:number[][];faces:number[][]};
export type StructurePart={id:string;name:string;parent:string|null;transform:StructureTransform;mesh:PolygonMesh};
export type StructureDocument={schema_version:1;id:string;name:string;parts:StructurePart[]};
export type StructureSelection={part:string;face?:number;vertex?:number};
const EPS=1e-8;
export const identityStructureTransform=():StructureTransform=>({translation:[0,0,0],rotation_degrees:[0,0,0],scale:[1,1,1]});
export function faceNormal(mesh:PolygonMesh,face:number){
 const indices=mesh.faces[face];if(!indices)throw new Error("Select a face.");const normal=[0,0,0];
 for(let i=0;i<indices.length;i++){const a=mesh.vertices[indices[i]],b=mesh.vertices[indices[(i+1)%indices.length]];normal[0]+=(a[1]-b[1])*(a[2]+b[2]);normal[1]+=(a[2]-b[2])*(a[0]+b[0]);normal[2]+=(a[0]-b[0])*(a[1]+b[1]);}
 const length=Math.hypot(...normal);if(length<EPS)throw new Error("Face is collapsed.");return normal.map(n=>n/length);
}
/** Dominant-plane ear clipping; kept in step with StructureDefinition in Rust. */
export function triangulateFace(mesh:PolygonMesh,face:number):number[]{
 const indices=mesh.faces[face],normal=faceNormal(mesh,face),axis=normal.reduce((best,n,i)=>Math.abs(n)>Math.abs(normal[best])?i:best,0),axes=[0,1,2].filter(i=>i!==axis);
 const points=indices.map(index=>axes.map(axis=>mesh.vertices[index][axis]));
 const cross=(a:number[],b:number[],c:number[])=>(b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]);
 const on=(a:number[],b:number[],p:number[])=>Math.abs(cross(a,b,p))<=EPS&&p.every((n,i)=>n>=Math.min(a[i],b[i])-EPS&&n<=Math.max(a[i],b[i])+EPS);
 for(let i=0;i<points.length;i++)for(let j=i+1;j<points.length;j++){
  if(j===i+1||(i===0&&j===points.length-1))continue;const a=points[i],b=points[(i+1)%points.length],c=points[j],d=points[(j+1)%points.length];
  if((cross(a,b,c)*cross(a,b,d)<-EPS&&cross(c,d,a)*cross(c,d,b)<-EPS)||on(a,b,c)||on(a,b,d)||on(c,d,a)||on(c,d,b))throw new Error("Face edges intersect.");
 }
 const area=points.reduce((sum,a,i)=>{const b=points[(i+1)%points.length];return sum+a[0]*b[1]-b[0]*a[1];},0),sign=Math.sign(area);if(Math.abs(area)<=EPS)throw new Error("Face is collapsed.");
 const remaining=indices.map((_,i)=>i);const triangles:number[]=[];
 for(let i=remaining.length-1;i>=0&&remaining.length>3;i--){const n=remaining.length;if(Math.abs(cross(points[remaining[(i+n-1)%n]],points[remaining[i]],points[remaining[(i+1)%n]]))<=EPS)remaining.splice(i,1);}
 while(remaining.length>3){let clipped=false;
  for(let i=0;i<remaining.length;i++){
   const a=remaining[(i+remaining.length-1)%remaining.length],b=remaining[i],c=remaining[(i+1)%remaining.length];if(cross(points[a],points[b],points[c])*sign<=EPS)continue;
   if(remaining.some(p=>p!==a&&p!==b&&p!==c&&cross(points[a],points[b],points[p])*sign>=-EPS&&cross(points[b],points[c],points[p])*sign>=-EPS&&cross(points[c],points[a],points[p])*sign>=-EPS))continue;
   triangles.push(indices[a],indices[b],indices[c]);remaining.splice(i,1);clipped=true;break;
  }
  if(!clipped)throw new Error("Face cannot be triangulated. Undo the collapsed or intersecting edit.");
 }
 triangles.push(...remaining.map(i=>indices[i]));
 for(let i=0;i<triangles.length;i+=3){const a=new Vector3().fromArray(mesh.vertices[triangles[i]]),b=new Vector3().fromArray(mesh.vertices[triangles[i+1]]),c=new Vector3().fromArray(mesh.vertices[triangles[i+2]]);if(b.sub(a).cross(c.sub(a)).lengthSq()<=1e-12)throw new Error("Edit creates a degenerate triangle.");}
 return triangles;
}
export function structureMatrix(transform:StructureTransform){return new Matrix4().compose(new Vector3(...transform.translation),new Quaternion().setFromEuler(new Euler(...transform.rotation_degrees.map(n=>n*Math.PI/180) as [number,number,number],"XYZ")),new Vector3(...transform.scale));}
export function structureWorldMatrices(parts:StructurePart[]){const result=new Map<string,Matrix4>();for(const {instance} of sceneHierarchy(parts)){const matrix=structureMatrix(instance.transform);if(instance.parent)matrix.premultiply(result.get(instance.parent)!);if(!matrix.elements.every(Number.isFinite)||Math.abs(matrix.determinant())<1e-16)throw new Error("Structure transform is singular or overflowing.");result.set(instance.id,matrix);}return result;}
export function structureDocument(value:unknown):StructureDocument{
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Structure document is required.");const d=value as StructureDocument;
 const id=(n:unknown)=>typeof n==="string"&&/^[a-z0-9_.-]{1,128}$/i.test(n),name=(n:unknown)=>typeof n==="string"&&!!n.trim()&&n.length<=64&&!/[\p{Cc}\p{Cf}]/u.test(n);
 if(Object.keys(d).some(key=>!["schema_version","id","name","parts"].includes(key))||d.schema_version!==1||!id(d.id)||!name(d.name)||!Array.isArray(d.parts)||d.parts.length>128)throw new Error("Use schema 1, a valid ID/name, and at most 128 structure parts.");
 let vertices=0,triangles=0;
 for(const p of d.parts){
  if(!p||Object.keys(p).some(key=>!["id","name","parent","transform","mesh"].includes(key))||!id(p.id)||!name(p.name)||(p.parent!==null&&!id(p.parent))||!p.transform||!p.mesh)throw new Error("Invalid structure part.");
  if(Object.keys(p.transform).some(key=>!["translation","rotation_degrees","scale"].includes(key)))throw new Error("Invalid part transform.");
  for(const key of ["translation","rotation_degrees","scale"] as const){const vector=p.transform[key];if(!Array.isArray(vector)||vector.length!==3||!vector.every(n=>Number.isFinite(n)&&Math.abs(n)<=10000&&(key!=="scale"||Math.abs(n)>=0.0001)))throw new Error("Part transforms require three finite values; scale axes must be nonzero.");}
  const m=p.mesh;
  if(Object.keys(m).some(key=>!["vertices","faces"].includes(key))||!Array.isArray(m.vertices)||m.vertices.length<3||m.vertices.length>4096||!m.vertices.every(v=>Array.isArray(v)&&v.length===3&&v.every(n=>Number.isFinite(n)&&Math.abs(n)<=10000))||!Array.isArray(m.faces)||!m.faces.length||m.faces.length>8192)throw new Error("Part meshes need 3–4096 finite vertices and 1–8192 polygon faces.");
  vertices+=m.vertices.length;
  for(let face=0;face<m.faces.length;face++){const indices=m.faces[face];if(!Array.isArray(indices)||indices.length<3||indices.length>64||new Set(indices).size!==indices.length||!indices.every(i=>Number.isInteger(i)&&i>=0&&i<m.vertices.length))throw new Error("Faces need 3–64 distinct valid vertex indices.");triangles+=triangulateFace(m,face).length/3;if(triangles>131072)throw new Error("Structure exceeds 131072 triangles.");}
 }
 if(vertices>65536||triangles>131072)throw new Error("Structure exceeds 65536 vertices or 131072 triangles.");structureWorldMatrices(d.parts);return d;
}
export function extrudeFace(mesh:PolygonMesh,face:number,distance:number,inset=0):PolygonMesh{
 if(!Number.isFinite(distance)||Math.abs(distance)>10000||!Number.isFinite(inset)||inset<0||inset>=1||(Math.abs(distance)<0.0001&&inset===0))throw new Error("Use a nonzero extrusion depth or an inset in 0–99%.");
 const normal=faceNormal(mesh,face),indices=mesh.faces[face],center=[0,1,2].map(c=>indices.reduce((sum,index)=>sum+mesh.vertices[index][c],0)/indices.length),result=structuredClone(mesh),cap:number[]=[];
 for(const index of indices){cap.push(result.vertices.length);result.vertices.push(mesh.vertices[index].map((n,c)=>n+(center[c]-n)*inset+normal[c]*distance));}
 result.faces[face]=cap;
 for(let i=0;i<indices.length;i++){const next=(i+1)%indices.length;result.faces.push([indices[i],indices[next],cap[next],cap[i]]);}
 return result;
}
export function moveMeshSelection(mesh:PolygonMesh,selection:Pick<StructureSelection,"face"|"vertex">,delta:number[]){
 if(delta.length!==3||!delta.every(Number.isFinite))throw new Error("Move requires three finite values.");const result=structuredClone(mesh),indices=selection.vertex!==undefined?[selection.vertex]:selection.face!==undefined?mesh.faces[selection.face]:mesh.vertices.map((_,i)=>i);if(!indices||indices.some(i=>!mesh.vertices[i]))throw new Error("Select a valid face or vertex.");for(const index of new Set(indices))result.vertices[index]=result.vertices[index].map((n,c)=>n+delta[c]);return result;
}
export function moveStructurePart(parts:StructurePart[],id:string,target:string|null,position:"before"|"child"|"after"){
 if(id===target)throw new Error("A part cannot be dropped onto itself.");const source=parts.find(p=>p.id===id),destination=target?parts.find(p=>p.id===target):undefined;if(!source||(target&&!destination))throw new Error("Part no longer exists.");const parent=position==="child"?target:destination?.parent??null;
 if(parent&&!validParents(parts,id).some(p=>p.id===parent))throw new Error("Parenting would create a cycle.");const result=parts.filter(p=>p.id!==id),insert=position==="child"?result.length:result.findIndex(p=>p.id===target)+(position==="after"?1:0);result.splice(Math.max(0,insert),0,{...source,parent});sceneHierarchy(result);return result;
}
