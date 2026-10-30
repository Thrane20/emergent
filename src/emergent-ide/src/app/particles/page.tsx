import {ParticleEditor} from "@/components/particle-editor";
import {readProjectSnapshot} from "@/lib/project";
import {projectScenes} from "@/lib/project-scenes";
import {projectImages} from "@/lib/image-library";
import {particleLibrary,readParticleAsset,particleSceneRevision} from "@/lib/particle-assets";
export const instant=false;
export default async function ParticlesPage({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}){
 const project=await readProjectSnapshot(),params=await searchParams,scenes=await Promise.all(projectScenes(project).map(async s=>({...s,revision:await particleSceneRevision(project,s.path)}))),asset=typeof params.asset==="string"?await readParticleAsset(project,params.asset):undefined;
 return <ParticleEditor key={`${project.id}:${String(params.asset??"")}`} projectId={project.id} projectName={project.name} library={particleLibrary(project)} images={projectImages(project)} scenes={scenes} initialScene={scenes.find(s=>s.path===params.scene)?.path??project.entryScene} initial={asset?{assetPath:asset.assetPath,effectPath:asset.effectPath,revision:asset.revision,definition:asset.definition}:undefined}/>;
}
