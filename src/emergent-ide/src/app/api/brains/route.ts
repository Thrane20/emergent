import { brainCatalogue, listBrains, readBrain, saveBrain } from "@/lib/brain-assets";
import { objectProject, sameOrigin } from "@/lib/object-http";
import { ObjectCreatorError } from "@/lib/object-types";

const failure = (error: unknown) => Response.json({ error: error instanceof Error ? error.message : "Brain request failed." }, { status: error instanceof ObjectCreatorError ? error.status : 400 });

export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams;
    const project = await objectProject(query.get("projectId"));
    const path = query.get("path");
    if (path) return Response.json(readBrain(project, path), { headers: { "Cache-Control": "no-store" } });
    return Response.json({ brains: listBrains(project), catalogue: await brainCatalogue(project) }, { headers: { "Cache-Control": "no-store" } });
  } catch (error) { return failure(error); }
}

/** Create a brain (no path) or save one (path + revision). Rust validates before anything is written. */
export async function PUT(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json();
    const project = await objectProject(body.projectId);
    if (body.path !== undefined && (typeof body.path !== "string" || typeof body.revision !== "string")) throw new Error("Saving needs the brain path and revision.");
    return Response.json(await saveBrain(project, { path: body.path, revision: body.revision, brain: body.brain }));
  } catch (error) { return failure(error); }
}
