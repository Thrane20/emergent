import { objectProject, sameOrigin } from "@/lib/object-http";
import { implementStatement } from "@/lib/statement-implementation";

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json();
    if (typeof body.subject !== "string" || typeof body.id !== "string") throw new Error("Statement subject and ID are required.");
    return Response.json(await implementStatement(await objectProject(body.projectId), body.subject, body.id));
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Unable to implement statement." }, { status: 400 });
  }
}
