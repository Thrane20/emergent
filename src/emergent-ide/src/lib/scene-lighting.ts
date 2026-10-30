import "server-only";
import {createHash} from "node:crypto";
import {readFile} from "node:fs/promises";
import {isDeepStrictEqual} from "node:util";
import {parse,stringify} from "smol-toml";
import {containedFile} from "./object-files";
import {lightingSettings,type LightingSettings} from "./lighting-settings";
import {projectScenes} from "./project-scenes";
import {projectImages} from "./image-library";
import {readProjectSnapshot,type ProjectSnapshot} from "./project";
import {foundationRoot} from "./project-code-context";
import {applyChanges,checkCandidate} from "./statement-files";

async function skyboxStamp(project:ProjectSnapshot,settings:LightingSettings){
 if(!settings.skybox)return "";
 if(!project.assetRoots.some(root=>settings.skybox!.startsWith(`${root}/`)))throw new Error("Skybox is outside project asset roots.");
 return createHash("sha256").update(await readFile(await containedFile(project.root,settings.skybox,16*1024*1024))).digest("hex");
}
export async function readSceneLighting(project:ProjectSnapshot,path:string){
 if(!projectScenes(project).some(scene=>scene.path===path))throw new Error("Choose a scene from this project.");
 const source=await readFile(await containedFile(project.root,path,256*1024),"utf8"),settings=lightingSettings(parse(source).environment);
 const revision=createHash("sha256").update(source).update(await skyboxStamp(project,settings)).digest("hex");
 return {path,source,settings,revision};
}
export function replaceSceneLighting(source:string,value:unknown){
 const settings=lightingSettings(value),serialized={...settings} as Partial<LightingSettings>;if(!settings.skybox)delete serialized.skybox;
 const before=parse(source),expected={...before,environment:serialized},table=stringify({environment:serialized}),header=/^\[environment\][ \t]*(?:#[^\r\n]*)?\r?$/m.exec(source);
 let content:string;
 if(header){const start=header.index,end=source.slice(start+header[0].length).search(/^\s*\[/m),finish=end<0?source.length:start+header[0].length+end;content=source.slice(0,start)+table+"\n"+source.slice(finish);}
 else{if(before.environment!==undefined)throw new Error("Use the TOML editor to convert inline environment settings into an [environment] table.");content=source.trimEnd()+"\n\n"+table;}
 if(Buffer.byteLength(content)>256*1024||!isDeepStrictEqual(parse(content),parse(stringify(expected))))throw new Error("Could not safely update scene lighting. Use the TOML editor.");
 return content;
}
const state=globalThis as typeof globalThis&{emergentScenePlacementActive?:boolean};
export async function saveSceneLighting(project:ProjectSnapshot,input:{path:string;revision:string;settings:unknown}){
 if(state.emergentScenePlacementActive)throw new Error("Another scene edit is saving.");state.emergentScenePlacementActive=true;
 try{
  const current=await readSceneLighting(project,input.path);if(current.revision!==input.revision)throw new Error("Scene lighting changed externally. Reload before saving.");
  const settings=lightingSettings(input.settings);if(settings.skybox&&!projectImages(project).some(image=>image.path===settings.skybox&&image.kind==="skybox"))throw new Error("Choose a Skybox from this project's image library.");
  const stamp=await skyboxStamp(project,settings),change={path:input.path,before:current.source,content:replaceSceneLighting(current.source,settings)};
  await checkCandidate(project,[change],{root:await foundationRoot(project)},[input.path]);
  const fresh=await readProjectSnapshot();if(fresh.id!==project.id||fresh.root!==project.root)throw new Error("Linked project changed. No lighting saved.");
  if((await readSceneLighting(fresh,input.path)).revision!==current.revision||await skyboxStamp(fresh,settings)!==stamp)throw new Error("Scene or skybox changed during validation. Reload before saving.");
  await applyChanges(project,[change]);const saved=await readSceneLighting(fresh,input.path);return {path:saved.path,settings:saved.settings,revision:saved.revision};
 }finally{state.emergentScenePlacementActive=false;}
}
