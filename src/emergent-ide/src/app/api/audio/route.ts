import { clipWaveform, deleteClip, importClip, inspectClip, listClips, readClip, saveClip } from "@/lib/audio-assets";
import { MAX_AUDIO_BYTES, type AudioCategory } from "@/lib/audio-types";
import { boundedBody } from "@/lib/object-files";
import { objectProject, sameOrigin } from "@/lib/object-http";

const failure = (error: unknown) => Response.json({ error: error instanceof Error ? error.message : "Audio request failed." }, { status: 400 });
const json = async (request: Request) => JSON.parse((await boundedBody(new Response(request.body, { headers: request.headers }), 16 * 1024)).toString("utf8"));

/** The clip library, one clip (`path`), one clip decoded by the engine (`path` + `inspect`), or its cached waveform outline (`path` + `waveform`). */
export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams, project = await objectProject(query.get("projectId")), clipPath = query.get("path");
    if (!clipPath) return Response.json({ clips: listClips(project) }, { headers: { "Cache-Control": "no-store" } });
    if (query.get("waveform")) return Response.json({ waveform: await clipWaveform(project, clipPath) }, { headers: { "Cache-Control": "no-store" } });
    const clip = readClip(project, clipPath);
    return Response.json(query.get("inspect") ? { ...clip, info: await inspectClip(project, clipPath) } : clip, { headers: { "Cache-Control": "no-store" } });
  } catch (error) { return failure(error); }
}

/** Imports an audio file sent as the raw request body; name and category travel in the query. */
export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const query = new URL(request.url).searchParams, project = await objectProject(query.get("projectId"));
    const bytes = await boundedBody(new Response(request.body, { headers: request.headers }), MAX_AUDIO_BYTES);
    return Response.json(await importClip(project, { name: query.get("name") ?? "", category: (query.get("category") ?? "sfx") as AudioCategory, bytes }), { status: 201 });
  } catch (error) { return failure(error); }
}

export async function PUT(request: Request) {
  try {
    sameOrigin(request);
    const body = await json(request), project = await objectProject(body.projectId);
    if (typeof body.path !== "string" || typeof body.revision !== "string") throw new Error("Saving needs the clip path and revision.");
    return Response.json(await saveClip(project, { path: body.path, revision: body.revision, settings: body.settings }));
  } catch (error) { return failure(error); }
}

export async function DELETE(request: Request) {
  try {
    sameOrigin(request);
    const body = await json(request), project = await objectProject(body.projectId);
    if (typeof body.path !== "string") throw new Error("Choose a clip to delete.");
    return Response.json(await deleteClip(project, body.path));
  } catch (error) { return failure(error); }
}
