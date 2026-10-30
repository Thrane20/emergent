import {parse,stringify} from "smol-toml";
export type CameraSettings={projection:"perspective"|"isometric";field_of_view_degrees?:number;view_size?:number;near:number;far:number};
export function cameraSettings(value:unknown):CameraSettings {
 if(!value||typeof value!=="object"||Array.isArray(value))throw new Error("Camera settings are required.");
 const input=value as Record<string,unknown>,s={...input,projection:input.projection==="orthographic"?"isometric":input.projection} as CameraSettings;
 if(Object.keys(input).some(key=>!["projection","field_of_view_degrees","view_size","near","far"].includes(key))||!["perspective","isometric"].includes(s.projection))throw new Error("Choose Perspective or Orthographic.");
 if(typeof s.near!=="number"||typeof s.far!=="number"||!Number.isFinite(s.near)||!Number.isFinite(s.far)||s.near<=0||s.far<=s.near||s.far>1000000||Math.fround(s.near)<=0||Math.fround(s.far)<=Math.fround(s.near))throw new Error("Clipping planes require 0 < near < far ≤ 1000000.");
 if(s.projection==="perspective"){
  if(typeof s.field_of_view_degrees!=="number"||!Number.isFinite(s.field_of_view_degrees)||s.field_of_view_degrees<1||s.field_of_view_degrees>=179||s.view_size!==undefined)throw new Error("Perspective cameras require FOV from 1 to less than 179 degrees.");
 }else if(typeof s.view_size!=="number"||!Number.isFinite(s.view_size)||s.view_size<0.001||s.view_size>1000000||s.field_of_view_degrees!==undefined)throw new Error("Orthographic cameras require a view height of 0.001–1000000.");
 return s;
}
export function cameraAssetSettings(source:string):CameraSettings|null {
 const document=parse(source);return document.kind==="camera"?cameraSettings(document.camera):null;
}
export function replaceCameraAsset(source:string,value:CameraSettings) {
 const document=parse(source);if(document.kind!=="camera")throw new Error("Choose a camera asset.");
 const settings=cameraSettings(value),header=/^\[camera\][ \t]*(?:#.*)?\r?$/m.exec(source);if(!header)throw new Error("Camera editing requires a [camera] table. Use TOML to convert this asset.");
 const next=source.slice(header.index+header[0].length).search(/^\s*\[/m),end=next<0?source.length:header.index+header[0].length+next;
 const comments=source.slice(header.index,end).split("\n").map(line=>line.match(/#.*$/)?.[0]).filter(Boolean).join("\n");
 const content=source.slice(0,header.index)+(comments?comments+"\n":"")+stringify({camera:settings})+"\n"+source.slice(end);
 document.camera=parse(stringify({camera:settings})).camera;
 if(JSON.stringify(parse(content))!==JSON.stringify(document))throw new Error("Could not safely update the camera. Use the TOML editor.");
 return content;
}
