import {Euler,Matrix4,Quaternion,Vector3} from "three";
import {sceneHierarchy,validParents} from "./scene-hierarchy";
export type StructureTransform={translation:number[];rotation_degrees:number[];scale:number[]};
export type PolygonMesh={vertices:number[][];faces:number[][]};
export type StructurePart={id:string;name:string;parent:string|null;transform:StructureTransform;mesh:PolygonMesh;/** Library material path overriding the object's material for this part. */material?:string};
export type StructureDocument={schema_version:1;id:string;name:string;parts:StructurePart[]};
export type StructureSelection={part:string;face?:number;vertex?:number;edge?:[number,number]};
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
  if(!p||Object.keys(p).some(key=>!["id","name","parent","transform","mesh","material"].includes(key))||(p.material!==undefined&&(typeof p.material!=="string"||!p.material.endsWith(".material.toml")||p.material.length>512))||!id(p.id)||!name(p.name)||(p.parent!==null&&!id(p.parent))||!p.transform||!p.mesh)throw new Error("Invalid structure part.");
  if(Object.keys(p.transform).some(key=>!["translation","rotation_degrees","scale"].includes(key)))throw new Error("Invalid part transform.");
  for(const key of ["translation","rotation_degrees","scale"] as const){const vector=p.transform[key];if(!Array.isArray(vector)||vector.length!==3||!vector.every(n=>Number.isFinite(n)&&Math.abs(n)<=10000&&(key!=="scale"||Math.abs(n)>=0.0001)))throw new Error("Part transforms require three finite values; scale axes must be nonzero.");}
  const m=p.mesh;
  if(Object.keys(m).some(key=>!["vertices","faces"].includes(key))||!Array.isArray(m.vertices)||!Array.isArray(m.faces)||m.vertices.length>4096||m.faces.length>8192||(m.vertices.length===0?m.faces.length!==0:m.vertices.length<3||m.faces.length===0)||!m.vertices.every(v=>Array.isArray(v)&&v.length===3&&v.every(n=>Number.isFinite(n)&&Math.abs(n)<=10000)))throw new Error("Part meshes need 3–4096 finite vertices and 1–8192 polygon faces.");
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
export function moveMeshSelection(mesh:PolygonMesh,selection:Pick<StructureSelection,"face"|"vertex"|"edge">,delta:number[]){
 if(delta.length!==3||!delta.every(Number.isFinite))throw new Error("Move requires three finite values.");const result=structuredClone(mesh),indices=selection.vertex!==undefined?[selection.vertex]:selection.edge??(selection.face!==undefined?mesh.faces[selection.face]:mesh.vertices.map((_,i)=>i));if(!indices||indices.some(i=>!mesh.vertices[i]))throw new Error("Select a valid face or vertex.");for(const index of new Set(indices))result.vertices[index]=result.vertices[index].map((n,c)=>n+delta[c]);return result;
}
export function moveStructurePart(parts:StructurePart[],id:string,target:string|null,position:"before"|"child"|"after"){
 if(id===target)throw new Error("A part cannot be dropped onto itself.");const source=parts.find(p=>p.id===id),destination=target?parts.find(p=>p.id===target):undefined;if(!source||(target&&!destination))throw new Error("Part no longer exists.");const parent=position==="child"?target:destination?.parent??null;
 if(parent&&!validParents(parts,id).some(p=>p.id===parent))throw new Error("Parenting would create a cycle.");const result=parts.filter(p=>p.id!==id),insert=position==="child"?result.length:result.findIndex(p=>p.id===target)+(position==="after"?1:0);result.splice(Math.max(0,insert),0,{...source,parent});sceneHierarchy(result);return result;
}

/** Set one part in parent or world coordinates; refuse transforms that need shear. */
export function editStructureTransform(parts:StructurePart[],id:string,space:"local"|"global",key:keyof StructureTransform,values:number[]):StructurePart[]{
 const part=parts.find(item=>item.id===id);if(!part)throw new Error("Select a part.");
 const local=structuredClone(part.transform);
 if(space==="local")local[key]=values;
 else{
  const world=structureWorldMatrices(parts).get(id)!;
  const position=new Vector3(),rotation=new Quaternion(),scale=new Vector3();world.decompose(position,rotation,scale);
  const angle=new Euler().setFromQuaternion(rotation,"XYZ"),current:StructureTransform={translation:position.toArray(),rotation_degrees:[angle.x,angle.y,angle.z].map(n=>n*180/Math.PI),scale:scale.toArray()};
  current[key]=values;
  const desired=structureMatrix(current),parent=part.parent?structureWorldMatrices(parts).get(part.parent)!.clone().invert():new Matrix4();
  const matrix=parent.multiply(desired),p=new Vector3(),q=new Quaternion(),s=new Vector3();matrix.decompose(p,q,s);
  const rebuilt=new Matrix4().compose(p,q,s);
  if(matrix.elements.some((n,i)=>Math.abs(n-rebuilt.elements[i])>1e-5))throw new Error("This world transform needs shear. Resize the parent mesh or use local controls.");
  local.translation=p.toArray();const localAngle=new Euler().setFromQuaternion(q,"XYZ");local.rotation_degrees=[localAngle.x,localAngle.y,localAngle.z].map(n=>n*180/Math.PI);local.scale=s.toArray();
 }
 const next=parts.map(item=>item.id===id?{...item,transform:local}:item);structureWorldMatrices(next);return next;
}

export function structureWorldTransform(parts:StructurePart[],id:string):StructureTransform{
 const matrix=structureWorldMatrices(parts).get(id);if(!matrix)throw new Error("Select a part.");
 const position=new Vector3(),rotation=new Quaternion(),scale=new Vector3();matrix.decompose(position,rotation,scale);
 const angle=new Euler().setFromQuaternion(rotation,"XYZ");return {translation:position.toArray(),rotation_degrees:[angle.x,angle.y,angle.z].map(n=>n*180/Math.PI),scale:scale.toArray()};
}

/** Turn each part's local scale into mesh dimensions, leaving hierarchy pivots unit-scaled. */
export function bakeStructureSizes(document:StructureDocument):StructureDocument{
 const next=structuredClone(document);
 for(const part of next.parts){
  const scale=part.transform.scale;
  part.mesh.vertices=part.mesh.vertices.map(vertex=>vertex.map((value,i)=>value*scale[i]));
  if(scale[0]*scale[1]*scale[2]<0)part.mesh.faces=part.mesh.faces.map(face=>[...face].reverse());
  part.transform.scale=[1,1,1];
 }
 return structureDocument(next);
}

export function selectedPartRoots(parts:StructurePart[],selected:string[]){const ids=new Set(selected);return parts.filter(part=>{if(!ids.has(part.id))return false;let parent=part.parent;while(parent){if(ids.has(parent))return false;parent=parts.find(item=>item.id===parent)?.parent??null;}return true;}).map(part=>part.id);}
export function deleteStructureParts(document:StructureDocument,selected:string[]):StructureDocument{
 const removed=new Set(selected);for(const {instance} of sceneHierarchy(document.parts))if(instance.parent&&removed.has(instance.parent))removed.add(instance.id);
 return structureDocument({...document,parts:document.parts.filter(part=>!removed.has(part.id))});
}
export function duplicateStructureParts(document:StructureDocument,selected:string[],fresh:()=>string):{document:StructureDocument;ids:string[]}{
 const roots=new Set(selectedPartRoots(document.parts,selected)),copies=new Set<string>();for(const {instance} of sceneHierarchy(document.parts))if(roots.has(instance.id)||instance.parent&&copies.has(instance.parent))copies.add(instance.id);
 const ids=new Map([...copies].map(id=>[id,fresh()]));const newParts=document.parts.filter(part=>copies.has(part.id)).map(part=>({...structuredClone(part),id:ids.get(part.id)!,name:`${part.name.slice(0,59)} copy`,parent:part.parent&&ids.has(part.parent)?ids.get(part.parent)!:part.parent,transform:{...part.transform,translation:part.transform.translation.map((n,i)=>n+(i===0&&roots.has(part.id)?0.5:0))}}));
 return {document:structureDocument({...document,parts:[...document.parts,...newParts]}),ids:[...ids.values()]};
}
export function groupStructureParts(document:StructureDocument,selected:string[],id:string):StructureDocument{
 const roots=selectedPartRoots(document.parts,selected);if(roots.length<2)throw new Error("Select at least two parts to group.");
 const rootParts=roots.map(key=>document.parts.find(part=>part.id===key)!);const currentParent=rootParts[0].parent;
 if(rootParts.some(part=>part.parent!==currentParent))throw new Error("Select parts with the same current parent to group them.");
 const centre=[0,1,2].map(axis=>rootParts.reduce((sum,part)=>sum+part.transform.translation[axis],0)/rootParts.length);
 const parent:StructurePart={id,name:"Group",parent:currentParent,transform:{...identityStructureTransform(),translation:centre},mesh:{vertices:[],faces:[]}};
 const updated=document.parts.map(part=>!roots.includes(part.id)?part:{...part,parent:id,transform:{...part.transform,translation:part.transform.translation.map((value,axis)=>value-centre[axis])}});
 return structureDocument({...document,parts:[parent,...updated]});
}

/** Apply one world-space transform around the selected roots' shared pivot. */
export function transformStructureParts(document:StructureDocument,selected:string[],action:"translate"|"rotate"|"scale",values:number[]):StructureDocument{
 if(values.length!==3||!values.every(Number.isFinite))throw new Error("Enter finite X, Y and Z values.");
 const roots=selectedPartRoots(document.parts,selected),world=structureWorldMatrices(document.parts),pivot=new Vector3();
 if(!roots.length)throw new Error("Select parts to transform.");
 for(const id of roots)pivot.add(new Vector3().setFromMatrixPosition(world.get(id)!));pivot.divideScalar(roots.length);
 const operation=action==="translate"?new Matrix4().makeTranslation(...values as [number,number,number]):action==="rotate"?new Matrix4().makeRotationFromEuler(new Euler(...values.map(n=>n*Math.PI/180) as [number,number,number],"XYZ")):new Matrix4().makeScale(...values as [number,number,number]);
 const around=action==="translate"?operation:new Matrix4().makeTranslation(...pivot.toArray()).multiply(operation).multiply(new Matrix4().makeTranslation(...pivot.clone().negate().toArray()));
 const next=document.parts.map(part=>{if(!roots.includes(part.id))return part;const parent=part.parent?world.get(part.parent)!.clone().invert():new Matrix4(),matrix=parent.multiply(around).multiply(world.get(part.id)!),position=new Vector3(),rotation=new Quaternion(),scale=new Vector3();matrix.decompose(position,rotation,scale);const rebuilt=new Matrix4().compose(position,rotation,scale);if(matrix.elements.some((value,i)=>Math.abs(value-rebuilt.elements[i])>1e-5))throw new Error("This group edit would require shear. Group under a new parent first.");const angle=new Euler().setFromQuaternion(rotation,"XYZ");return {...part,transform:{translation:position.toArray(),rotation_degrees:[angle.x,angle.y,angle.z].map(n=>n*180/Math.PI),scale:scale.toArray()}};});
 return structureDocument({...document,parts:next});
}
