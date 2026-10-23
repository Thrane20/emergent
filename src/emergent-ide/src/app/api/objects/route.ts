import { boundedBody } from "@/lib/object-files";
import { objectError, objectProject, sameOrigin } from "@/lib/object-http";
import { objectJobs } from "@/lib/object-jobs";
import { MAX_IMAGE_BYTES, ObjectCreatorError } from "@/lib/object-types";

export async function GET(request: Request) {
  try {
    const project = await objectProject(new URL(request.url).searchParams.get("projectId"));
    return Response.json(await objectJobs.list(project));
  } catch (error) { return objectError(error); }
}

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const bytes = await boundedBody(new Response(request.body, { headers: request.headers }), MAX_IMAGE_BYTES + 64 * 1024);
    const form = await new Response(new Uint8Array(bytes), { headers: { "Content-Type": request.headers.get("content-type") ?? "" } }).formData();
    const project = await objectProject(form.get("projectId"));
    const id = form.get("id"), name = form.get("name"), image = form.get("image");
    if (typeof id !== "string" || typeof name !== "string" || !(image instanceof File)) throw new ObjectCreatorError("Object name and reference image are required.");
    return Response.json(await objectJobs.create(project, id, name, Buffer.from(await image.arrayBuffer())), { status: 201 });
  } catch (error) { return objectError(error); }
}
