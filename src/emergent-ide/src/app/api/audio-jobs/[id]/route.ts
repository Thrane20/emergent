import { audioJobs } from "@/lib/audio-jobs";
import { boundedBody } from "@/lib/object-files";
import { objectProject, sameOrigin } from "@/lib/object-http";

export async function POST(request: Request, context: { params: Promise<{ id: string }> }) {
  try {
    sameOrigin(request);
    const { id } = await context.params, body = JSON.parse((await boundedBody(new Response(request.body, { headers: request.headers }), 8192)).toString("utf8")), project = await objectProject(body.projectId);
    if (body.action === "refresh") return Response.json(await audioJobs.refresh(project, id));
    if (body.action === "keep") return Response.json(await audioJobs.keep(project, id, body.name));
    if (body.action === "delete") return Response.json(await audioJobs.discard(project, id));
    throw new Error("Choose refresh, keep or delete.");
  } catch (error) { return Response.json({ error: error instanceof Error ? error.message : "Audio job update failed." }, { status: 400 }); }
}
