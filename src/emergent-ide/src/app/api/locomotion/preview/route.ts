import { previewProfile } from "@/lib/locomotion-assets";
import { objectProject, sameOrigin } from "@/lib/object-http";

/** Simulates the unsaved profile along a course in the Rust runtime; returns the plain-text trace. */
export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json();
    const project = await objectProject(body.projectId);
    const trace = await previewProfile(project, typeof body.path === "string" ? body.path : undefined, body.profile, body.options ?? {});
    return new Response(trace, { headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store" } });
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Preview failed." }, { status: 400 });
  }
}
