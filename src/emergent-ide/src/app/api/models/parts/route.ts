import { objectProject, sameOrigin } from "@/lib/object-http";
import { ObjectCreatorError as CreatorError } from "@/lib/object-types";
import { MAX_PARTS, saveModelParts } from "@/lib/model-parts";
import { MAX_MODEL_BYTES } from "@/lib/object-types";

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    if (Number(request.headers.get("content-length") ?? 0) > 2 * MAX_MODEL_BYTES + 1024 * 1024) throw new CreatorError("The part set exceeds 200 MiB.", 413);
    const form = await request.formData(), project = await objectProject(form.get("projectId"));
    const files = new Map<string, Buffer>();
    for (const [key, value] of form.entries()) {
      if (!key.startsWith("part:") || typeof value === "string") continue;
      if (files.size >= MAX_PARTS) throw new CreatorError(`A model needs at most ${MAX_PARTS} parts.`);
      files.set(key.slice(5), Buffer.from(await value.arrayBuffer()));
    }
    const parts = JSON.parse(String(form.get("parts") ?? "null"));
    return Response.json(await saveModelParts(project, { descriptor: form.get("descriptor"), parts, files }), { status: 201 });
  } catch (error) {
    // Engine validation diagnostics are the useful part of a refused save; pass them through.
    return Response.json({ error: error instanceof Error ? error.message : "Model parts could not be saved." }, { status: error instanceof CreatorError ? error.status : 400 });
  }
}
