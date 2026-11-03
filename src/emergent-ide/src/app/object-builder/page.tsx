import {structureThumbnail} from "@/lib/structure-thumbnail";
import {ObjectBuilder} from "@/components/object-builder";
import {readProjectForPage} from "@/lib/project-page";
import {projectScenes} from "@/lib/project-scenes";
import {materialLibrary} from "@/lib/material-assets";
import {readStructureAsset,structureLibrary} from "@/lib/structure-assets";
import {modelPartsLibrary} from "@/lib/model-parts";
export const instant=false;
export default async function ObjectBuilderPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
 const project=await readProjectForPage(),params=await searchParams;
 const scenes=projectScenes(project);
 const initial=typeof params.asset==="string"?await readStructureAsset(project,params.asset):undefined;
 const structures=[...await Promise.all(structureLibrary(project).map(async item=>{try{return {...item,thumbnail:structureThumbnail((await readStructureAsset(project,item.path)).document)};}catch{return item;}})),...modelPartsLibrary(project)];
 return <ObjectBuilder key={`${project.id}:${String(params.asset??"")}`} projectId={project.id} projectName={project.name} initial={initial?{document:initial.document,material:initial.material,assetPath:initial.assetPath,revision:initial.revision}:undefined} structures={structures} materials={await materialLibrary(project)} scenes={scenes} initialInstance={typeof params.instance==="string"?params.instance:undefined} initialScene={scenes.find(scene=>scene.path===params.scene)?.path??project.entryScene}/>;
}
