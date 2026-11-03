import { redirect } from "next/navigation";
import { readProjectSnapshot } from "./project";
import { workspaceState } from "./project-workspace";

/** Project pages share the chooser; API calls keep a structured no-project error. */
export async function readProjectForPage() {
  if (!(await workspaceState()).active) redirect("/projects");
  return readProjectSnapshot();
}
