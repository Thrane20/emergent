import { objectProject, sameOrigin } from "@/lib/object-http";
import { projectIndex } from "@/lib/project-index";
import { placeAsset } from "@/lib/project-ops";

// Viewport "add an asset here": the IDE's path-based view of the MCP place_asset op, so it shares validation, journal and live refresh.
export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json();
    const project = await objectProject(body.projectId);
    if (typeof body.path !== "string" || typeof body.asset !== "string" || typeof body.revision !== "string") throw new Error("Scene, asset and revision are required.");
    const index = await projectIndex(project);
    const scene = index.scenes.find(entry => entry.path === body.path), asset = index.assets.find(entry => entry.path === body.asset);
    if (!scene || !asset) throw new Error("Choose a scene and an asset with durable IDs.");
    if (body.parent !== undefined && body.parent !== null && typeof body.parent !== "string") throw new Error("Parent must be a scene instance ID.");
    const parent = typeof body.parent === "string" ? index.instances.find(entry => entry.scene === scene.ref && entry.id === body.parent) : null;
    if (typeof body.parent === "string" && !parent) throw new Error("Choose a parent from this scene.");
    return Response.json(await placeAsset(project, { scene: scene.ref, asset: asset.ref, revision: body.revision, parent: parent?.ref ?? null, translation: body.translation, name: typeof body.name === "string" && body.name.trim() ? body.name.trim() : undefined, source: "ide" }));
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Placement failed." }, { status: 400 });
  }
}
