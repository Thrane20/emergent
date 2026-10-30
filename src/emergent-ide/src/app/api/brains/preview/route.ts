import { previewBrain } from "@/lib/brain-assets";
import { objectProject, sameOrigin } from "@/lib/object-http";

/** Simulates the unsaved brain in the Rust runtime; returns the plain-text trace (see parseTrace). */
export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json();
    const project = await objectProject(body.projectId);
    const trace = await previewBrain(project, typeof body.path === "string" ? body.path : undefined, body.brain, body.options ?? {});
    return new Response(trace, { headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store" } });
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Preview failed." }, { status: 400 });
  }
}
