import { objectProject, sameOrigin } from "@/lib/object-http";
import { evaluateStatement } from "@/lib/statement-ai";

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json();
    if (typeof body.subject !== "string" || typeof body.id !== "string") throw new Error("Statement subject and ID are required.");
    const project = await objectProject(body.projectId);
    return Response.json({ statements: await evaluateStatement(project, body.subject, body.id) });
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Unable to check statement." }, { status: 400 });
  }
}
