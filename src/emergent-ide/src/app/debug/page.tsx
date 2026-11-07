import { DebugEditor } from "@/components/debug-editor";
import { readProjectForPage } from "@/lib/project-page";
import { projectScenes } from "@/lib/project-scenes";
export const instant = false;
export default async function DebugPage({ searchParams }: { searchParams: Promise<Record<string, string | string[] | undefined>> }) {
  const [project, params] = await Promise.all([readProjectForPage(), searchParams]);
  const scenes = projectScenes(project);
  const scene = scenes.find(item => item.path === params.scene)?.path ?? project.entryScene;
  return <DebugEditor key={`${project.id}:${scene}`} projectId={project.id} projectName={project.name} scenePath={scene} scenes={scenes.map(item => ({ path: item.path, name: item.name }))} />;
}
