import "server-only";
import {readTerrainAsset} from "./terrain-assets";
import {readMaterial} from "./material-assets";
import {runTerrainTool} from "./terrain-tool";
import {terrainThumbnailSvg} from "./terrain-thumbnail";
import type {ProjectSnapshot} from "./project";
const cache=new Map<string,Promise<string>>(),active=new Map<string,Promise<string>>();
let tail:Promise<unknown>=Promise.resolve();
export async function terrainThumbnail(project:ProjectSnapshot,assetPath:unknown):Promise<string>{
 const key=JSON.stringify([project.id,project.root,assetPath]);if(active.has(key))return active.get(key)!;
 if(active.size>=64)throw new Error("Terrain preview queue is busy.");
 const task=tail.catch(()=>{}).then(()=>render(project,assetPath));tail=task;active.set(key,task);
 try{return await task;}finally{active.delete(key);}
}
async function render(project:ProjectSnapshot,assetPath:unknown){
 const asset=await readTerrainAsset(project,assetPath),key=JSON.stringify([project.id,project.root,asset.assetPath,asset.revision]);if(cache.has(key))return cache.get(key)!;
 const task=(async()=>{const heights=asset.heights??(await runTerrainTool(project,asset.settings,false))[0].values,colors=await Promise.all((asset.paint?.materials??[]).map(async path=>(await readMaterial(project,path)).material.albedo??[.65,.65,.65]));const material=asset.asset.material as {albedo?:number[]}|undefined;
 if((await readTerrainAsset(project,asset.assetPath)).revision!==asset.revision)throw new Error("Terrain changed while preparing its preview.");return terrainThumbnailSvg(asset.settings,heights,asset.paint,colors,material?.albedo);})();
 if(cache.size>=64)cache.delete(cache.keys().next().value!);cache.set(key,task);try{return await task;}catch(error){cache.delete(key);throw error;}
}
