import {SceneMapEditor} from "@/components/scene-map-editor";
import {readProjectForPage} from "@/lib/project-page";
export const instant=false;
export default async function SceneMapPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
  const [project,params]=await Promise.all([readProjectForPage(),searchParams]);
  const scenes=project.files.filter(file=>file.path.endsWith(".scene.toml")).map(file=>file.path).sort();
  const prefabs=project.files.filter(file=>file.path.endsWith(".prefab.toml")).map(file=>({path:file.path,name:file.displayName??file.path.replace(/^.*\//,"").replace(/\.prefab\.toml$/,"")})).sort((a,b)=>a.name.localeCompare(b.name));
  const scene=typeof params.scene==="string"&&[...scenes,...prefabs.map(prefab=>prefab.path)].includes(params.scene)?params.scene:project.entryScene;
  return <SceneMapEditor projectId={project.id} projectName={project.name} scenes={scenes} prefabs={prefabs} initialScene={scene}/>;
}
