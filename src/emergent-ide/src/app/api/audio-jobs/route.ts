import { audioJobs } from "@/lib/audio-jobs";
import { boundedBody } from "@/lib/object-files";
import { objectProject, sameOrigin } from "@/lib/object-http";

const failure = (error: unknown) => Response.json({ error: error instanceof Error ? error.message : "Audio generation failed." }, { status: 400 });
export async function GET(request: Request) {
  try { return Response.json({ jobs: await audioJobs.list(await objectProject(new URL(request.url).searchParams.get("projectId"))) }, { headers: { "Cache-Control": "no-store" } }); }
  catch (error) { return failure(error); }
}
/** Starts one paid generation. The client picks the job ID, so retrying the same request never submits twice. */
export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = JSON.parse((await boundedBody(new Response(request.body, { headers: request.headers }), 16 * 1024)).toString("utf8"));
    if (typeof body.id !== "string") throw new Error("Job ID required.");
    return Response.json(await audioJobs.create(await objectProject(body.projectId), body.id, body.request));
  } catch (error) { return failure(error); }
}
