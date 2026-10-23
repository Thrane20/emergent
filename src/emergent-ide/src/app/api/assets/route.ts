import { AssetCreationError, createProjectAsset } from "@/lib/project-assets";
import { readProjectSnapshot } from "@/lib/project";

export async function POST(request: Request) {
  try {
    const body = await request.json() as {
      projectId?: unknown;
      templateId?: unknown;
      name?: unknown;
    };
    const project = await readProjectSnapshot();
    if (body.projectId !== project.id) {
      throw new AssetCreationError("The request does not match the linked project.");
    }
    if (typeof body.templateId !== "string" || typeof body.name !== "string") {
      throw new AssetCreationError("Template and asset name are required.");
    }
    return Response.json(
      await createProjectAsset(project, body.templateId, body.name),
      { status: 201 },
    );
  } catch (error) {
    const status = error instanceof AssetCreationError ? error.status : 500;
    const message = error instanceof Error ? error.message : "Asset creation failed.";
    return Response.json({ error: message }, { status });
  }
}
