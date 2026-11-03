import test from "node:test";
import assert from "node:assert/strict";
import {objectLabels,classification,toggleTag,defaultLabels} from "../src/lib/object-labels";
test("labels preserve reserved bit slots and unsigned bit 31",()=>{const labels=defaultLabels();labels.tags=Array(32).fill("");labels.tags[31]="Enemy";assert.equal(objectLabels(labels).tags[0],"");assert.equal(toggleTag(0,31),2147483648);assert.equal(toggleTag(2147483648,31),0);assert.deepEqual(classification({layer:31,tags:4294967295}),{layer:31,tags:4294967295});});
test("invalid masks and label tables are refused",()=>{for(const c of [{layer:32,tags:0},{layer:0,tags:-1},{layer:0,tags:4294967296}])assert.throws(()=>classification(c));assert.throws(()=>objectLabels({...defaultLabels(),tags:Array(33).fill("")}));assert.throws(()=>objectLabels({...defaultLabels(),tags:["Enemy","enemy"]}));});

test("partial authored defaults and invalid toggle indices",()=>{assert.deepEqual(classification({tags:1}),{layer:0,tags:1});assert.throws(()=>classification(1));assert.throws(()=>toggleTag(0,32));assert.throws(()=>toggleTag(-1,0));});
