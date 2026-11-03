import {AudioEditor} from "@/components/audio-editor";
import {listClips} from "@/lib/audio-assets";
import {readProjectForPage} from "@/lib/project-page";
export const instant=false;
export default async function AudioPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
  const [project,params]=await Promise.all([readProjectForPage(),searchParams]);
  const clips=listClips(project),scenes=project.files.filter(file=>file.path.endsWith(".scene.toml")).map(file=>file.path).sort();
  const asset=typeof params.asset==="string"&&clips.some(clip=>clip.path===params.asset)?params.asset:clips[0]?.path;
  const scenePath=typeof params.scene==="string"&&scenes.includes(params.scene)?params.scene:project.entryScene;
  return <AudioEditor projectId={project.id} projectName={project.name} scenePath={scenePath} scenes={scenes} initialClips={clips} initialPath={asset}/>;
}
