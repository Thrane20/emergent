import {ImageGenerator} from "@/components/image-generator";
import {projectImages} from "@/lib/image-library";
import {publicObjectSettings} from "@/lib/object-settings";
import {readProjectSnapshot} from "@/lib/project";
import {textureJobs} from "@/lib/texture-jobs";
import "./images.css";

export const instant=false;

export default async function ImagesPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
 const params=await searchParams,project=await readProjectSnapshot();
 const [jobs,settings]=await Promise.all([textureJobs.list(project),publicObjectSettings()]);
 return <ImageGenerator key={`${project.id}:${project.root}:${String(params.kind??"image")}`} projectId={project.id} projectName={project.name} initialImages={projectImages(project)} initialJobs={jobs} initialKind={params.kind==="skybox"?"skybox":params.kind==="sprite"?"sprite":"image"} configured={settings.configured}/>;
}
