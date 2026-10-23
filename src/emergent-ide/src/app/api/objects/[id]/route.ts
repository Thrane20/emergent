import { objectError, objectProject, sameOrigin } from "@/lib/object-http";
import { objectJobs } from "@/lib/object-jobs";
import { ObjectCreatorError } from "@/lib/object-types";

export async function POST(request: Request, context: { params: Promise<{ id: string }> }) {
  try {
    sameOrigin(request);
    const body = await request.json();
    const project = await objectProject(body.projectId);
    const { id } = await context.params;
    if (body.action === "refresh") return Response.json(await objectJobs.refresh(project, id));
    if (body.action === "save" && typeof body.name === "string") return Response.json(await objectJobs.save(project, id, body.name));
    throw new ObjectCreatorError("Unknown object operation.");
  } catch (error) { return objectError(error); }
}
