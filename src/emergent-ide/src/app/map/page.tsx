import {SceneMapEditor} from "@/components/scene-map-editor";
import {readProjectForPage} from "@/lib/project-page";
export const instant=false;
export default async function SceneMapPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
  const [project,params]=await Promise.all([readProjectForPage(),searchParams]);
  const scenes=project.files.filter(file=>file.path.endsWith(".scene.toml")).map(file=>file.path).sort();
  const scene=typeof params.scene==="string"&&scenes.includes(params.scene)?params.scene:project.entryScene;
  return <SceneMapEditor projectId={project.id} projectName={project.name} scenes={scenes} initialScene={scene}/>;
}
