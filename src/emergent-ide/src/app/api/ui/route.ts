import { objectProject, sameOrigin } from "@/lib/object-http";
import { ObjectCreatorError } from "@/lib/object-types";
import { listUis, readUi, saveUi, placeUi, uiCatalogue } from "@/lib/ui-assets";

const failure = (error: unknown) => Response.json({ error: error instanceof Error ? error.message : "UI request failed." }, { status: error instanceof ObjectCreatorError ? error.status : 400 });

export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams;
    const project = await objectProject(query.get("projectId"));
    const path = query.get("path");
    if (path) return Response.json(readUi(project, path), { headers: { "Cache-Control": "no-store" } });
    return Response.json({ uis: listUis(project), catalogue: await uiCatalogue(project) }, { headers: { "Cache-Control": "no-store" } });
  } catch (error) { return failure(error); }
}

/** Create a UI (no path) or save one (path + revision). Rust validates before anything is written. */
export async function PUT(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json();
    const project = await objectProject(body.projectId);
    if (body.path !== undefined && (typeof body.path !== "string" || typeof body.revision !== "string")) throw new Error("Saving needs the UI path and revision.");
    return Response.json(await saveUi(project, { path: body.path, revision: body.revision, ui: body.ui }));
  } catch (error) { return failure(error); }
}

export async function POST(request:Request){
 try{sameOrigin(request);const body=await request.json();return Response.json(await placeUi(await objectProject(body.projectId),body));}catch(error){return failure(error);}
}
