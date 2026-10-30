import assert from "node:assert/strict";
import test from "node:test";
import {lookDirection,focusView,type OrbitView} from "../src/lib/inspection-navigation";
import {inspectionCommand} from "../src/lib/inspection-protocol";
import type {SceneInstance} from "../src/lib/scene-placement";
const identity={translation:[0,0,0],rotation_degrees:[0,0,0],scale:[1,1,1]};
function node(id:string,parent:string|null=null,rotation=[0,0,0],scale=[1,1,1]):SceneInstance{return {id,parent,asset:"asset",transform:{...identity,rotation_degrees:rotation,scale},defaults:identity,overridden:false};}
test("look-at direction includes XYZ rotation, parent scale and reflection",()=>{
 const origin=node("origin"),parent=node("parent",null,[0,0,90],[2,3,4]),child=node("child","parent",[0,90,0]);
 assert.deepEqual(lookDirection(origin,[origin]),[0,0,-1]);
 const result=lookDirection(child,[parent,child])!;assert.ok(Math.abs(result[0])<1e-6&&Math.abs(result[1]+1)<1e-6&&Math.abs(result[2])<1e-6);
 assert.deepEqual(lookDirection(node("reflection",null,[0,0,0],[1,1,-1]),[]),[0,0,1]);
 assert.equal(lookDirection(node("zero",null,[0,0,0],[0,0,0]),[]),null);
 const cyclic=node("cycle","cycle");assert.equal(lookDirection(cyclic,[cyclic]),null);
});
test("focus fits bounds with native perspective or orthographic camera orientation",()=>{
 const current:OrbitView={yaw:0,pitch:0,distance:10,target:[0,0,0],projection:"perspective",fieldOfView:45,orthographicSize:12};
 const report={bounds:{center:[2,3,4],radius:2},view:{eye:[7,5,9],forward:[0,0,-1],projection:"perspective" as const,field_of_view_degrees:60,view_size:12}};
 const focused=focusView(current,report);assert.deepEqual(focused.target,[2,3,4]);assert.ok(Math.abs(focused.yaw)<1e-6);assert.ok(Math.abs(focused.distance-4.8)<1e-6);assert.equal(current.distance,10);
 const ortho=focusView(current,{...report,view:{...report.view,projection:"isometric"}});assert.equal(ortho.projection,"isometric");assert.equal(ortho.orthographicSize,4.8);
 assert.throws(()=>focusView(current,{}),/Reload/);assert.throws(()=>focusView(current,{...report,bounds:{center:[20000,0,0],radius:1}}),/bounds/);
 assert.match(inspectionCommand({op:"focus",id:"cube"},1),/op="focus"/);assert.match(inspectionCommand({op:"align",id:"cube"},2),/op="align"/);assert.throws(()=>inspectionCommand({op:"align",id:'bad"'},2));
});
