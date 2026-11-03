import { objectError, objectProject, sameOrigin } from "@/lib/object-http";
import { generatedModels } from "@/lib/model-parts";
import { splitJobs, SPLIT_ENDPOINTS } from "@/lib/model-split";
import { ObjectCreatorError } from "@/lib/object-types";

export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams, project = await objectProject(query.get("projectId"));
    const model = query.get("model") ?? undefined;
    return Response.json({ models: generatedModels(project), jobs: await splitJobs.list(project, model), endpoints: Object.entries(SPLIT_ENDPOINTS).map(([id, value]) => ({ id, label: value.label })) }, { headers: { "Cache-Control": "no-store" } });
  } catch (error) { return objectError(error); }
}

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json() as Record<string, unknown>, project = await objectProject(body.projectId);
    if (body.action === "split") return Response.json(await splitJobs.create(project, body), { status: 201 });
    if (body.action === "refresh" && typeof body.id === "string") return Response.json(await splitJobs.refresh(project, body.id));
    throw new ObjectCreatorError("Unknown model action.");
  } catch (error) { return objectError(error); }
}
