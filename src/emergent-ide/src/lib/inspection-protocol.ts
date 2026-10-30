import {cameraSettings,type CameraSettings} from "./camera-settings";
import {parse} from "smol-toml";
export type InspectionFrame = { sequence: number; command: number; tick: number; renderUs: number; encodeUs: number; width: number; height: number; workerPid: number; png: Buffer };
export type ToolResult = {bounds?:{center:number[];radius:number};view?:{eye:number[];forward:number[];projection:"perspective"|"isometric";field_of_view_degrees:number;view_size:number};placement?:{translation:number[];rotation_degrees:number[];scale:number[]};frustum?:{id:string;segments:number[][][]};selected:string;tool:string;space:string;origin?:number[];handles:{axis:number;points:number[][]}[];markers:{id:string;point:number[]}[];transform?:{translation:number[];rotation_degrees:number[];scale:number[]}};
export type InspectionControl = {op:"preview";quality:"auto"|"full"|"half";stepHeatmap:boolean} | {op:"focus"|"align";id:string} | {op:"camera";id:string;camera:CameraSettings} | { op: "view"; yaw: number; pitch: number; distance: number; target: number[]; projection: "perspective" | "isometric"; fieldOfView: number; orthographicSize: number } | { op: "transform"; id: string; translation: number[]; rotation_degrees: number[]; scale: number[] } | {op:"reparent";id:string;parent:string|null} | {op:"pick";x:number;y:number} | {op:"drag";token:string;x:number;y:number} | {op:"select";id:string|null;space:"local"|"global";tool:"translate"|"rotate"|"scale"} | {op:"begin";id:string;space:string;tool:string;token:string;axis:number;x:number;y:number} | {op:"finish"|"cancel";token:string};
const MAX_PACKET = 4 * 1024 * 1024;

export class InspectionDecoder {
  private buffer: Buffer = Buffer.alloc(0);
  constructor(private frame: (value: InspectionFrame) => void, private ack: (seq: number, error: string | null, result?:ToolResult) => void) {}
  push(chunk: Buffer) {
    if (this.buffer.length + chunk.length > MAX_PACKET * 2) throw new Error("Inspection output exceeded its bounded buffer.");
    this.buffer = Buffer.concat([this.buffer,chunk]);
    while (this.buffer.length >= 12) {
      if (this.buffer.toString("ascii",0,4) !== "EMI1") throw new Error("Game output does not implement the inspection protocol.");
      const kind = this.buffer.readUInt32LE(4), size = this.buffer.readUInt32LE(8);
      if (size > MAX_PACKET || (kind !== 0 && kind !== 1)) throw new Error("Invalid inspection packet.");
      if (this.buffer.length < size + 12) return;
      const bytes = this.buffer.subarray(12,size+12);
      this.buffer = this.buffer.subarray(size+12);
      if (kind === 1) {
        if (bytes.length < 8 || bytes.length > 256*1024+8) throw new Error("Invalid inspection acknowledgement.");
        const failed=bytes.readUInt32LE(4)!==0;
        if(failed&&bytes.length>2056)throw new Error("Invalid inspection error length.");
        this.ack(bytes.readUInt32LE(0),failed?bytes.toString("utf8",8):null,!failed&&bytes.length>8?toolResult(parse(bytes.toString("utf8",8))):undefined);
      } else {
        if (bytes.length < 44 || bytes.readBigUInt64LE(8) !== BigInt(0) || bytes.readUInt32LE(24) !== 960 || bytes.readUInt32LE(28) !== 540 || !bytes.subarray(36,44).equals(Buffer.from([137,80,78,71,13,10,26,10]))) throw new Error("Inspection must produce bounded 960×540 PNG frames at tick zero.");
        this.frame({sequence:bytes.readUInt32LE(0),command:bytes.readUInt32LE(4),tick:0,renderUs:bytes.readUInt32LE(16),encodeUs:bytes.readUInt32LE(20),width:960,height:540,workerPid:bytes.readUInt32LE(32),png:Buffer.from(bytes.subarray(36))});
      }
    }
  }
}

export function inspectionCommand(value: unknown, seq: number): string {
  if (!value || typeof value !== "object") throw new Error("An inspection command is required.");
  const input = value as Record<string,unknown>;
  const vector = (name: string) => {
    const result = input[name];
    if (!Array.isArray(result) || result.length !== 3 || !result.every(n => typeof n === "number" && Number.isFinite(n) && Math.abs(n) <= 3.4028234e38)) throw new Error(`Invalid ${name} vector.`);
    return result as number[];
  };
  const token=()=>{if(typeof input.token!=="string"||!/^[a-z0-9-]{1,64}$/i.test(input.token))throw new Error("Invalid gesture token.");return `,token=${JSON.stringify(input.token)}`;};
  let fields: string;
  if(input.op==="preview"){
    if(!["auto","full","half"].includes(input.quality as string)||typeof input.stepHeatmap!=="boolean")throw new Error("Invalid terrain preview options.");
    fields=`op="preview",quality=${JSON.stringify(input.quality)},step_heatmap=${input.stepHeatmap}`;
  } else if(input.op==="focus"||input.op==="align"){
    if(typeof input.id!=="string"||!/^[a-z0-9_.-]{1,128}$/i.test(input.id))throw new Error("Invalid scene instance ID.");
    fields=`op=${JSON.stringify(input.op)},id=${JSON.stringify(input.id)}`;
  } else if(input.op==="camera"){
    if(typeof input.id!=="string"||!/^[a-z0-9_.-]{1,128}$/i.test(input.id))throw new Error("Invalid camera instance ID.");
    const camera=cameraSettings(input.camera);
    fields=`op="camera",id=${JSON.stringify(input.id)},projection=${JSON.stringify(camera.projection)},near=${camera.near},far=${camera.far}${camera.projection==="perspective"?`,field_of_view_degrees=${camera.field_of_view_degrees}`:`,view_size=${camera.view_size}`}`;
  } else if (input.op === "view") {
    if (![input.yaw,input.pitch,input.distance].every(n => typeof n === "number" && Number.isFinite(n)) || Math.abs(input.yaw as number)>10000 || Math.abs(input.pitch as number)>1.45 || (input.distance as number)<0.25 || (input.distance as number)>30000) throw new Error("Camera is outside inspection bounds.");
    const target=vector("target"); if (target.some(n=>Math.abs(n)>10000)) throw new Error("Camera target is outside inspection bounds.");
    if(!["perspective","isometric"].includes(input.projection as string))throw new Error("Camera projection is unsupported.");
    if(typeof input.fieldOfView!=="number"||!Number.isFinite(input.fieldOfView)||input.fieldOfView<1||input.fieldOfView>=179)throw new Error("Camera field of view is outside inspection bounds.");
    if(typeof input.orthographicSize!=="number"||!Number.isFinite(input.orthographicSize)||input.orthographicSize<0.001||input.orthographicSize>1000000)throw new Error("Camera orthographic size is outside inspection bounds.");
    fields=`op="view", yaw=${input.yaw}, pitch=${input.pitch}, distance=${input.distance}, target=${JSON.stringify(target)}, projection=${JSON.stringify(input.projection)}, field_of_view_degrees=${input.fieldOfView}, view_size=${input.orthographicSize}`;
  } else if (input.op === "transform") {
    if (typeof input.id !== "string" || !/^[a-z0-9_.-]{1,128}$/i.test(input.id)) throw new Error("Invalid scene instance ID.");
    const scale=vector("scale");if(scale.some(n=>Math.abs(n)<0.0001))throw new Error("Scale axes must be non-zero.");
    fields=`op="transform", id=${JSON.stringify(input.id)}, translation=${JSON.stringify(vector("translation"))}, rotation_degrees=${JSON.stringify(vector("rotation_degrees"))}, scale=${JSON.stringify(scale)}`;
  } else if(input.op==="reparent"){
    if(typeof input.id!=="string"||!/^[a-z0-9_.-]{1,128}$/i.test(input.id)||(input.parent!==null&&(typeof input.parent!=="string"||!/^[a-z0-9_.-]{1,128}$/i.test(input.parent))))throw new Error("Invalid parent or instance ID.");
    fields=`op="reparent",id=${JSON.stringify(input.id)}${input.parent===null?"":`,parent=${JSON.stringify(input.parent)}`}`;
  } else if(input.op==="select"){
    if(input.id!==null&&(typeof input.id!=="string"||!/^[a-z0-9_.-]{1,128}$/i.test(input.id)))throw new Error("Invalid scene instance ID.");
    if(!["local","global"].includes(input.space as string)||!["translate","rotate","scale"].includes(input.tool as string))throw new Error("Invalid gizmo mode.");
    fields=`op="select",space=${JSON.stringify(input.space)},tool=${JSON.stringify(input.tool)}${input.id===null?"":`,id=${JSON.stringify(input.id)}`}`;
  } else if(input.op==="pick"||input.op==="begin"||input.op==="drag"){
    if(![input.x,input.y].every(n=>typeof n==="number"&&Number.isFinite(n)&&n>=0&&n<=1))throw new Error("Pointer is outside the viewport.");
    if(input.op==="begin"&&(!Number.isInteger(input.axis)||(input.axis as number)<0||(input.axis as number)>2))throw new Error("Invalid gizmo axis.");
    if(input.op==="begin"&&(typeof input.id!=="string"||!/^[a-z0-9_.-]{1,128}$/i.test(input.id)||!["local","global"].includes(input.space as string)||!["translate","rotate","scale"].includes(input.tool as string)))throw new Error("Invalid gizmo selection.");
    fields=`op=${JSON.stringify(input.op)},x=${input.x},y=${input.y}${input.op==="pick"?"":token()}${input.op==="begin"?`,axis=${input.axis},id=${JSON.stringify(input.id)},space=${JSON.stringify(input.space)},tool=${JSON.stringify(input.tool)}`:""}`;
  } else if(input.op==="finish"||input.op==="cancel")fields=`op=${JSON.stringify(input.op)}${token()}`;
  else throw new Error("Unsupported inspection command.");
  return `request={seq=${seq}, ${fields}}\n`;
}

function toolResult(value:unknown):ToolResult {
  const result=value as ToolResult;
  const point=(v:unknown)=>Array.isArray(v)&&v.length===2&&v.every(n=>typeof n==="number"&&Number.isFinite(n)&&Math.abs(n)<1e6);
  if(!result||!["translate","rotate","scale"].includes(result.tool)||!["local","global"].includes(result.space)||typeof result.selected!=="string"||result.selected.length>128||!Array.isArray(result.handles)||result.handles.length>3||!Array.isArray(result.markers)||result.markers.length>1024||result.origin&&!point(result.origin))throw new Error("Invalid tool result.");
  for(const h of result.handles)if(!Number.isInteger(h.axis)||h.axis<0||h.axis>2||!Array.isArray(h.points)||h.points.length<2||h.points.length>65||!h.points.every(point))throw new Error("Invalid gizmo projection.");
  for(const marker of result.markers)if(typeof marker.id!=="string"||marker.id.length>128||!point(marker.point))throw new Error("Invalid scene marker.");
  for(const transform of [result.transform,result.placement])if(transform)for(const key of ["translation","rotation_degrees","scale"] as const){const v=transform[key];if(!Array.isArray(v)||v.length!==3||!v.every(n=>typeof n==="number"&&Number.isFinite(n)))throw new Error("Invalid tool transform.");}
  if(result.frustum&&(typeof result.frustum.id!=="string"||result.frustum.id.length>128||!Array.isArray(result.frustum.segments)||result.frustum.segments.length>12||!result.frustum.segments.every(segment=>Array.isArray(segment)&&segment.length===2&&segment.every(point))))throw new Error("Invalid camera frustum overlay.");
  const vector3=(v:unknown)=>Array.isArray(v)&&v.length===3&&v.every(n=>typeof n==="number"&&Number.isFinite(n)&&Math.abs(n)<=3.4028234e38);
  if(result.bounds&&(!vector3(result.bounds.center)||!Number.isFinite(result.bounds.radius)||result.bounds.radius<0||result.bounds.radius>3.4028234e38))throw new Error("Invalid selection bounds.");
  if(result.view&&(!vector3(result.view.eye)||!vector3(result.view.forward)||Math.abs(Math.hypot(...result.view.forward)-1)>0.001||!["perspective","isometric"].includes(result.view.projection)||!Number.isFinite(result.view.field_of_view_degrees)||result.view.field_of_view_degrees<0.999||result.view.field_of_view_degrees>=179||!Number.isFinite(result.view.view_size)||result.view.view_size<0.001||result.view.view_size>1000000))throw new Error("Invalid inspection view.");
  return result;
}
