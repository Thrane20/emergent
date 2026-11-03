import { artifactResponse, objectError, objectProject } from "@/lib/object-http";
import { splitJobs } from "@/lib/model-split";

export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams, project = await objectProject(query.get("projectId"));
    return await artifactResponse(await splitJobs.artifact(project, query.get("id") ?? ""));
  } catch (error) { return objectError(error); }
}
