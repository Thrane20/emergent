import { ProjectStudio } from "@/components/project-studio";
import { readPublicAISettings } from "@/lib/ai-settings";
import { readProjectSnapshot } from "@/lib/project";

// Project manifests and asset references are intentionally read from the
// selected local game project when the IDE route is requested.
export const instant = false;

export default async function Home({searchParams}:{searchParams:Promise<Record<string,string|string[]|undefined>>}) {
  const [project, aiSettings] = await Promise.all([
    readProjectSnapshot(),
    readPublicAISettings(),
  ]);
  const params=await searchParams;
  const scene=typeof params.scene==="string"&&project.files.some(file=>file.path===params.scene&&file.kind==="scene")?params.scene:project.entryScene;
  return <ProjectStudio initialView={params.view==="source"?"source":"scene"} key={`${params.view==="source"?"source":"scene"}:${scene}:${typeof params.instance==="string"?params.instance:""}:${typeof params.terrainRevision==="string"?params.terrainRevision:""}`} project={project} aiSettings={aiSettings} initialScene={scene} initialInstance={typeof params.instance==="string"?params.instance:undefined} />;
}
