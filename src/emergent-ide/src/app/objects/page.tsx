import { ObjectCreator } from "@/components/object-creator";
import { projectImages } from "@/lib/image-library";
import { readProjectForPage } from "@/lib/project-page";
import { objectJobs } from "@/lib/object-jobs";
import { publicObjectSettings } from "@/lib/object-settings";

export const instant = false;

export default async function ObjectsPage() {
  const project = await readProjectForPage();
  const [jobs, settings] = await Promise.all([objectJobs.list(project), publicObjectSettings()]);
  // Sprites and skyboxes are not useful references for a 3D object.
  const images = projectImages(project).filter(image => (image.kind ?? "image") === "image");
  return <ObjectCreator projectId={project.id} projectName={project.name} initialJobs={jobs} settings={settings} images={images} />;
}
