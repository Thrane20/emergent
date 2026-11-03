import {ModelPartsEditor} from "@/components/model-parts-editor";
import {generatedModels} from "@/lib/model-parts";
import {readProjectForPage} from "@/lib/project-page";
import {projectScenes} from "@/lib/project-scenes";
import {publicObjectSettings} from "@/lib/object-settings";
export const instant=false;
export default async function ModelsPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
 const project=await readProjectForPage(),params=await searchParams,models=generatedModels(project),settings=await publicObjectSettings();
 return <ModelPartsEditor key={`${project.id}:${String(params.model??"")}`} projectId={project.id} projectName={project.name} models={models} scenes={projectScenes(project)} configured={settings.configured} initialModel={models.find(model=>model.descriptor===params.model)?.descriptor??models[0]?.descriptor}/>;
}
