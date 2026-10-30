import "server-only";
import {createHash,randomUUID} from "node:crypto";
import {readFile} from "node:fs/promises";
import {parse,stringify} from "smol-toml";
import {containedFile} from "./object-files";
import {structureDocument} from "./structure";
import {readMaterial} from "./material-assets";
import {projectScenes} from "./project-scenes";
import {applyChanges,checkCandidate,type FileChange} from "./statement-files";
import {foundationRoot} from "./project-code-context";
import {readProjectBinding,type ProjectSnapshot} from "./project";
const revision=(value:string)=>createHash("sha256").update(value).digest("hex");
async function source(project:ProjectSnapshot,path:string,limit:number){if(!project.assetRoots.some(root=>path.startsWith(`${root}/`)))throw new Error("Choose a structure under a project asset root.");return readFile(await containedFile(project.root,path,limit),"utf8");}
export async function readStructureAsset(project:ProjectSnapshot,assetPath:string){
 if(!assetPath.endsWith(".entity.toml"))throw new Error("Choose a structure entity asset.");const entitySource=await source(project,assetPath,256*1024),entity=parse(entitySource);
 if(entity.schema_version!==1||entity.kind!=="structure"||typeof entity.structure!=="string"||!entity.structure.endsWith(".structure.toml"))throw new Error("Choose an Object Builder structure.");
 const structurePath=entity.structure,structureSource=await source(project,structurePath,2*1024*1024),raw=parse(structureSource);
 const normalized={...raw,parts:Array.isArray(raw.parts)?raw.parts.map(part=>part&&typeof part==="object"&&!Array.isArray(part)?{...part,parent:"parent" in part?part.parent:null}:part):raw.parts};
 const document=structureDocument(structuredClone(normalized)),material=entity.material as {asset?:string}|undefined;
 const library=material?.asset?await readMaterial(project,material.asset):undefined;
 return {assetPath,structurePath,entitySource,structureSource,entity,document,material:library?.path??(entity.material===undefined?null:undefined),revision:revision(JSON.stringify([entitySource,structureSource,library?.source]))};
}
export function structureLibrary(project:ProjectSnapshot){return project.files.filter(file=>file.path.endsWith(".entity.toml")&&file.content&&parse(file.content).kind==="structure").map(file=>({path:file.path,name:typeof parse(file.content!).name==="string"?String(parse(file.content!).name):file.name}));}
export async function prepareStructureSave(project:ProjectSnapshot,input:{document:unknown;assetPath?:unknown;revision?:unknown;material?:unknown;scenePath?:unknown;sceneRevision?:unknown}){
 const document=structureDocument(input.document);if(!document.parts.length)throw new Error("Add a primitive before saving.");
 if(input.assetPath!==undefined&&typeof input.assetPath!=="string")throw new Error("Choose a structure asset path.");
 const current=typeof input.assetPath==="string"?await readStructureAsset(project,input.assetPath):undefined;
 if(current&&input.revision!==current.revision)throw new Error("Structure or shared material changed externally. Reopen before saving.");
 if(input.material!==undefined&&input.material!==null&&typeof input.material!=="string")throw new Error("Choose a library material.");
 const material=input.material===undefined?current?.material:input.material;
 const library=typeof material==="string"?await readMaterial(project,material):undefined;
 const root=project.assetRoots[0];if(!root)throw new Error("Project has no asset root.");const id=current?.entity.id??randomUUID(),assetPath=current?.assetPath??`${root}/structures/${id}.entity.toml`,structurePath=current?.structurePath??`${root}/structures/${id}.structure.toml`;
 const authored={...document,parts:document.parts.map(part=>({...part,parent:part.parent??undefined}))};
 const structureSource=stringify(authored);const entity:Record<string,unknown>={...current?.entity,schema_version:1,id,name:document.name,kind:"structure",structure:structurePath};
 if(library)entity.material={asset:library.path};else if(input.material!==undefined)delete entity.material;
 const entitySource=stringify(entity);if(Buffer.byteLength(structureSource)>2*1024*1024||Buffer.byteLength(entitySource)>256*1024)throw new Error("Structure exceeds its source size budget.");
 const changes:FileChange[]=[{path:structurePath,before:current?.structureSource??null,content:structureSource},{path:assetPath,before:current?.entitySource??null,content:entitySource}];
 if(library)changes.push({path:library.path,before:library.source,content:library.source});
 let instanceId:string|undefined,sceneRevision:string|undefined;
 if(input.scenePath!==undefined){
  if(typeof input.scenePath!=="string"||!projectScenes(project).some(scene=>scene.path===input.scenePath))throw new Error("Choose a scene in this project.");
  const before=await source(project,input.scenePath,256*1024);if(input.sceneRevision!==revision(before))throw new Error("Scene changed externally. Refresh the destination scene before adding this object.");
  const scene=parse(before);if(scene.schema_version!==1||!Array.isArray(scene.entities)||scene.entities.length>=1024)throw new Error("Choose a schema-1 scene with fewer than 1024 instances.");
  instanceId=randomUUID();const first=before.search(/^\[/m),header=first<0?before:before.slice(0,first),tail=first<0?"":before.slice(first);
  const content=header.replace(/^entities\s*=\s*\[\s*\]\s*$/m,"")+tail+`\n[[entities]]\nid = ${JSON.stringify(instanceId)}\nasset = ${JSON.stringify(assetPath)}\n`;
  parse(content);if(Buffer.byteLength(content)>256*1024)throw new Error("Scene exceeds 256 KiB.");changes.push({path:input.scenePath,before,content});sceneRevision=revision(content);
 }
 return {changes,result:{assetPath,structurePath,instanceId,scenePath:input.scenePath as string|undefined,sceneRevision}};
}
const state=globalThis as typeof globalThis&{emergentScenePlacementActive?:boolean};
export async function saveStructure(project:ProjectSnapshot,input:Parameters<typeof prepareStructureSave>[1]){
 if(state.emergentScenePlacementActive)throw new Error("Another scene edit is being saved.");state.emergentScenePlacementActive=true;
 try{
  const {changes,result}=await prepareStructureSave(project,input),scenes=projectScenes(project).map(scene=>scene.path);if(scenes.length>126)throw new Error("Structure validation supports at most 126 project scenes.");
  const token=randomUUID(),probe=`${project.assetRoots[0]}/structures/${token}.scene.toml`;
  await checkCandidate(project,[...changes,{path:probe,before:null,content:stringify({schema_version:1,id:token,entities:[{id:token,asset:result.assetPath}]})}],{root:await foundationRoot(project)},[...scenes,probe]);
  const binding=await readProjectBinding();if(binding.id!==project.id||binding.root!==project.root)throw new Error("Linked project changed during validation.");
  await applyChanges(project,changes);const saved=await readStructureAsset(project,result.assetPath);return {...result,revision:saved.revision,document:saved.document,material:saved.material};
 }finally{state.emergentScenePlacementActive=false;}
}
