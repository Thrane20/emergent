import { readFile } from "node:fs/promises";
import path from "node:path";
import { audioJobs } from "@/lib/audio-jobs";
import { objectProject } from "@/lib/object-http";

const TYPES: Record<string, string> = { ".wav": "audio/wav", ".mp3": "audio/mpeg", ".ogg": "audio/ogg", ".flac": "audio/flac" };
export async function GET(request: Request, context: { params: Promise<{ id: string }> }) {
  try {
    const { id } = await context.params, file = await audioJobs.artifact(await objectProject(new URL(request.url).searchParams.get("projectId")), id);
    return new Response(new Uint8Array(await readFile(file)), { headers: { "Content-Type": TYPES[path.extname(file)] ?? "application/octet-stream", "Cache-Control": "private, no-store", "X-Content-Type-Options": "nosniff" } });
  } catch (error) { return Response.json({ error: error instanceof Error ? error.message : "Audio is not ready." }, { status: 404 }); }
}
