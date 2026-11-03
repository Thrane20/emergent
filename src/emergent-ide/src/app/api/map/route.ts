import { boundedBody } from "@/lib/object-files";
import { objectProject, sameOrigin } from "@/lib/object-http";
import { addMapItems, deleteMapItem, moveMapItem, renameMapItem, sceneMap, setMapComponent, setMapWind, toggleWaypointLink } from "@/lib/scene-map";
import type { MapItemInput } from "@/lib/scene-map-types";
import { undoChange } from "@/lib/project-ops";

const failure = (error: unknown) => Response.json({ error: error instanceof Error ? error.message : "Map request failed." }, { status: 400 });
const scenePath = (value: unknown) => { if (typeof value !== "string" || !value.endsWith(".scene.toml")) throw new Error("Choose a scene."); return value; };
const finite = (value: unknown) => typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 1e6;

/** The scene's map: instances, components, world matrices and top-down outlines. */
export async function GET(request: Request) {
  try {
    const query = new URL(request.url).searchParams, project = await objectProject(query.get("projectId"));
    return Response.json(await sceneMap(project, scenePath(query.get("scene"))), { headers: { "Cache-Control": "no-store" } });
  } catch (error) { return failure(error); }
}

/** One map edit (journalled and undoable); answers with the updated map. */
export async function POST(request: Request) {
  try {
    sameOrigin(request);
    const body = JSON.parse((await boundedBody(new Response(request.body, { headers: request.headers }), 256 * 1024)).toString("utf8"));
    const project = await objectProject(body.projectId), scene = scenePath(body.scene), revision = String(body.revision ?? "");
    const id = typeof body.id === "string" ? body.id : "";
    switch (body.action) {
      case "add": {
        const items = (Array.isArray(body.items) ? body.items : []) as MapItemInput[];
        if (!items.every(item => (item.kind === "waypoint" || item.kind === "zone") && finite(item.x) && finite(item.z))) throw new Error("Each map item needs a kind and a position.");
        const added = await addMapItems(project, { scene, revision, parent: typeof body.parent === "string" ? body.parent : null, items, path: body.path === true, name: typeof body.name === "string" ? body.name.slice(0, 64) : undefined });
        return Response.json({ map: await sceneMap(project, scene), added: added.ids, group: added.group });
      }
      case "move":
        if (!finite(body.x) || !finite(body.z)) throw new Error("Choose where to move it.");
        await moveMapItem(project, { scene, revision, id, x: body.x, z: body.z }); break;
      case "link":
        await toggleWaypointLink(project, { scene, revision, from: String(body.from ?? ""), to: String(body.to ?? "") }); break;
      case "component":
        if (typeof body.type !== "string" || (body.fields !== null && (typeof body.fields !== "object" || Array.isArray(body.fields)))) throw new Error("Choose a component and its fields.");
        await setMapComponent(project, { scene, revision, id, type: body.type, fields: body.fields }); break;
      case "rename":
        if (typeof body.name !== "string" || !body.name.trim() || body.name.length > 64) throw new Error("Names are 1-64 characters.");
        await renameMapItem(project, { scene, revision, id, name: body.name.trim() }); break;
      case "delete":
        await deleteMapItem(project, { scene, revision, id }); break;
      case "undo":
        await undoChange(project, {}); break;
      case "wind":
        await setMapWind(project, { scene, revision, wind: body.wind ?? null }); break;
      default: throw new Error("Unknown map action.");
    }
    return Response.json({ map: await sceneMap(project, scene) });
  } catch (error) { return failure(error); }
}
