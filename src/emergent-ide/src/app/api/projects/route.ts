import { sameOrigin } from "@/lib/object-http";
import { ObjectCreatorError } from "@/lib/object-types";
import { NoActiveProjectError } from "@/lib/workspace";
import { createProject, duplicateProject, openProject, ProjectWorkspaceError, saveProject, workspaceState } from "@/lib/project-workspace";

const noStore = (value: unknown, status = 200) => Response.json(value, { status, headers: { "Cache-Control": "no-store" } });

export async function GET() {
  try { return noStore(await workspaceState()); } catch (error) { return failure(error); }
}

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json() as Record<string, unknown>;
    switch (body.action) {
      case "open": return noStore({ project: await openProject(body.root) });
      case "new": return noStore(await createProject(body), 201);
      case "duplicate": return noStore(await duplicateProject(body), 201);
      case "save": return noStore(await saveProject(body.projectId, body.message));
      default: throw new ProjectWorkspaceError("Unknown project action.");
    }
  } catch (error) { return failure(error); }
}

function failure(error: unknown) {
  return noStore({ error: error instanceof Error ? error.message : "Project action failed." }, error instanceof ProjectWorkspaceError || error instanceof ObjectCreatorError || error instanceof NoActiveProjectError ? error.status : 500);
}
