import path from "node:path";
import { realpath } from "node:fs/promises";
import { containedFile, within } from "@/lib/object-files";
import { artifactResponse, objectError, objectProject } from "@/lib/object-http";
import { MAX_MODEL_BYTES, ObjectCreatorError } from "@/lib/object-types";

export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams;
    const project = await objectProject(query.get("projectId"));
    const target = await containedFile(project.root, query.get("path") ?? "", MAX_MODEL_BYTES);
    let allowed = false;
    for (const relative of project.assetRoots) {
      const root = await realpath(path.resolve(project.root, relative));
      if (within(await realpath(project.root), root) && within(root, target)) allowed = true;
    }
    if (!allowed) throw new ObjectCreatorError("Preview is outside the project's asset roots.", 403);
    return await artifactResponse(target);
  } catch (error) { return objectError(error); }
}
