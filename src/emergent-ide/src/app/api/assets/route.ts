import { AssetCreationError, AssetDeletionError, createProjectAsset, deleteProjectAsset } from "@/lib/project-assets";
import { sameOrigin } from "@/lib/object-http";
import { readProjectSnapshot } from "@/lib/project";
import { ObjectCreatorError } from "@/lib/object-types";

export async function POST(request: Request) {
  try {
    const body = await request.json() as {
      projectId?: unknown;
      templateId?: unknown;
      name?: unknown;
      scenePath?: unknown;
    };
    const project = await readProjectSnapshot();
    if (body.projectId !== project.id) {
      throw new AssetCreationError("The request does not match the linked project.");
    }
    if (typeof body.templateId !== "string" || typeof body.name !== "string") {
      throw new AssetCreationError("Template and asset name are required.");
    }
    if (body.scenePath !== undefined && typeof body.scenePath !== "string") throw new AssetCreationError("Scene path must be a string.");
    return Response.json(
      await createProjectAsset(project, body.templateId, body.name, body.scenePath),
      { status: 201 },
    );
  } catch (error) {
    const status = error instanceof AssetCreationError ? error.status : 500;
    const message = error instanceof Error ? error.message : "Asset creation failed.";
    return Response.json({ error: message }, { status });
  }
}

export async function DELETE(request:Request) {
  try {
    sameOrigin(request);
    const body=await request.json() as {projectId?:unknown;path?:unknown};
    const project=await readProjectSnapshot();
    if(body.projectId!==project.id)throw new AssetDeletionError("The request does not match the linked project.",403);
    if(typeof body.path!=="string")throw new AssetDeletionError("Asset path is required.");
    await deleteProjectAsset(project,body.path);
    return Response.json({path:body.path});
  } catch(error) {
    const status=error instanceof AssetDeletionError?error.status:error instanceof ObjectCreatorError?error.status:500;
    const message=error instanceof Error?error.message:"Asset deletion failed.";
    return Response.json({error:message},{status});
  }
}
