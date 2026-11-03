import { sameOrigin } from "@/lib/object-http";
import { ObjectCreatorError } from "@/lib/object-types";
import { createPlugin, PluginError, readProjectPlugins } from "@/lib/project-plugins";
import { readProjectBinding } from "@/lib/project";
import { NoActiveProjectError } from "@/lib/workspace";

const noStore = (value: unknown, status = 200) => Response.json(value, { status, headers: { "Cache-Control": "no-store" } });

export async function GET() {
  try { return noStore({ plugins: await readProjectPlugins((await readProjectBinding()).root) }); } catch (error) { return failure(error); }
}

export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = await request.json() as Record<string, unknown>;
    if (body.action !== "new") throw new PluginError("Unknown plugin action.");
    return noStore(await createPlugin(await readProjectBinding(), body), 201);
  } catch (error) { return failure(error); }
}

function failure(error: unknown) {
  return noStore({ error: error instanceof Error ? error.message : "Plugin action failed." }, error instanceof PluginError || error instanceof ObjectCreatorError || error instanceof NoActiveProjectError ? error.status : 500);
}
