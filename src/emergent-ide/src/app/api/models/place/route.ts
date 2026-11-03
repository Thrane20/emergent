import { objectProject, sameOrigin } from "@/lib/object-http";
import { ObjectCreatorError as CreatorError } from "@/lib/object-types";
import { placeModelParts } from "@/lib/model-parts";

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json() as Record<string, unknown>, project = await objectProject(body.projectId);
    return Response.json(await placeModelParts(project, { parts: body.parts, scenePath: body.scenePath }), { status: 201 });
  } catch (error) {
    // Engine validation diagnostics are the useful part of a refused save; pass them through.
    return Response.json({ error: error instanceof Error ? error.message : "Model parts could not be saved." }, { status: error instanceof CreatorError ? error.status : 400 });
  }
}
