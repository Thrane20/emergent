import assert from "node:assert/strict";
import test from "node:test";
import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { InspectionDecoder, inspectionCommand } from "../src/lib/inspection-protocol";
import { InspectionSessions } from "../src/lib/inspection-session";
import type { ProjectSnapshot } from "../src/lib/project";

function packet(kind:number,payload:Buffer){const header=Buffer.alloc(12);header.write("EMI1");header.writeUInt32LE(kind,4);header.writeUInt32LE(payload.length,8);return Buffer.concat([header,payload]);}
function frame(){const body=Buffer.alloc(44);body.writeUInt32LE(1,0);body.writeUInt32LE(960,24);body.writeUInt32LE(540,28);body.writeUInt32LE(123,32);Buffer.from([137,80,78,71,13,10,26,10]).copy(body,36);return body;}

test("inspection protocol tolerates packet fragmentation, separates acknowledgements, and bounds tick-zero frames",()=>{
  const frames:number[]=[],acks:string[]=[];
  const decoder=new InspectionDecoder(value=>{frames.push(value.sequence);assert.equal(value.workerPid,123);assert.equal(value.tick,0);},(seq,error)=>acks.push(`${seq}:${error??"ok"}`));
  const ack=Buffer.alloc(8);ack.writeUInt32LE(7,0);
  const stream=Buffer.concat([packet(0,frame()),packet(1,ack)]);
  for(let offset=0;offset<stream.length;offset+=3)decoder.push(stream.subarray(offset,offset+3));
  assert.deepEqual(frames,[1]);assert.deepEqual(acks,["7:ok"]);
  const malformed=frame();malformed.writeUInt32LE(1,8);
  assert.throws(()=>decoder.push(packet(0,malformed)),/tick zero/);
  assert.throws(()=>new InspectionDecoder(()=>{},()=>{}).push(Buffer.from("bad output!!")),/protocol/);
  const big=packet(0,Buffer.alloc(0));big.writeUInt32LE(4*1024*1024+1,8);
  assert.throws(()=>new InspectionDecoder(()=>{},()=>{}).push(big),/Invalid inspection packet/);
});

test("control serialization admits only finite bounded views and authored-instance transforms",()=>{
  assert.match(inspectionCommand({op:"preview",quality:"auto",stepHeatmap:true},1),/quality="auto",step_heatmap=true/);
  assert.throws(()=>inspectionCommand({op:"preview",quality:"unbounded",stepHeatmap:false},1),/preview options/);
  assert.throws(()=>inspectionCommand({op:"preview",quality:"half",stepHeatmap:"yes"},1),/preview options/);
  assert.match(inspectionCommand({op:"view",yaw:0,pitch:0.4,distance:12,target:[0,0,0],projection:"perspective",fieldOfView:60,orthographicSize:12},1),/field_of_view_degrees=60/);
  assert.match(inspectionCommand({op:"transform",id:"sphere",translation:[2,0,0],rotation_degrees:[0,90,0],scale:[1,1,1]},2),/id="sphere"/);
  assert.match(inspectionCommand({op:"reparent",id:"sphere",parent:"cube"},3),/parent="cube"/);
  assert.doesNotMatch(inspectionCommand({op:"reparent",id:"sphere",parent:null},4),/parent=/);
  for(const value of [
    {op:"view",yaw:NaN,pitch:0,distance:12,target:[0,0,0]},
    {op:"view",yaw:0,pitch:2,distance:12,target:[0,0,0]},
    {op:"view",yaw:0,pitch:0,distance:12,target:[Infinity,0,0]},
    {op:"transform",id:'injected"',translation:[0,0,0],rotation_degrees:[0,0,0],scale:[1,1,1]},
    {op:"transform",id:"cube",translation:[0,0,0],rotation_degrees:[0,0,0],scale:[0,1,1]},
    {op:"run-game"},
    {op:"reparent",id:"cube",parent:'injected"'},
    {op:"reparent",id:"cube"},
  ])assert.throws(()=>inspectionCommand(value,3));
});

test("session stop forcibly reaps an unresponsive owned worker and rejects stale bindings",{skip:process.platform==="win32"},async()=>{
  const root=await mkdtemp(path.join(os.tmpdir(),"emergent-inspection-process-test-"));
  const previousPath=process.env.PATH;
  const manager=new InspectionSessions();let session:string|null=null;
  const scene="assets/test.scene.toml";
  const project:ProjectSnapshot={root,id:"process-test",name:"Process test",assetRoots:["assets"],entryScene:scene,files:[{path:scene,name:"test.scene.toml",kind:"scene",content:'id="test"',bytes:9,modifiedAt:"",authored:true,generated:false}],entities:[],connections:[],outputs:[]};
  try{
    const script=path.join(root,"cargo");
    await writeFile(script,`#!${process.execPath}\nrequire('node:fs').writeFileSync(require('node:path').join(__dirname,'pid'),String(process.pid));process.stdin.resume();process.on('SIGTERM',()=>{});setInterval(()=>{},1000);\n`);
    await chmod(script,0o700);process.env.PATH=`${root}${path.delimiter}${previousPath??""}`;
    const started=await manager.start(project,scene);session=started.id;
    let pid=0;
    for(let n=0;n<100&&!pid;n++){try{pid=Number(await readFile(path.join(root,"pid"),"utf8"));}catch{await new Promise(resolve=>setTimeout(resolve,20));}}
    assert.ok(pid>0,"test worker started");
    assert.equal((await manager.start(project,scene)).id,session);
    assert.throws(()=>manager.status(project,"stale"),/stale/);
    assert.throws(()=>manager.status({...project,id:"other"},session!),/another project/);
    await assert.rejects(manager.command(project,session,{op:"view"}),/not running/);
    const stopped=await manager.stop(project,session);assert.equal(stopped.state,"stopped");
    let alive=true;
    for(let n=0;n<50&&alive;n++){try{process.kill(pid,0);await new Promise(resolve=>setTimeout(resolve,20));}catch{alive=false;}}
    assert.equal(alive,false,"unresponsive worker must be killed, not merely reported stopped");
    assert.equal(manager.status(project,session).state,"stopped","late close must preserve the explicit stop result");
  }finally{
    if(session)await manager.stop(project,session);
    if(previousPath===undefined)delete process.env.PATH;else process.env.PATH=previousPath;
    await rm(root,{recursive:true,force:true});
  }
});

test("tool results are bounded and malformed gizmo controls cannot cross the protocol",()=>{
 const results:string[]=[];const decoder=new InspectionDecoder(()=>{},(_seq,_error,result)=>results.push(result?.selected??""));
 const ack=Buffer.alloc(8);ack.writeUInt32LE(4);
 const source='selected="cube"\ntool="translate"\nspace="local"\nhandles=[]\nmarkers=[]\n';
 decoder.push(packet(1,Buffer.concat([ack,Buffer.from(source)])));assert.deepEqual(results,["cube"]);
 assert.throws(()=>decoder.push(packet(1,Buffer.concat([ack,Buffer.from(source.replace('handles=[]','handles=[{axis=4,points=[]}]'))]))),/projection/);
 for(const points of [[],[[0.5,0.5]]])assert.throws(()=>decoder.push(packet(1,Buffer.concat([ack,Buffer.from(source.replace('handles=[]',`handles=[{axis=0,points=${JSON.stringify(points)}}]`))]))),/projection/);
 for(const value of [{op:"pick",x:NaN,y:0},{op:"drag",x:2,y:0},{op:"begin",axis:3,x:0.5,y:0.5},{op:"select",id:"cube",space:"screen",tool:"scale"}])assert.throws(()=>inspectionCommand(value,5));
 assert.match(inspectionCommand({op:"begin",axis:2,x:0.4,y:0.6,id:"cube",space:"local",tool:"translate",token:"test-gesture"},5),/axis=2/);
 assert.match(inspectionCommand({op:"cancel",token:"test-gesture"},6),/cancel/);
});

test("terrain-sized inspection orbits remain bounded",()=>{const camera={projection:"isometric",fieldOfView:45,orthographicSize:200};assert.match(inspectionCommand({op:"view",yaw:0,pitch:0,distance:200,target:[0,0,0],...camera},1),/view_size=200/);assert.throws(()=>inspectionCommand({op:"view",yaw:0,pitch:0,distance:30001,target:[0,0,0],...camera},1),/bounds/);});
