import {BrainEditor} from "@/components/brain-editor";
import {readPublicAISettings} from "@/lib/ai-settings";
import {listBrains,readBrain} from "@/lib/brain-assets";
import {readProjectSnapshot} from "@/lib/project";
export const instant=false;
export default async function BrainsPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
  const [project,aiSettings,params]=await Promise.all([readProjectSnapshot(),readPublicAISettings(),searchParams]);
  const brains=listBrains(project),asset=typeof params.asset==="string"?params.asset:brains.find(brain=>!brain.error)?.path;
  let initial;try{initial=asset?readBrain(project,asset):undefined;}catch{initial=undefined;}
  const scenePath=typeof params.scene==="string"&&project.files.some(file=>file.path===params.scene&&file.path.endsWith(".scene.toml"))?params.scene:project.entryScene;
  return <BrainEditor key={initial?.path??"new"} projectId={project.id} projectName={project.name} scenePath={scenePath} aiSettings={aiSettings} initialBrains={brains} initial={initial}/>;
}
