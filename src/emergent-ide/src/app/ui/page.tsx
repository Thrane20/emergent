import {UiDesigner} from "@/components/ui-designer";
import {readPublicAISettings} from "@/lib/ai-settings";
import {readProjectForPage} from "@/lib/project-page";
import {listUis,readUi} from "@/lib/ui-assets";
export const instant=false;
export default async function UiPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
  const [project,aiSettings,params]=await Promise.all([readProjectForPage(),readPublicAISettings(),searchParams]);
  const uis=listUis(project),asset=typeof params.asset==="string"?params.asset:uis.find(ui=>!ui.error)?.path;
  let initial;try{initial=asset?readUi(project,asset):undefined;}catch{initial=undefined;}
  const scenePath=typeof params.scene==="string"&&project.files.some(file=>file.path===params.scene&&file.path.endsWith(".scene.toml"))?params.scene:project.entryScene;
  return <UiDesigner key={initial?.path??"new"} projectId={project.id} projectName={project.name} scenePath={scenePath} aiSettings={aiSettings} initialUis={uis} initial={initial}/>;
}
