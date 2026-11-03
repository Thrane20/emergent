import {LocomotionEditor} from "@/components/locomotion-editor";
import {readPublicAISettings} from "@/lib/ai-settings";
import {listProfiles,readProfile} from "@/lib/locomotion-assets";
import {readProjectForPage} from "@/lib/project-page";
export const instant=false;
export default async function LocomotionPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
  const [project,aiSettings,params]=await Promise.all([readProjectForPage(),readPublicAISettings(),searchParams]);
  const profiles=listProfiles(project),asset=typeof params.asset==="string"?params.asset:profiles.find(profile=>!profile.error)?.path;
  let initial;try{initial=asset?readProfile(project,asset):undefined;}catch{initial=undefined;}
  const scenePath=typeof params.scene==="string"&&project.files.some(file=>file.path===params.scene&&file.path.endsWith(".scene.toml"))?params.scene:project.entryScene;
  return <LocomotionEditor key={initial?.path??"new"} projectId={project.id} projectName={project.name} scenePath={scenePath} aiSettings={aiSettings} initialProfiles={profiles} initial={initial}/>;
}
