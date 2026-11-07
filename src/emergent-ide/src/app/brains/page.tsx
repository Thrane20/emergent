import {BrainEditor} from "@/components/brain-editor";
import {readPublicAISettings} from "@/lib/ai-settings";
import {listBrains,readBrain} from "@/lib/brain-assets";
import {readProjectForPage} from "@/lib/project-page";
import {readStructureAsset,structureLibrary} from "@/lib/structure-assets";
import {structureThumbnail} from "@/lib/structure-thumbnail";
export const instant=false;
export default async function BrainsPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
  const [project,aiSettings,params]=await Promise.all([readProjectForPage(),readPublicAISettings(),searchParams]);
  const brains=listBrains(project),asset=typeof params.asset==="string"?params.asset:brains.find(brain=>!brain.error)?.path;
  let initial;try{initial=asset?readBrain(project,asset):undefined;}catch{initial=undefined;}
  const scenePath=typeof params.scene==="string"&&project.files.some(file=>file.path===params.scene&&file.path.endsWith(".scene.toml"))?params.scene:project.entryScene;
  const objects=await Promise.all(structureLibrary(project).map(async item=>{try{return {...item,thumbnail:structureThumbnail((await readStructureAsset(project,item.path)).document)};}catch{return item;}}));
  return <BrainEditor objects={objects} key={initial?.path??"new"} projectId={project.id} projectName={project.name} scenePath={scenePath} aiSettings={aiSettings} initialBrains={brains} initial={initial}/>;
}
