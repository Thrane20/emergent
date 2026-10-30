import {createHash} from "node:crypto";
import {ObjectBuilder} from "@/components/object-builder";
import {readProjectSnapshot} from "@/lib/project";
import {projectScenes} from "@/lib/project-scenes";
import {materialLibrary} from "@/lib/material-assets";
import {readStructureAsset,structureLibrary} from "@/lib/structure-assets";
export const instant=false;
export default async function ObjectBuilderPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
 const project=await readProjectSnapshot(),params=await searchParams;
 const scenes=projectScenes(project).map(scene=>({...scene,revision:createHash("sha256").update(project.files.find(file=>file.path===scene.path)?.content??"").digest("hex")}));
 const initial=typeof params.asset==="string"?await readStructureAsset(project,params.asset):undefined;
 return <ObjectBuilder key={`${project.id}:${String(params.asset??"")}`} projectId={project.id} projectName={project.name} initial={initial?{document:initial.document,material:initial.material,assetPath:initial.assetPath,revision:initial.revision}:undefined} structures={structureLibrary(project)} materials={await materialLibrary(project)} scenes={scenes} initialInstance={typeof params.instance==="string"?params.instance:undefined} initialScene={scenes.find(scene=>scene.path===params.scene)?.path??project.entryScene}/>;
}
