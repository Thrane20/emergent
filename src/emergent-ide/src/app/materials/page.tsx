import {MaterialGenerator} from "@/components/material-generator";
import {readProjectSnapshot} from "@/lib/project";
import {materialLibrary} from "@/lib/material-assets";
import {materialSettings,isTexturePath} from "@/lib/material-settings";
export const instant=false;
export default async function MaterialsPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
 const project=await readProjectSnapshot(),params=await searchParams,items=await materialLibrary(project);
 let source;try{if(typeof params.source==="string"){const value=materialSettings(JSON.parse(params.source));if(!value.asset&&!value.terrain_paint)source=value;}}catch{}
 return <MaterialGenerator key={`${project.id}:${String(params.asset??"")}:${String(params.source??"")}`} projectId={project.id} projectName={project.name} initialItems={items} selectedPath={typeof params.asset==="string"?params.asset:undefined} source={source} noiseAssets={project.files.filter(file=>file.path.endsWith(".noise.toml"))} textureAssets={project.files.filter(file=>isTexturePath(file.path))}/>;
}
