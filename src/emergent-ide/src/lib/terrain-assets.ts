import "server-only";
import {createHash,randomUUID} from "node:crypto";
import {isDeepStrictEqual} from "node:util";
import {readFile} from "node:fs/promises";
import {parse,stringify} from "smol-toml";
import {containedFile} from "./object-files";
import {readProjectBinding,type ProjectSnapshot} from "./project";
import {projectScenes} from "./project-scenes";
import {terrainPaint} from "./terrain-paint";
import {readMaterial} from "./material-assets";
import {materialChannels,materialTextures} from "./material-settings";
import {terrainHeights} from "./terrain-brush";
import {TERRAIN_DATA_MAX_BYTES,terrainSettings} from "./terrain";
import {applyChanges,checkCandidate,type FileChange} from "./statement-files";
import {foundationRoot} from "./project-code-context";
export const terrainSceneRevision=(source:string)=>createHash("sha256").update(source).digest("hex");
export async function terrainPaintInputs(project:ProjectSnapshot,paint:ReturnType<typeof terrainPaint>|undefined){
 const inputs:unknown[]=[];
 for(const path of paint?.materials??[]){const item=await readMaterial(project,path);inputs.push(item.source);for(const channel of materialChannels){const map=item.material[`${channel}_map`];if(map)inputs.push(await readFile(await containedFile(project.root,map.asset,256*1024),"utf8"));}for(const map of materialTextures(item.material))inputs.push(terrainSceneRevision((await readFile(await containedFile(project.root,map.asset,16*1024*1024))).toString("base64")));}
 return inputs;
}
/** Resolve an instance through its scene, never trust a client-supplied asset path. */
export async function readTerrainEdit(project:ProjectSnapshot,scenePath:unknown,instanceId:unknown){
 if(typeof scenePath!=="string"||!projectScenes(project).some(scene=>scene.path===scenePath)||typeof instanceId!=="string")throw new Error("Choose a terrain instance in a project scene.");
 const sceneSource=await readFile(await containedFile(project.root,scenePath,256*1024),"utf8"),scene=parse(sceneSource);
 if(scene.schema_version!==1||!Array.isArray(scene.entities))throw new Error("Choose a schema-1 scene.");
 const instance=scene.entities.find(entry=>entry&&typeof entry==="object"&&!Array.isArray(entry)&&"id" in entry&&entry.id===instanceId) as Record<string,unknown>|undefined;
 if(!instance||typeof instance.asset!=="string"||!instance.asset.endsWith(".entity.toml"))throw new Error("Terrain instance no longer exists.");
 const loaded=await readTerrainAsset(project,instance.asset);
 const revision=terrainSceneRevision(JSON.stringify([scenePath,sceneSource,loaded.assetPath,loaded.assetSource,loaded.heightPath,loaded.heightSource,loaded.paintPath,loaded.paintSource,loaded.inputs]));
 return {...loaded,instanceId,scenePath,sceneSource,sceneRevision:terrainSceneRevision(sceneSource),assetRevision:loaded.revision,revision,name:typeof instance.name==="string"?instance.name:loaded.name};
}

export async function readTerrainAsset(project:ProjectSnapshot,path:unknown){
 async function assetSource(path:unknown,limit:number){
  if(typeof path!=="string"||!project.assetRoots.some(root=>path.startsWith(`${root}/`)))throw new Error("Terrain must reference project assets.");
  return readFile(await containedFile(project.root,path,limit),"utf8");
 }
 if(typeof path!=="string"||!path.endsWith(".entity.toml"))throw new Error("Choose a terrain entity asset.");
 const assetPath=path,assetSourceText=await assetSource(assetPath,256*1024),asset=parse(assetSourceText);
 if(asset.schema_version!==1||asset.kind!=="terrain"||!asset.terrain)throw new Error("Select a terrain asset.");
 const settings=terrainSettings(asset.terrain),heightPath=asset.terrain_heights;
 const heightSource=heightPath===undefined?undefined:await assetSource(heightPath,TERRAIN_DATA_MAX_BYTES);
 let heights:Float32Array|undefined;
 if(heightSource!==undefined){const field=parse(heightSource);if(field.schema_version!==1)throw new Error("Unsupported terrain heightfield version.");heights=terrainHeights(field.values,settings);}
 const paintPath=asset.terrain_paint,paintSource=paintPath===undefined?undefined:await assetSource(paintPath,TERRAIN_DATA_MAX_BYTES);
 const paint=paintSource===undefined?undefined:terrainPaint(parse(paintSource),settings.resolution),inputs=await terrainPaintInputs(project,paint);
 const revision=terrainSceneRevision(JSON.stringify([assetPath,assetSourceText,heightPath,heightSource,paintPath,paintSource,inputs]));
 return {inputs,paint,paintPath:paintPath as string|undefined,paintSource,assetPath,assetSource:assetSourceText,asset,settings,heights,heightPath:heightPath as string|undefined,heightSource,revision,name:typeof asset.name==="string"?asset.name:assetPath};
}

export async function prepareTerrainUpdate(project:ProjectSnapshot,input:{scenePath:unknown;instanceId:unknown;editRevision:unknown;settings:unknown;heights:unknown}){
 const current=await readTerrainEdit(project,input.scenePath,input.instanceId);
 if(input.editRevision!==current.revision)throw new Error("Terrain or scene changed externally. Reopen the terrain before saving.");
 const {changes:assetChanges,content,heightPath,heightContent}=terrainAssetUpdate(current,input.settings,input.heights);
 // Include the unchanged scene as a conflict guard in the same publication batch.
 const changes:FileChange[]=[...assetChanges,{path:current.scenePath,before:current.sceneSource,content:current.sceneSource}];
 const revision=terrainSceneRevision(JSON.stringify([current.scenePath,current.sceneSource,current.assetPath,content,heightPath,heightContent,current.paintPath,current.paintSource,current.inputs]));
 return {changes,result:{id:current.instanceId,path:current.assetPath,scenePath:current.scenePath,sceneRevision:current.sceneRevision,editRevision:revision}};
}

export function terrainAssetUpdate(current:Awaited<ReturnType<typeof readTerrainAsset>>,value:unknown,field:unknown){
 const settings=terrainSettings(value),heights=terrainHeights(field,settings),recipe=Object.fromEntries(Object.entries(settings).filter(([key])=>!key.startsWith("sun")));
 const heightPath=current.heightPath??current.assetPath.replace(/\.entity\.toml$/,`.${randomUUID()}.heights.toml`);
 const expected={...current.asset,terrain:recipe,terrain_heights:heightPath};
 const header=/^[ \t]*\[terrain\][ \t]*(?:#.*)?\r?$/m.exec(current.assetSource);
 if(!header)throw new Error("Terrain editing requires a [terrain] table. Convert this asset using TOML first.");
 const tail=current.assetSource.slice(header.index+header[0].length),next=tail.search(/^[ \t]*\[/m),end=next<0?current.assetSource.length:header.index+header[0].length+next;
 const comments=current.assetSource.slice(header.index,end).split("\n").filter(line=>line.trimStart().startsWith("#")).join("\n");
 let content=current.assetSource.slice(0,header.index)+(comments?comments+"\n":"")+stringify({terrain:recipe})+"\n"+current.assetSource.slice(end);
 if(!current.heightPath)content=`terrain_heights = ${JSON.stringify(heightPath)}\n`+content;
 if(Buffer.byteLength(content)>256*1024||!isDeepStrictEqual(parse(content),parse(stringify(expected))))throw new Error("Could not safely update terrain. Use the TOML editor.");
 const heightContent=stringify({schema_version:1,values:Array.from(heights)});
 return {content,heightPath,heightContent,changes:[{path:heightPath,before:current.heightSource??null,content:heightContent},{path:current.assetPath,before:current.assetSource,content}] as FileChange[]};
}

async function prepareTerrainBaseSave(project:ProjectSnapshot,input:{scenePath:unknown;sceneRevision:unknown;name:unknown;settings:unknown;collider:unknown;heights?:unknown;instanceId?:unknown;editRevision?:unknown;paint?:unknown}){
 if(input.instanceId!==undefined)return prepareTerrainUpdate(project,{...input,instanceId:input.instanceId,editRevision:input.editRevision,heights:input.heights});
 if(typeof input.scenePath!=="string"||!projectScenes(project).some(scene=>scene.path===input.scenePath))throw new Error("Choose a scene in the linked project.");
 if(typeof input.name!=="string"||!input.name.trim()||input.name.trim().length>64||/[\p{Cc}\p{Cf}]/u.test(input.name))throw new Error("Use a terrain name of 1–64 printable characters.");
 if(typeof input.collider!=="boolean")throw new Error("Choose whether to add a terrain collider.");
 const settings=terrainSettings(input.settings),recipe=Object.fromEntries(Object.entries(settings).filter(([key])=>!key.startsWith("sun")));
 const heights=input.heights===undefined?undefined:terrainHeights(input.heights,settings);
 const before=await readFile(await containedFile(project.root,input.scenePath,256*1024),"utf8");
 if(input.sceneRevision!==terrainSceneRevision(before))throw new Error("Scene changed since Terrain Builder opened. Reload before saving.");
 const scene=parse(before);
 if(scene.schema_version!==1||!Array.isArray(scene.entities)||scene.entities.length>=1024||scene.entities.some(entry=>!entry||typeof entry!=="object"||Array.isArray(entry)||!("id" in entry)||typeof entry.id!=="string"||!("asset" in entry)||typeof entry.asset!=="string"))throw new Error("Choose a schema-1 scene with referenced entities and fewer than 1024 instances.");
 const id=randomUUID(),assetId=randomUUID(),root=project.assetRoots[0];if(!root)throw new Error("Project has no asset root.");
 const assetPath=`${root}/terrains/${assetId}.entity.toml`;
 const heightPath=`${root}/terrains/${assetId}.heights.toml`;
 const source=stringify({schema_version:1,terrain_heights:heights?heightPath:undefined,id:assetId,name:input.name.trim(),kind:"terrain",terrain:recipe,material:{shading:"smooth",albedo:[0.65,0.65,0.65]},physics:{enabled:input.collider,body:"static",friction:0.5,restitution:0,collider:{shape:"terrain"}}});
 const headerEnd=before.search(/^\[/m),header=headerEnd<0?before:before.slice(0,headerEnd),rest=headerEnd<0?"":before.slice(headerEnd);
 const content=header.replace(/^entities\s*=\s*\[\s*\]\s*$/m,"")+rest+`\n[[entities]]\nid = ${JSON.stringify(id)}\nasset = ${JSON.stringify(assetPath)}\n`;
 parse(content);if(Buffer.byteLength(content)>256*1024)throw new Error("Updated scene exceeds 256 KiB.");
 const changes:FileChange[]=[...(heights?[{path:heightPath,before:null,content:stringify({schema_version:1,values:Array.from(heights)})}]:[]),{path:assetPath,before:null,content:source},{path:input.scenePath,before,content}];
 return {changes,result:{id,path:assetPath,scenePath:input.scenePath,sceneRevision:terrainSceneRevision(content),editRevision:undefined as string|undefined}};
}
export async function prepareTerrainSave(project:ProjectSnapshot,input:Parameters<typeof prepareTerrainBaseSave>[1]){
 const prepared=await prepareTerrainBaseSave(project,input);
 if(input.paint===undefined)return prepared;
 const assetChange=prepared.changes.find(change=>change.path===prepared.result.path)!;
 const asset=parse(assetChange.content),settings=terrainSettings(input.settings);
 let path=typeof asset.terrain_paint==="string"?asset.terrain_paint:undefined;
 if(input.paint!==null){
  const paint=terrainPaint(input.paint,settings.resolution);
  for(const reference of paint.materials){
   const item=await readMaterial(project,reference);
   if(!prepared.changes.some(change=>change.path===reference))prepared.changes.push({path:reference,before:item.source,content:item.source});
   for(const channel of materialChannels){const map=item.material[`${channel}_map`];if(map&&!prepared.changes.some(change=>change.path===map.asset)){const source=await readFile(await containedFile(project.root,map.asset,256*1024),"utf8");prepared.changes.push({path:map.asset,before:source,content:source});}}
  }
  path??=prepared.result.path.replace(/\.entity\.toml$/,`.${randomUUID()}.paint.toml`);
  const before=asset.terrain_paint===undefined?null:await readFile(await containedFile(project.root,path,TERRAIN_DATA_MAX_BYTES),"utf8");
  prepared.changes.push({path,before,content:stringify(paint)});
 }else path=undefined;
 const headerEnd=assetChange.content.search(/^\[/m),header=headerEnd<0?assetChange.content:assetChange.content.slice(0,headerEnd),tail=headerEnd<0?"":assetChange.content.slice(headerEnd);
 const content=header.replace(/^terrain_paint\s*=.*\r?\n/gm,"")+(path?`terrain_paint = ${JSON.stringify(path)}\n`:"")+tail;
 const expected={...asset};if(path)expected.terrain_paint=path;else delete expected.terrain_paint;
 if(!isDeepStrictEqual(parse(content),parse(stringify(expected))))throw new Error("Could not safely update terrain paint. Use the TOML editor.");
 assetChange.content=content;
 const scene=prepared.changes.find(change=>change.path===prepared.result.scenePath)!;
 const heightPath=asset.terrain_heights,heightSource=prepared.changes.find(change=>change.path===heightPath)?.content;
 const paintSource=path?prepared.changes.find(change=>change.path===path)?.content:undefined;
 const revision=terrainSceneRevision(JSON.stringify([prepared.result.scenePath,scene.content,prepared.result.path,content,heightPath,heightSource,path,paintSource,await terrainPaintInputs(project,input.paint===null?undefined:terrainPaint(input.paint,settings.resolution))]));
 return {...prepared,result:{...prepared.result,editRevision:revision}};
}
const state=globalThis as typeof globalThis&{emergentScenePlacementActive?:boolean};
export async function saveTerrain(project:ProjectSnapshot,input:Parameters<typeof prepareTerrainSave>[1]){
 if(state.emergentScenePlacementActive)throw new Error("Another scene edit is being saved.");state.emergentScenePlacementActive=true;
 try{
 const {changes,result}=await prepareTerrainSave(project,input);
 const scenes=input.instanceId===undefined?[result.scenePath]:projectScenes(project).map(scene=>scene.path);
 if(scenes.length>127)throw new Error("Terrain updates support validation of at most 127 scenes.");
 await checkCandidate(project,changes,{root:await foundationRoot(project)},scenes);
 const current=await readProjectBinding();if(current.id!==project.id||current.root!==project.root)throw new Error("Linked project changed during terrain validation. No terrain was saved.");
 await applyChanges(project,changes);const updated=await readTerrainEdit(project,result.scenePath,result.id);return {...result,editRevision:updated.revision,assetRevision:updated.assetRevision,assetName:updated.asset.name};
 }finally{state.emergentScenePlacementActive=false;}
}
