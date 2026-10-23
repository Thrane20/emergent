import { addStatement, removeStatement, statementsFor } from "@/lib/statements";
import { readProjectSnapshot } from "@/lib/project";

export async function GET(request: Request) {
  try {
    const project = await readProjectSnapshot();
    const url = new URL(request.url);
    return Response.json({ statements: await statementsFor(project, url.searchParams.get("subject") ?? "") });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function POST(request: Request) {
  try {
    const body = await request.json() as { projectId?: unknown; subject?: unknown; text?: unknown };
    const project = await matchingProject(body.projectId);
    const subject = requiredString(body.subject, "Subject");
    const text = requiredString(body.text, "Statement");
    return Response.json({ statements: await addStatement(project, subject, text) });
  } catch (error) {
    return errorResponse(error);
  }
}

export async function DELETE(request: Request) {
  try {
    const body = await request.json() as { projectId?: unknown; subject?: unknown; id?: unknown };
    const project = await matchingProject(body.projectId);
    const subject = requiredString(body.subject, "Subject");
    const id = requiredString(body.id, "Statement ID");
    return Response.json({ statements: await removeStatement(project, subject, id) });
  } catch (error) {
    return errorResponse(error);
  }
}

async function matchingProject(projectId: unknown) {
  const project = await readProjectSnapshot();
  if (projectId !== project.id) throw new Error("The request does not match the linked project.");
  return project;
}

function requiredString(value: unknown, label: string): string {
  if (typeof value !== "string" || !value.trim()) throw new Error(`${label} is required.`);
  return value.trim();
}

function errorResponse(error: unknown) {
  return Response.json({ error: error instanceof Error ? error.message : "Statement request failed." }, { status: 400 });
}
