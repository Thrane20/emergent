import "server-only";
import {randomUUID} from "node:crypto";
import {parse,stringify} from "smol-toml";
import {isDeepStrictEqual} from "node:util";
import {readProjectBinding,type ProjectSnapshot} from "./project";
import {projectScenes} from "./project-scenes";
import {terrainSettings} from "./terrain";
import {terrainHeights} from "./terrain-brush";
import {terrainPaint} from "./terrain-paint";
import {readTerrainAsset,terrainAssetUpdate,terrainPaintInputs,terrainSceneRevision} from "./terrain-assets";
import {prepareSceneAssetPlacement} from "./scene-asset-placement";
import {applyChanges,checkCandidate,type FileChange} from "./statement-files";
import {foundationRoot} from "./project-code-context";
import type {TerrainLibraryItem} from "./terrain-library-types";
export function terrainLibrary(project:ProjectSnapshot):TerrainLibraryItem[]{const version=terrainSceneRevision(JSON.stringify(project.files.map(file=>[file.path,file.modifiedAt,file.bytes])));return project.files.filter(file=>file.path.endsWith(".entity.toml")&&project.assetRoots.some(root=>file.path.startsWith(`${root}/`))&&file.content&&/^kind\s*=\s*["']terrain["']/m.test(file.content)).sort((a,b)=>b.modifiedAt.localeCompare(a.modifiedAt)).map(file=>({path:file.path,name:file.displayName??file.name.replace(/\.entity\.toml$/,""),version}));}
export async function prepareTerrainLibrarySave(project:ProjectSnapshot,input:{assetPath?:unknown;revision?:unknown;name:unknown;settings:unknown;heights:unknown;paint:unknown;collider:unknown}){
 const settings=terrainSettings(input.settings),heights=terrainHeights(input.heights,settings);
 if(typeof input.name!=="string"||!input.name.trim()||input.name.trim().length>64||/[\p{Cc}\p{Cf}]/u.test(input.name))throw new Error("Use a terrain name of 1–64 printable characters.");
 if(typeof input.collider!=="boolean")throw new Error("Choose whether to use a terrain collider.");
 if(input.assetPath!==undefined&&typeof input.assetPath!=="string")throw new Error("Choose a terrain asset.");
 const current=input.assetPath===undefined?undefined:await readTerrainAsset(project,input.assetPath);
 if(current&&current.revision!==input.revision)throw new Error("Terrain changed externally. Reopen it before saving.");
 if(current&&current.name!==input.name.trim())throw new Error("Save a copy to use a different terrain name.");
 const root=project.assetRoots[0];if(!root)throw new Error("Project has no asset root.");
 const id=randomUUID(),assetPath=current?.assetPath??`${root}/terrains/${id}.entity.toml`;
 let changes:FileChange[];
 if(current)changes=terrainAssetUpdate(current,settings,input.heights).changes;
 else{const heightPath=`${root}/terrains/${id}.heights.toml`;changes=[{path:heightPath,before:null,content:stringify({schema_version:1,values:Array.from(heights)})},{path:assetPath,before:null,content:stringify({schema_version:1,id,name:input.name.trim(),kind:"terrain",terrain_heights:heightPath,terrain:Object.fromEntries(Object.entries(settings).filter(([key])=>!key.startsWith("sun"))),material:{shading:"smooth",albedo:[.65,.65,.65]},physics:{enabled:input.collider,body:"static",friction:.5,restitution:0,collider:{shape:"terrain"}}})}];}
 const paint=input.paint===null?undefined:terrainPaint(input.paint,settings.resolution),inputs=await terrainPaintInputs(project,paint),assetChange=changes.find(change=>change.path===assetPath)!,asset=parse(assetChange.content);
 let paintPath:string|undefined;
 if(paint){paintPath=current?.paintPath??assetPath.replace(/\.entity\.toml$/,`.${randomUUID()}.paint.toml`);changes.push({path:paintPath,before:current?.paintSource??null,content:stringify(paint)});}
 const first=assetChange.content.search(/^\[/m),header=first<0?assetChange.content:assetChange.content.slice(0,first),tail=first<0?"":assetChange.content.slice(first);
 assetChange.content=header.replace(/^terrain_paint\s*=.*\r?\n/gm,"")+(paintPath?`terrain_paint = ${JSON.stringify(paintPath)}\n`:"")+tail;
 if(paintPath)asset.terrain_paint=paintPath;else delete asset.terrain_paint;
 if(!isDeepStrictEqual(parse(assetChange.content),parse(stringify(asset))))throw new Error("Could not safely update terrain paint.");
 return {changes,assetPath,name:input.name.trim(),paint,inputs,current};
}
const state=globalThis as typeof globalThis&{emergentScenePlacementActive?:boolean};
export async function saveTerrainLibrary(project:ProjectSnapshot,input:Parameters<typeof prepareTerrainLibrarySave>[1]){
 if(state.emergentScenePlacementActive)throw new Error("Another scene edit is saving.");state.emergentScenePlacementActive=true;
 try{const prepared=await prepareTerrainLibrarySave(project,input),id=randomUUID(),probe=`${project.assetRoots[0]}/terrains/${id}.scene.toml`,scenes=projectScenes(project);
 if(scenes.length>126)throw new Error("Terrain saving supports at most 126 scenes.");
 await checkCandidate(project,[...prepared.changes,{path:probe,before:null,content:stringify({schema_version:1,id,entities:[{id,asset:prepared.assetPath}]})}],{root:await foundationRoot(project)},[probe,...scenes.map(scene=>scene.path)]);
 if(JSON.stringify(await terrainPaintInputs(project,prepared.paint))!==JSON.stringify(prepared.inputs))throw new Error("A terrain material changed during validation. Reload before saving.");
 if(prepared.current&&(await readTerrainAsset(project,prepared.assetPath)).revision!==prepared.current.revision)throw new Error("Terrain changed during validation. Reopen before saving.");
 const binding=await readProjectBinding();if(binding.id!==project.id||binding.root!==project.root)throw new Error("Linked project changed. No terrain saved.");
 await applyChanges(project,prepared.changes);const asset=await readTerrainAsset(project,prepared.assetPath);return {assetPath:asset.assetPath,revision:asset.revision,name:asset.name};
 }finally{state.emergentScenePlacementActive=false;}
}
export async function prepareTerrainPlacement(project:ProjectSnapshot,input:{assetPath:unknown;revision?:unknown;scenePath:unknown;sceneRevision:unknown}){
 const asset=await readTerrainAsset(project,input.assetPath);if(input.revision!==undefined&&input.revision!==asset.revision)throw new Error("Terrain changed. Reload the library before placing it.");
 const placement=await prepareSceneAssetPlacement(project,{...input,assetPath:asset.assetPath});return {...placement,asset};
}
export async function placeTerrain(project:ProjectSnapshot,input:Parameters<typeof prepareTerrainPlacement>[1]){
 if(state.emergentScenePlacementActive)throw new Error("Another scene edit is saving.");state.emergentScenePlacementActive=true;
 try{const {asset,change,instanceId}=await prepareTerrainPlacement(project,input);await checkCandidate(project,[change],{root:await foundationRoot(project)},[change.path]);
 if((await readTerrainAsset(project,asset.assetPath)).revision!==asset.revision)throw new Error("Terrain changed during validation. Reload the library before placing it.");
 const binding=await readProjectBinding();if(binding.id!==project.id||binding.root!==project.root)throw new Error("Linked project changed. Nothing placed.");await applyChanges(project,[change]);return {id:instanceId,scenePath:change.path,sceneRevision:terrainSceneRevision(change.content),assetPath:asset.assetPath,name:asset.name};
 }finally{state.emergentScenePlacementActive=false;}
}
