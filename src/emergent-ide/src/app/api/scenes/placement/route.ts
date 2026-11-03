import { objectProject, sameOrigin } from "@/lib/object-http";
import { ObjectCreatorError } from "@/lib/object-types";
import { duplicateSceneInstance, deleteSceneInstance, renameSceneInstance, saveSceneMaterial, saveScenePhysics, saveSceneCamera, reparentSceneInstance, saveScenePlacement, saveSceneComponent, scenePlacement } from "@/lib/scene-placement";
import {saveSceneClassification} from "@/lib/scene-placement";

export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams;
    const project = await objectProject(query.get("projectId"));
    const space=query.get("space")??"local";if(space!=="local"&&space!=="global")throw new Error("Invalid transform space.");
    return Response.json(await scenePlacement(project,query.get("path") ?? "",space), {headers:{"Cache-Control":"no-store"}});
  } catch (error) { return failure(error); }
}

export async function PUT(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json();
    const project = await objectProject(body.projectId);
    if (typeof body.path !== "string" || typeof body.id !== "string" || typeof body.revision !== "string") throw new Error("Scene, instance and revision are required.");
    const space=body.space??"local";if(space!=="local"&&space!=="global")throw new Error("Invalid transform space.");
    if(body.action==="duplicate")return Response.json(await duplicateSceneInstance(project,body.path,body.id,body.revision,space));
    if(body.action==="classification")return Response.json(await saveSceneClassification(project,body.path,body.id,body.revision,body.classification,space));
    if(body.action==="rename")return Response.json(await renameSceneInstance(project,body.path,body.id,body.revision,body.name,space));
    if(body.action==="camera"){
      if(body.camera===undefined)throw new Error("Camera settings are required.");
      return Response.json(await saveSceneCamera(project,body.path,body.id,body.revision,body.camera,space));
    }
    if(body.action==="material"){
      if(body.material===undefined)throw new Error("Material settings are required.");
      return Response.json(await saveSceneMaterial(project,body.path,body.id,body.revision,body.material,space));
    }
    if(body.action==="physics"){
      if(body.physics===undefined)throw new Error("Physics settings are required.");
      return Response.json(await saveScenePhysics(project,body.path,body.id,body.revision,body.physics,space));
    }
    if(body.action==="component"){
      if(typeof body.type!=="string"||body.patch===undefined)throw new Error("Component type and changes are required.");
      return Response.json(await saveSceneComponent(project,body.path,body.id,body.revision,body.type,body.patch,space));
    }
    if(body.action==="reparent"){
      if(body.parent!==null&&typeof body.parent!=="string")throw new Error("Parent must be an instance ID or null.");
      return Response.json(await reparentSceneInstance(project,body.path,body.id,body.parent,body.revision,space));
    }
    if(body.transform===undefined||body.positionOnly!==undefined&&typeof body.positionOnly!=="boolean")throw new Error("Transform is required.");
    return Response.json(await saveScenePlacement(project,body.path,body.id,body.revision,body.transform,space,body.positionOnly===true));
  } catch (error) { return failure(error); }
}

export async function DELETE(request:Request) {
  try {
    sameOrigin(request);
    const body=await request.json();
    const project=await objectProject(body.projectId);
    if(typeof body.path!=="string"||typeof body.id!=="string"||typeof body.revision!=="string")throw new Error("Scene, instance and revision are required.");
    const space=body.space??"local";if(space!=="local"&&space!=="global")throw new Error("Invalid transform space.");
    return Response.json(await deleteSceneInstance(project,body.path,body.id,body.revision,space));
  } catch(error) {
    return Response.json({error:error instanceof Error?error.message:"Scene placement failed."},{status:error instanceof ObjectCreatorError?error.status:400});
  }
}

function failure(error: unknown) {
  return Response.json({error:error instanceof Error ? error.message : "Scene placement failed."},{status:400});
}
