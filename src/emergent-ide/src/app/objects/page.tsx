import { ObjectCreator } from "@/components/object-creator";
import { readProjectSnapshot } from "@/lib/project";
import { objectJobs } from "@/lib/object-jobs";
import { publicObjectSettings } from "@/lib/object-settings";

export const instant = false;

export default async function ObjectsPage() {
  const project = await readProjectSnapshot();
  const [jobs, settings] = await Promise.all([objectJobs.list(project), publicObjectSettings()]);
  return <ObjectCreator projectId={project.id} projectName={project.name} initialJobs={jobs} settings={settings} />;
}
