import { objectProject, sameOrigin } from "@/lib/object-http";
import { renderProjectScene } from "@/lib/scene-preview";

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json();
    const project = await objectProject(body.projectId);
    if (typeof body.path !== "string") throw new Error("A scene path is required.");
    return new Response(new Uint8Array(await renderProjectScene(project, body.path)), { headers: { "Content-Type": "image/png", "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" } });
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Scene could not be rendered." }, { status: 400 });
  }
}
