import { artifactResponse, objectError, objectProject } from "@/lib/object-http";
import { objectJobs } from "@/lib/object-jobs";

export async function GET(request: Request, context: { params: Promise<{ id: string }> }) {
  try {
    const query = new URL(request.url).searchParams;
    const project = await objectProject(query.get("projectId"));
    const { id } = await context.params;
    return await artifactResponse(await objectJobs.artifact(project, id, query.get("kind") ?? "model"));
  } catch (error) { return objectError(error); }
}
