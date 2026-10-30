import {terrainLibrary} from "@/lib/terrain-library";
import {materialLibrary} from "@/lib/material-assets";
import { TerrainBuilder } from "@/components/terrain-builder";
import {createHash} from "node:crypto";
import {projectScenes} from "@/lib/project-scenes";
import { readProjectSnapshot } from "@/lib/project";
import {parse} from "smol-toml";
import {RaymarchedTerrainBuilder} from "@/components/raymarched-terrain-builder";
import {raymarchedLibrary} from "@/lib/raymarched-assets";
import {isTerrainBiome} from "@/lib/terrain-biomes";

export const instant = false;
export default async function TerrainPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
 const project=await readProjectSnapshot(),params=await searchParams;
 const scenes=projectScenes(project).map(scene=>({...scene,revision:createHash("sha256").update(project.files.find(file=>file.path===scene.path)?.content??"").digest("hex")}));
 const selected=scenes.find(scene=>scene.path===params.scene)?.path??project.entryScene;
 const instance=typeof params.instance==="string"?params.instance:undefined;
 const sceneFile=project.files.find(file=>file.path===selected)?.content;
 const entry=instance&&sceneFile?(parse(sceneFile).entities as {id:string;asset:string}[]).find(item=>item.id===instance):undefined;
 const assetFile=entry?project.files.find(file=>file.path===entry.asset)?.content:undefined;
 const raymarched=assetFile&&parse(assetFile).kind==="raymarched-terrain";
 const biome=isTerrainBiome(params.biome)?params.biome:undefined;
 if(params.method==="raymarch"||params.variant==="raymarch"||raymarched)return <RaymarchedTerrainBuilder key={`${project.id}:${selected}:${entry?.asset??"new"}:${params.open??""}:${biome??""}`} projectId={project.id} projectName={project.name} scenes={scenes} initialScene={selected} initialAsset={raymarched?entry?.asset:undefined} initialBiome={biome} initialLibrary={raymarchedLibrary(project)}/>;
 return <TerrainBuilder key={`${project.id}:${selected}:${instance??"new"}:${typeof params.open==="string"?params.open:""}:${biome??""}`} initialBiome={biome} initialLibrary={terrainLibrary(project)} initialMaterials={await materialLibrary(project)} initialInstance={instance} projectId={project.id} projectName={project.name} scenes={scenes} initialScene={selected}/>;
}
