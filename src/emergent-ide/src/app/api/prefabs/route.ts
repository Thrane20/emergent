import { objectProject, sameOrigin } from "@/lib/object-http";
import { savePrefab } from "@/lib/prefab-assets";

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json(), project = await objectProject(body.projectId);
    if (typeof body.path !== "string" || typeof body.root !== "string" || typeof body.revision !== "string") throw new Error("Scene, root and revision are required.");
    return Response.json(await savePrefab(project, body));
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Could not save prefab." }, { status: 400 });
  }
}
