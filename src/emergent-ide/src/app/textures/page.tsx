import {TextureCreator} from "@/components/texture-creator";
import {readProjectSnapshot} from "@/lib/project";
import {textureJobs} from "@/lib/texture-jobs";
import {publicObjectSettings} from "@/lib/object-settings";
export const instant=false;
export default async function TexturesPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){const project=await readProjectSnapshot(),[jobs,settings]=await Promise.all([textureJobs.list(project),publicObjectSettings()]);const params=await searchParams,scenePath=typeof params.scene==="string"&&project.files.some(file=>file.path===params.scene&&file.kind==="scene")?params.scene:undefined;return <TextureCreator scenePath={scenePath} projectId={project.id} projectName={project.name} initialJobs={jobs} configured={settings.configured}/>;}
