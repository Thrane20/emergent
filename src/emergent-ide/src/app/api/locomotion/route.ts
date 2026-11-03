import { listProfiles, locomotionCatalogue, readProfile, saveProfile } from "@/lib/locomotion-assets";
import { objectProject, sameOrigin } from "@/lib/object-http";
import { ObjectCreatorError } from "@/lib/object-types";

const failure = (error: unknown) => Response.json({ error: error instanceof Error ? error.message : "Locomotion request failed." }, { status: error instanceof ObjectCreatorError ? error.status : 400 });

export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams;
    const project = await objectProject(query.get("projectId"));
    const path = query.get("path");
    if (path) return Response.json(readProfile(project, path), { headers: { "Cache-Control": "no-store" } });
    return Response.json({ profiles: listProfiles(project), catalogue: await locomotionCatalogue(project) }, { headers: { "Cache-Control": "no-store" } });
  } catch (error) { return failure(error); }
}

/** Create a profile (no path) or save one (path + revision). Rust validates before anything is written. */
export async function PUT(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json();
    const project = await objectProject(body.projectId);
    if (body.path !== undefined && (typeof body.path !== "string" || typeof body.revision !== "string")) throw new Error("Saving needs the profile path and revision.");
    return Response.json(await saveProfile(project, { path: body.path, revision: body.revision, profile: body.profile }));
  } catch (error) { return failure(error); }
}
