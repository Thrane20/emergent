import assert from "node:assert/strict";
import test from "node:test";
import {faceNormal,extrudeFace,moveMeshSelection,moveStructurePart,identityStructureTransform,structureDocument,structureWorldMatrices,triangulateFace,deleteStructureParts,duplicateStructureParts,groupStructureParts,transformStructureParts,type StructureDocument} from "../src/lib/structure";
import {structurePrimitive,STRUCTURE_PRIMITIVES} from "../src/lib/structure-primitives";
const document=():StructureDocument=>({schema_version:1,id:"test",name:"Test",parts:[{id:"cube",name:"Cube",parent:null,transform:identityStructureTransform(),mesh:structurePrimitive("Cube")}]});
test("all structure primitives have valid finite topology and metre dimensions",()=>{
 for(const primitive of STRUCTURE_PRIMITIVES){const d=document();d.parts[0].mesh=structurePrimitive(primitive);assert.equal(structureDocument(d),d);}
 const mesh=structurePrimitive("Cube");assert.deepEqual(faceNormal(mesh,1),[0,1,0]);assert.equal(Math.max(...mesh.vertices.map(v=>v[0]))-Math.min(...mesh.vertices.map(v=>v[0])),1);
});
test("ear clipping supports concave faces and rejects intersecting/collapsed faces",()=>{
 const mesh={vertices:[[0,0,0],[2,0,0],[2,2,0],[1,1,0],[0,2,0]],faces:[[0,1,2,3,4]]};assert.equal(triangulateFace(mesh,0).length,9);
 assert.throws(()=>triangulateFace({vertices:mesh.vertices,faces:[[0,2,1,4]]},0));assert.throws(()=>triangulateFace({vertices:[[0,0,0],[1,0,0],[2,0,0]],faces:[[0,1,2]]},0));
});
test("extrusion and inset retain cap identity and create a ring of connected sides",()=>{
 const d=document(),mesh=d.parts[0].mesh;
 for(const depth of [-0.2,0.5]){const extruded=extrudeFace(mesh,1,depth);assert.equal(extruded.vertices.length,12);assert.equal(extruded.faces.length,10);assert.ok(extruded.faces[1].every(index=>Math.abs(extruded.vertices[index][1]-(0.5+depth))<1e-8));d.parts[0].mesh=extruded;structureDocument(d);}
 const inset=extrudeFace(mesh,1,0,0.2);assert.equal(inset.faces.length,10);assert.deepEqual(inset.vertices[8],[-0.4,0.5,-0.4]);d.parts[0].mesh=inset;structureDocument(d);d.parts[0].mesh=extrudeFace(inset,1,1);structureDocument(d);
 assert.throws(()=>extrudeFace(mesh,1,0));assert.throws(()=>extrudeFace(mesh,1,1,1));
 const moved=moveMeshSelection(mesh,{face:1},[0,1,0]);assert.equal(moved.vertices[4][1],1.5);assert.equal(moved.vertices[0][1],-0.5);assert.equal(moveMeshSelection(mesh,{vertex:4},[1,0,0]).vertices[5][0],0.5);
});
test("local hierarchy reorder/reparent checks cycles and composes parent transforms",()=>{
 const d=document();d.parts.push({...structuredClone(d.parts[0]),id:"child",name:"Child",parent:"cube"});d.parts[0].transform.translation=[2,0,0];d.parts[1].transform.translation=[0,3,0];assert.deepEqual(structureWorldMatrices(d.parts).get("child")!.elements.slice(12,15),[2,3,0]);
 assert.throws(()=>moveStructurePart(d.parts,"cube","child","child"));const roots=moveStructurePart(d.parts,"child",null,"child");assert.equal(roots[1].parent,null);assert.deepEqual(roots[1].transform.translation,[0,3,0]);const before=moveStructurePart(roots,"child","cube","before");assert.equal(before[0].id,"child");
 const cyclic=structuredClone(d);cyclic.parts[0].parent="child";assert.throws(()=>structureDocument(cyclic));const duplicate=structuredClone(d);duplicate.parts[1].id="cube";assert.throws(()=>structureDocument(duplicate));
});

test("AI creates an empty draft, preserves edited meshes and rejects invalid hierarchies",async()=>{
 const {structureProposal}=await import("../src/lib/structure-ai");
 const blank={...document(),parts:[]};
 const made=structureProposal(blank,{name:"Drone",parts:[{id:"body",name:"Body",primitive:"Cube"},{id:"rotor",name:"Rotor",parent:"body",primitive:"Cylinder",transform:{translation:[1,0,0],rotation_degrees:[0,0,0],scale:[1,0.1,1]}}]});
 assert.equal(made.id,blank.id);assert.equal(made.parts.length,2);assert.equal(made.parts[1].parent,"body");assert.equal(blank.parts.length,0);
 made.parts[0].mesh=extrudeFace(made.parts[0].mesh,1,0.25);
 const edited=structureProposal(made,{parts:[{id:"body",name:"Retained body"},{id:"rotor",name:"Rotor",parent:"body"}]});
 assert.deepEqual(edited.parts[0].mesh,made.parts[0].mesh);assert.notEqual(edited.parts[0].mesh,made.parts[0].mesh);
 assert.throws(()=>structureProposal(blank,{parts:[{id:"new",name:"No primitive"}]}),/primitive/);
 assert.throws(()=>structureProposal(blank,{parts:[{id:"a",name:"Bad",primitive:"Unknown"}]}),/primitive/);
 assert.throws(()=>structureProposal(made,{parts:[{id:"body",name:"Body",parent:"rotor"},{id:"rotor",name:"Rotor",parent:"body"}]}));
 assert.throws(()=>structureProposal(made,{parts:[{id:"body",name:"Body",transform:{translation:[0,0,0],rotation_degrees:[0,0,0],scale:[0,1,1]}}]}));
});

test("geometry thumbnails reflect composed transforms without markup or nonfinite output",async()=>{
 const {structureThumbnail}=await import("../src/lib/structure-thumbnail");
 const d=document(),first=structureThumbnail(d);assert.match(first,/^data:image\/svg\+xml,/);const svg=decodeURIComponent(first.split(",")[1]);assert.match(svg,/<polygon /);assert.doesNotMatch(svg,/NaN|Infinity/);
 d.parts.push({...structuredClone(d.parts[0]),id:"child",name:"Child",parent:"cube"});d.parts[1].transform.translation=[2,0,0];assert.notEqual(structureThumbnail(d),first);
 d.name="<script>bad</script>";assert.doesNotMatch(decodeURIComponent(structureThumbnail(d)),/<script>/);assert.doesNotMatch(decodeURIComponent(structureThumbnail({...d,parts:[]})),/NaN|Infinity/);
});

test("global part edits preserve local hierarchy and reject shear",async()=>{
 const {editStructureTransform,structureWorldTransform}=await import("../src/lib/structure");
 const parts=document().parts;parts[0].transform.translation=[2,0,0];parts.push({...structuredClone(parts[0]),id:"child",name:"Child",parent:"cube",transform:identityStructureTransform()});
 const moved=editStructureTransform(parts,"child","global","translation",[4,3,0]);
 assert.deepEqual(moved[1].transform.translation,[2,3,0]);assert.deepEqual(structureWorldTransform(moved,"child").translation,[4,3,0]);assert.deepEqual(parts[1].transform.translation,[0,0,0]);
 const local=editStructureTransform(parts,"child","local","translation",[1,0,0]);assert.deepEqual(structureWorldTransform(local,"child").translation,[3,0,0]);
 parts[0].transform.scale=[2,1,1];assert.throws(()=>editStructureTransform(parts,"child","global","rotation_degrees",[0,45,0]),/shear/);
});

test("AI primitive sizing stays local to mesh and does not flatten descendants",async()=>{
 const {structureProposal}=await import("../src/lib/structure-ai");const {structureWorldTransform}=await import("../src/lib/structure");
 const source={...document(),parts:[]};const proposed=structureProposal(source,{name:"Launcher",parts:[{id:"pad",name:"Pad",primitive:"Cylinder",size:[3,0.15,3],transform:{translation:[0,0,0],rotation_degrees:[0,0,0],scale:[1,1,1]}},{id:"turret",name:"Turret",parent:"pad",primitive:"Cube",size:[0.7,0.25,0.7],transform:{translation:[0,0.2,0],rotation_degrees:[0,0,0],scale:[1,1,1]}}]});
 assert.deepEqual(proposed.parts[0].transform.scale,[1,1,1]);assert.deepEqual(structureWorldTransform(proposed.parts,"turret").translation,[0,0.2,0]);
 const y=proposed.parts[0].mesh.vertices.map(vertex=>vertex[1]);assert.ok(Math.max(...y)-Math.min(...y)<0.151);
 const legacy=structureProposal(source,{parts:[{id:"pad",name:"Pad",primitive:"Cube",transform:{translation:[0,0,0],rotation_degrees:[0,0,0],scale:[3,0.15,3]}}]});assert.deepEqual(legacy.parts[0].transform.scale,[1,1,1]);
});

test("baking part sizes repairs descendant scale without changing each part's own dimensions",async()=>{
 const {bakeStructureSizes}=await import("../src/lib/structure");
 const d=document();d.parts[0].transform.scale=[3,0.15,3];d.parts.push({...structuredClone(d.parts[0]),id:"child",name:"Child",parent:"cube",transform:{...identityStructureTransform(),translation:[0,1,0],scale:[0.7,0.25,0.7]}});
 const repaired=bakeStructureSizes(d);assert.deepEqual(repaired.parts.map(p=>p.transform.scale),[[1,1,1],[1,1,1]]);
 assert.deepEqual(structureWorldMatrices(repaired.parts).get("child")!.elements.slice(12,15),[0,1,0]);
 assert.deepEqual(d.parts[0].transform.scale,[3,0.15,3]);
 assert.equal(repaired.parts[0].mesh.vertices[0][1],d.parts[0].mesh.vertices[0][1]*0.15);
});

test("line edits move its two endpoints; group, duplicate and delete preserve hierarchy",()=>{
 const d=document(),original=d.parts[0].mesh.vertices;
 const moved=moveMeshSelection(d.parts[0].mesh,{edge:[0,1]},[0,2,0]);
 assert.equal(moved.vertices[0][1],original[0][1]+2);assert.equal(moved.vertices[1][1],original[1][1]+2);assert.deepEqual(moved.vertices[2],original[2]);
 d.parts.push({...structuredClone(d.parts[0]),id:"child",name:"Child",parent:"cube"},{...structuredClone(d.parts[0]),id:"other",name:"Other",parent:null});
 d.parts[0].transform.translation=[2,0,0];d.parts[2].transform.translation=[-2,0,0];
 const before=structureWorldMatrices(d.parts),grouped=groupStructureParts(d,["cube","child","other"],"group"),after=structureWorldMatrices(grouped.parts);
 assert.deepEqual(grouped.parts.find(p=>p.id==="group")!.mesh,{vertices:[],faces:[]});
 for(const id of ["cube","child","other"])assert.deepEqual(after.get(id)!.elements,before.get(id)!.elements);
 assert.equal(grouped.parts.find(p=>p.id==="cube")!.parent,"group");assert.equal(grouped.parts.find(p=>p.id==="child")!.parent,"cube");
 let counter=0;const duplicated=duplicateStructureParts(grouped,["group"],()=>`copy${++counter}`);
 assert.equal(duplicated.document.parts.length,grouped.parts.length+grouped.parts.length);assert.equal(duplicated.document.parts.find(p=>p.id==="copy2")!.parent,"copy1");
 assert.deepEqual(deleteStructureParts(duplicated.document,["group"]).parts.map(p=>p.id),duplicated.ids);
 const rotated=transformStructureParts(d,["cube","other"],"rotate",[0,180,0]);
 assert.ok(Math.abs(structureWorldMatrices(rotated.parts).get("cube")!.elements[12]+2)<1e-8);
 assert.ok(Math.abs(structureWorldMatrices(rotated.parts).get("other")!.elements[12]-2)<1e-8);
 const siblings=document();siblings.parts.push({...structuredClone(siblings.parts[0]),id:"left",name:"Left",parent:"cube"},{...structuredClone(siblings.parts[0]),id:"right",name:"Right",parent:"cube"});
 siblings.parts[1].transform.translation=[-1,0,0];siblings.parts[2].transform.translation=[1,0,0];
 siblings.parts[0].transform.scale=[2,1,1];siblings.parts[0].transform.rotation_degrees=[0,30,0];const siblingWorld=structureWorldMatrices(siblings.parts);
 const nested=groupStructureParts(siblings,["left","right"],"nested");
 assert.equal(nested.parts.find(part=>part.id==="nested")!.parent,"cube");
 assert.deepEqual(nested.parts.find(part=>part.id==="nested")!.transform.translation,[0,0,0]);
 for(const id of ["left","right"])assert.deepEqual(structureWorldMatrices(nested.parts).get(id)!.elements,siblingWorld.get(id)!.elements);
 siblings.parts.push({...structuredClone(siblings.parts[0]),id:"outside",name:"Outside",parent:null});
 assert.throws(()=>groupStructureParts(siblings,["outside","right"],"invalid"),/same current parent/);
});
test("structure parts accept only a library material path",()=>{
 const part=(material?:unknown)=>({schema_version:1,id:"s",name:"S",parts:[{id:"p",name:"P",parent:null,transform:identityStructureTransform(),mesh:structurePrimitive("Cube"),...(material===undefined?{}:{material})}]});
 assert.equal(structureDocument(part("assets/stone.material.toml")).parts[0].material,"assets/stone.material.toml");
 assert.throws(()=>structureDocument(part(7)));assert.throws(()=>structureDocument(part("assets/stone.png")));
});
