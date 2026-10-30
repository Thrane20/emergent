import assert from "node:assert/strict";
import test from "node:test";
import {parse,stringify} from "smol-toml";
import {cameraSettings,cameraAssetSettings,replaceCameraAsset} from "../src/lib/camera-settings";
import {replaceInstanceCamera} from "../src/lib/scene-placement";
import {inspectionCommand,InspectionDecoder} from "../src/lib/inspection-protocol";
const perspective={projection:"perspective" as const,field_of_view_degrees:60,near:0.1,far:1000};
test("camera settings enforce projection-specific fields and usable clipping planes",()=>{
 assert.deepEqual(cameraSettings(perspective),perspective);
 assert.equal(cameraSettings({projection:"orthographic",view_size:12,near:1,far:10}).projection,"isometric");
 for(const patch of [{near:0},{near:10,far:1},{near:1,far:1.00000000001},{far:Infinity},{far:1000001},{field_of_view_degrees:179},{view_size:3},{projection:"unknown"}])assert.throws(()=>cameraSettings({...perspective,...patch}));
 assert.throws(()=>cameraSettings({projection:"isometric",view_size:0,near:1,far:10}));
});
test("camera asset and instance edits preserve other data and reset to inheritance",()=>{
 const source="# metadata\nschema_version=1\nid='cam'\nname='Camera'\nkind='camera'\n[transform]\ntranslation=[7,5,9]\n[camera]\n# clip note\nprojection='perspective'\nfield_of_view_degrees=60\nnear=0.1\nfar=1000\n";
 const ortho=cameraSettings({projection:"isometric",view_size:8,near:0.5,far:20});
 const changed=replaceCameraAsset(source,ortho);assert.ok(changed.includes("# metadata"));assert.ok(changed.includes("# clip note"));assert.deepEqual(cameraAssetSettings(changed),ortho);assert.deepEqual(parse(changed).transform,parse(source).transform);
 const scene='# retain\nschema_version=1\nid="main"\n[[entities]]\nid="cam"\nasset="assets/cam.entity.toml"\n[entities.transform]\ntranslation=[1,2,3]\n[[entities]]\nid="other"\nasset="assets/cam.entity.toml"\n';
 const updated=replaceInstanceCamera(scene,"cam",ortho),entries=parse(updated).entities as {camera?:unknown;transform?:unknown}[];assert.deepEqual(JSON.parse(JSON.stringify(entries[0].camera)),ortho);assert.equal(entries[1].camera,undefined);assert.ok(updated.startsWith("# retain"));
 const reset=replaceInstanceCamera(updated,"cam",null);assert.deepEqual(parse(reset),parse(scene));assert.throws(()=>replaceInstanceCamera(scene,"missing",ortho),/no longer exists/);
});
test("inspection commands validate camera edits and bound frustum overlays",()=>{
 assert.match(inspectionCommand({op:"camera",id:"camera",camera:perspective},1),/field_of_view_degrees=60/);
 assert.throws(()=>inspectionCommand({op:"camera",id:"camera",camera:{...perspective,far:0}},1));
 const message=(frustum:{id:string;segments:number[][][]})=>{const body=Buffer.concat([Buffer.alloc(8),Buffer.from(stringify({selected:"camera",tool:"translate",space:"local",handles:[],markers:[],frustum}))]),header=Buffer.alloc(12);header.write("EMI1");header.writeUInt32LE(1,4);header.writeUInt32LE(body.length,8);return Buffer.concat([header,body]);};
 let count=0;const decoder=new InspectionDecoder(()=>{},(_seq,_error,result)=>{count=result?.frustum?.segments.length??0;});decoder.push(message({id:"camera",segments:[[[0,0],[1,1]]]}));assert.equal(count,1);
 assert.throws(()=>decoder.push(message({id:"camera",segments:Array(13).fill([[0,0],[1,1]])})),/frustum/);
});
