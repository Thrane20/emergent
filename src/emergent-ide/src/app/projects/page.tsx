import { redirect } from "next/navigation";
import { ProjectSwitcher } from "@/components/project-switcher";
import { workspaceState } from "@/lib/project-workspace";

export const instant = false;

export default async function ProjectsPage() {
  const state = await workspaceState();
  if (state.active) redirect("/");
  return <main className="object-creator">
    <header className="object-heading"><div><span className="eyebrow">EMERGENT</span><h1>Open or create a project</h1><p>Choose a project folder, reopen a recent project, or create a new game from the standard template.</p></div></header>
    <ProjectSwitcher projectId="" projectName="Open or create a project" />
  </main>;
}
