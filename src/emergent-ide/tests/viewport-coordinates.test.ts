import assert from "node:assert/strict";
import test from "node:test";
import {imageRect,viewportPoint} from "../src/lib/viewport-coordinates";
test("viewport pointers match contained pixels, rejecting letterbox bars and clamping captured drags",()=>{
 const box={left:100,top:50,width:800,height:800};assert.deepEqual(imageRect(800,800),{left:0,top:175,width:800,height:450});
 assert.deepEqual(viewportPoint(box,500,450),{x:0.5,y:0.5});assert.equal(viewportPoint(box,500,100),null);
 assert.deepEqual(viewportPoint(box,1000,100,true),{x:1,y:0});
 assert.equal(viewportPoint({left:0,top:0,width:0,height:10},0,0),null);
});
