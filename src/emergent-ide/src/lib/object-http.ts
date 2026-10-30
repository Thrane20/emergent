import { readFile } from "node:fs/promises";
import path from "node:path";
import { readProjectSnapshot,readProjectBinding } from "./project";
import { ObjectCreatorError } from "./object-types";

export async function objectProject(id: unknown) {
  const project = await readProjectSnapshot();
  if (id !== project.id) throw new ObjectCreatorError("The request does not match the linked project.", 409);
  return project;
}

// Session traffic validates the binding without racing the asset tree's atomic saves.
export async function inspectionProject(id:unknown){
  const project=await readProjectBinding();
  if(id!==project.id)throw new ObjectCreatorError("The request does not match the linked project.",409);
  return project;
}

export function sameOrigin(request: Request) {
  const origin = request.headers.get("origin");
  if (origin && origin !== new URL(request.url).origin) throw new ObjectCreatorError("Cross-origin changes are not allowed.", 403);
}

export function objectError(error: unknown) {
  return Response.json({ error: error instanceof ObjectCreatorError ? error.message : "Object Creator could not complete the operation." }, { status: error instanceof ObjectCreatorError ? error.status : 500 });
}

export async function artifactResponse(target: string) {
  const type = { ".glb": "model/gltf-binary", ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp" }[path.extname(target).toLowerCase()];
  if (!type) throw new ObjectCreatorError("This asset has no supported preview.");
  return new Response(new Uint8Array(await readFile(target)), { headers: { "Content-Type": type, "Cache-Control": "private, no-store", "X-Content-Type-Options": "nosniff" } });
}
