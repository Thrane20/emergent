import { boundedBody } from "@/lib/object-files";
import { objectError, objectProject, sameOrigin } from "@/lib/object-http";
import { importRigged, inspectRig, saveRigResult } from "@/lib/model-rig";
import { RIG_ENDPOINTS, splitJobs } from "@/lib/model-split";
import { MAX_MODEL_BYTES, ObjectCreatorError } from "@/lib/object-types";

/** The engine's rig report for `path`, plus that model's auto-rig jobs. */
export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams, project = await objectProject(query.get("projectId")), file = query.get("path") ?? "";
    const [report, jobs] = await Promise.all([inspectRig(project, file), splitJobs.list(project, file, true)]);
    return Response.json({ report, jobs }, { headers: { "Cache-Control": "no-store" } });
  } catch (error) { return objectError(error); }
}

/**
 * `?action=import&name=&extension=` imports the raw body as a rigged model or BVH take.
 * JSON actions: `autorig` (paid fal job), `refresh`, `save` (copy a ready result into the project).
 */
export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const query = new URL(request.url).searchParams;
    if (query.get("action") === "import") {
      const project = await objectProject(query.get("projectId"));
      const bytes = await boundedBody(new Response(request.body, { headers: request.headers }), MAX_MODEL_BYTES);
      return Response.json(await importRigged(project, { name: query.get("name") ?? "", extension: query.get("extension") ?? "", bytes }), { status: 201 });
    }
    const body = await request.json() as Record<string, unknown>, project = await objectProject(body.projectId);
    if (body.action === "autorig") return Response.json(await splitJobs.create(project, { id: body.id, model: body.model, endpoint: Object.keys(RIG_ENDPOINTS)[0] }), { status: 201 });
    if (body.action === "refresh" && typeof body.id === "string") return Response.json(await splitJobs.refresh(project, body.id));
    if (body.action === "save" && typeof body.id === "string" && typeof body.name === "string") return Response.json(await saveRigResult(project, await splitJobs.artifact(project, body.id), body.name), { status: 201 });
    throw new ObjectCreatorError("Unknown rig action.");
  } catch (error) { return objectError(error); }
}
