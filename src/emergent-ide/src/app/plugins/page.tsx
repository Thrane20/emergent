import { PluginManager } from "@/components/plugin-manager";
import { readProjectForPage } from "@/lib/project-page";
import { readProjectPlugins } from "@/lib/project-plugins";

export const instant = false;

export default async function PluginsPage() {
  const project = await readProjectForPage();
  return <PluginManager projectName={project.name} initialPlugins={await readProjectPlugins(project.root)} />;
}
