import "server-only";
import {createHash,randomUUID} from "node:crypto";
import {readFile} from "node:fs/promises";
import {parse,stringify} from "smol-toml";
import {containedFile} from "./object-files";
import {materialSettings,materialChannels,materialTextures} from "./material-settings";
import {applyChanges,checkCandidate,type FileChange} from "./statement-files";
import {foundationRoot} from "./project-code-context";
import {projectScenes} from "./project-scenes";
import {readProjectBinding,type ProjectSnapshot} from "./project";
const revision=(source:string)=>createHash("sha256").update(source).digest("hex");
export async function readMaterial(project:ProjectSnapshot,path:string){
 if(!path.endsWith(".material.toml")||!project.assetRoots.some(root=>path.startsWith(`${root}/`)))throw new Error("Choose a project library material.");
 const source=await readFile(await containedFile(project.root,path,256*1024),"utf8"),file=parse(source);
 if(Object.keys(file).some(key=>!["schema_version","id","name","material"].includes(key))||file.schema_version!==1||typeof file.id!=="string"||!file.id||file.id.length>128||typeof file.name!=="string"||!file.name.trim()||file.name.length>64||/[\p{Cc}\p{Cf}]/u.test(file.name))throw new Error(`${path}: invalid material asset.`);
 if(!file.material||typeof file.material!=="object"||Array.isArray(file.material)||Object.prototype.toString.call(file.material)!=="[object Object]")throw new Error("Material assets require a [material] table.");
 const material=materialSettings(structuredClone(file.material));
 if(material.asset||material.terrain_paint)throw new Error("Library materials cannot reference other materials or terrain paint.");
 return {path,id:file.id,name:file.name,material,revision:revision(source),source};
}
export async function materialLibrary(project:ProjectSnapshot){
 return Promise.all(project.files.filter(file=>file.path.endsWith(".material.toml")).map(async file=>{const {source,...item}=await readMaterial(project,file.path);void source;return item;}));
}
const state=globalThis as typeof globalThis&{emergentScenePlacementActive?:boolean};
export async function saveMaterial(project:ProjectSnapshot,input:{path?:unknown;revision?:unknown;name:unknown;material:unknown}){
 if(state.emergentScenePlacementActive)throw new Error("Another scene edit is being saved.");state.emergentScenePlacementActive=true;
 try{
 if(typeof input.name!=="string"||!input.name.trim()||input.name.length>64||/[\p{Cc}\p{Cf}]/u.test(input.name))throw new Error("Use a material name of 1–64 printable characters.");
 if(input.path!==undefined&&typeof input.path!=="string")throw new Error("Choose a material asset path.");
 const material=materialSettings(input.material);if(material.asset||material.terrain_paint)throw new Error("Edit inline surface settings in the library.");
 const current=typeof input.path==="string"?await readMaterial(project,input.path):undefined;
 if(current&&input.revision!==current.revision)throw new Error("Material changed externally. Reopen before saving.");
 if(!project.assetRoots.length)throw new Error("Project has no asset root.");
 const id=current?.id??randomUUID(),path=current?.path??`${project.assetRoots[0]}/materials/${id}.material.toml`;
 const content=stringify({schema_version:1,id,name:input.name.trim(),material});
 const changes:FileChange[]=[{path,before:current?.source??null,content}];
 const dependencies=[...materialChannels.flatMap(channel=>material[`${channel}_map`]?.asset?[material[`${channel}_map`]!.asset]:[]),...materialTextures(material).map(map=>map.asset)];
 const stamps=await Promise.all(dependencies.map(async path=>({path,bytes:await readFile(await containedFile(project.root,path,16*1024*1024))})));
 // Validate an unassigned material through the same native loader as scene objects.
 const token=randomUUID(),root=project.assetRoots[0],probeEntity=`${root}/materials/${token}.entity.toml`,probeScene=`${root}/materials/${token}.scene.toml`;
 const probes:FileChange[]=[{path:probeEntity,before:null,content:stringify({schema_version:1,id:token,kind:"primitive",geometry:{primitive:"cube",half_extent:0.5,color:[1,1,1,1]},material:{asset:path}})},{path:probeScene,before:null,content:stringify({schema_version:1,id:token,entities:[{id:token,asset:probeEntity}]})}];
 const scenes=projectScenes(project).map(scene=>scene.path);if(scenes.length>126)throw new Error("Material validation supports at most 126 project scenes.");
 await checkCandidate(project,[...changes,...probes],{root:await foundationRoot(project)},[...scenes,probeScene]);
 const binding=await readProjectBinding();if(binding.id!==project.id||binding.root!==project.root)throw new Error("Project changed during validation.");
 for(const stamp of stamps)if(!(await readFile(await containedFile(project.root,stamp.path,16*1024*1024))).equals(stamp.bytes))throw new Error("Material input changed during validation. Reopen before saving.");
 await applyChanges(project,changes);return {path,id,name:input.name.trim(),material,revision:revision(content)};
 }finally{state.emergentScenePlacementActive=false;}
}
