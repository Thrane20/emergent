import { readProjectSnapshot } from "@/lib/project";
import {
  ProjectRunError,
  projectRunManager,
} from "@/lib/project-runner";

export async function GET() {
  return noStore(projectRunManager.status());
}

export async function POST(request: Request) {
  try {
    const project = await matchingProject(request);
    return noStore(await projectRunManager.start(project), 202);
  } catch (error) {
    return runError(error);
  }
}

export async function DELETE(request: Request) {
  try {
    const project = await matchingProject(request);
    return noStore(projectRunManager.stop(project.id));
  } catch (error) {
    return runError(error);
  }
}

async function matchingProject(request: Request) {
  const body = await request.json() as { projectId?: unknown };
  const project = await readProjectSnapshot();
  if (body.projectId !== project.id) {
    throw new ProjectRunError("The run request does not match the linked project.", 409);
  }
  return project;
}

function noStore(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "Cache-Control": "no-store" },
  });
}

function runError(error: unknown) {
  const status = error instanceof ProjectRunError ? error.status : 500;
  const message = error instanceof Error ? error.message : "Project run request failed.";
  return noStore({ error: message }, status);
}
