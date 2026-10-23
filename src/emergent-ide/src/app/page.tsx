import { ProjectStudio } from "@/components/project-studio";
import { readPublicAISettings } from "@/lib/ai-settings";
import { readProjectSnapshot } from "@/lib/project";

// Project manifests and asset references are intentionally read from the
// selected local game project when the IDE route is requested.
export const instant = false;

export default async function Home() {
  const [project, aiSettings] = await Promise.all([
    readProjectSnapshot(),
    readPublicAISettings(),
  ]);
  return <ProjectStudio project={project} aiSettings={aiSettings} />;
}
