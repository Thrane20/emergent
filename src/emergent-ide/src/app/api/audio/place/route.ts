import { placeClip } from "@/lib/audio-assets";
import { boundedBody } from "@/lib/object-files";
import { objectProject, sameOrigin } from "@/lib/object-http";

/** Adds a Point that plays a clip to a scene. */
export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = JSON.parse((await boundedBody(new Response(request.body, { headers: request.headers }), 8192)).toString("utf8"));
    const project = await objectProject(body.projectId), t = body.translation;
    if (typeof body.clip !== "string" || typeof body.scene !== "string" || !Array.isArray(t) || t.length !== 3 || !t.every((n: unknown) => typeof n === "number")) throw new Error("Choose a clip, a scene and a position.");
    return Response.json(await placeClip(project, { clip: body.clip, scene: body.scene, translation: t as [number, number, number] }));
  } catch (error) { return Response.json({ error: error instanceof Error ? error.message : "Placing the clip failed." }, { status: 400 }); }
}
