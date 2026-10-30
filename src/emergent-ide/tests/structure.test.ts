import assert from "node:assert/strict";
import test from "node:test";
import {faceNormal,extrudeFace,moveMeshSelection,moveStructurePart,identityStructureTransform,structureDocument,structureWorldMatrices,triangulateFace,type StructureDocument} from "../src/lib/structure";
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
