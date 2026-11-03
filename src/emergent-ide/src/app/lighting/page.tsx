import {LightingEditor} from "@/components/lighting-editor";
import {readProjectForPage} from "@/lib/project-page";
import {projectScenes} from "@/lib/project-scenes";
import {projectImages} from "@/lib/image-library";
import {readSceneLighting} from "@/lib/scene-lighting";
import "./lighting.css";
export const instant=false;
export default async function LightingPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
 const project=await readProjectForPage(),params=await searchParams,scenes=projectScenes(project),path=typeof params.scene==="string"&&scenes.some(scene=>scene.path===params.scene)?params.scene:project.entryScene;
 const initial=await readSceneLighting(project,path);
 return <LightingEditor key={`${project.id}:${path}`} projectId={project.id} projectName={project.name} scenes={scenes} initial={{path,settings:initial.settings,revision:initial.revision}} skyboxes={projectImages(project).filter(image=>image.kind==="skybox")}/>;
}
