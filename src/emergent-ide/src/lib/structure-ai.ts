import {identityStructureTransform,structureDocument,type StructureDocument} from "./structure";
import {STRUCTURE_PRIMITIVES,structurePrimitive} from "./structure-primitives";

// AI composes primitives or retains existing edited meshes by durable part ID.
export function structureProposal(current:StructureDocument,value:unknown):StructureDocument{
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Expected an object proposal.");
 const proposal=value as {name?:unknown;parts?:unknown};
 if(!Array.isArray(proposal.parts)||!proposal.parts.length||proposal.parts.length>128)throw new Error("Propose 1–128 parts.");
 const parts=proposal.parts.map((raw:unknown)=>{
  if(!raw||typeof raw!=="object"||Array.isArray(raw))throw new Error("Invalid proposed part.");
  const part=raw as {id:string;name:string;parent?:string|null;primitive?:typeof STRUCTURE_PRIMITIVES[number];transform?:StructureDocument["parts"][number]["transform"];size?:number[]};
  if(part.primitive!==undefined&&!STRUCTURE_PRIMITIVES.includes(part.primitive))throw new Error("Choose a supported primitive.");
  const existing=current.parts.find(item=>item.id===part.id);
  const mesh=part.primitive?structurePrimitive(part.primitive):existing?.mesh;
  if(!mesh)throw new Error("New parts require a primitive.");
  const transform=structuredClone(part.transform??existing?.transform??identityStructureTransform());
  const shape=structuredClone(mesh);
  if(part.primitive){
   const size=part.size??[1,1,1];
   if(!Array.isArray(size)||size.length!==3||size.some(n=>typeof n!=="number"||!Number.isFinite(n)||n<=0)||transform.scale.some(n=>n<=0))throw new Error("Primitive dimensions and scale must be positive.");
   const dimensions=size.map((n,i)=>n*transform.scale[i]);
   shape.vertices=shape.vertices.map(vertex=>vertex.map((n,i)=>n*dimensions[i]));
   transform.scale=[1,1,1];
  }
  return {id:part.id,name:part.name,parent:part.parent??null,transform,mesh:shape};
 });
 return structureDocument({...current,name:proposal.name??current.name,parts});
}
