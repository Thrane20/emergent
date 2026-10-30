import { objectProject, sameOrigin } from "@/lib/object-http";
import { readPlayerSettings, savePlayerSettings } from "@/lib/player-settings";
import { ObjectCreatorError } from "@/lib/object-types";

export async function GET(request: Request) {
  try {
    const project = await objectProject(new URL(request.url).searchParams.get("projectId"));
    return Response.json(await readPlayerSettings(project), { headers: { "Cache-Control": "no-store" } });
  } catch (error) {
    return failure(error);
  }
}

export async function PUT(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json() as { projectId?: unknown; settings?: unknown; revision?: unknown };
    const project = await objectProject(body.projectId);
    return Response.json(await savePlayerSettings(project, body.settings, body.revision));
  } catch (error) {
    return failure(error);
  }
}

function failure(error: unknown) {
  return Response.json(
    { error: error instanceof Error ? error.message : "Player settings could not be loaded." },
    { status: error instanceof ObjectCreatorError ? error.status : 400 },
  );
}
