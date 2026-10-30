import { objectProject } from "@/lib/object-http";
import { componentRegistry } from "@/lib/scene-preview";

export async function GET(request: Request) {
  try {
    const project = await objectProject(new URL(request.url).searchParams.get("projectId"));
    return Response.json({ types: await componentRegistry(project) });
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Component registry unavailable." }, { status: 400 });
  }
}
