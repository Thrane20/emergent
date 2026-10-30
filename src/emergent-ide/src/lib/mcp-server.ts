import "server-only";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { McpServer, ResourceTemplate } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { readProjectSnapshot, type ProjectSnapshot } from "./project";
import { findEntries, projectIndex, RefError, resolveRef, type IndexEntry, type InstanceEntry, type ProjectIndex } from "./project-index";
import { foundationRoot } from "./project-code-context";
import { renderProjectScene } from "./scene-preview";
import { scenePlacement } from "./scene-placement";

const INSTRUCTIONS = `Emergent project server. It exposes the linked game project's scenes, scene instances and assets.

Identity rules:
- Every object has a durable ref: asset:<id>, scene:<id> or instance:<scene id>/<instance id>. Refs never contain file paths or runtime handles.
- Names are NOT unique. To act on something the user names ("the pyramid"), call find first, then use the returned ref.
- If find returns several plausible matches, ask the user which one they mean, using each match's hierarchy and scene. Never guess.
- Never invent a ref. A ref that fails to resolve was deleted or renamed: call find again.

Use describe for full detail and view to see a rendered scene at gameplay tick zero. This server is read-only in this version.`;

const refSchema = z.string().min(3).max(320).describe("A durable ref returned by find/list, e.g. instance:main/pyramid");

/** One server per request: the endpoint is stateless and all project state is re-read and cached by revision. */
export function createMcpServer() {
  const server = new McpServer({ name: "emergent", version: "0.1.0" }, { instructions: INSTRUCTIONS });

  server.registerTool("project_overview", {
    title: "Project overview",
    description: "Summarise the linked Emergent project: scenes with refs, asset counts by kind and identity diagnostics. Start here.",
    annotations: { readOnlyHint: true },
  }, async () => run(async ({ project, index }) => json(overview(project, index))));

  server.registerTool("find", {
    title: "Find by name",
    description: "Resolve a human name (\"pyramid\", \"key light\", \"mossy ground\") to ranked candidate refs. Names are not unique; inspect hierarchy/scene before choosing.",
    inputSchema: {
      query: z.string().min(1).max(120).describe("Name, ID, hierarchy path or kind to look for"),
      type: z.enum(["any", "instance", "asset", "scene"]).optional().describe("Restrict the kind of result (default any)"),
      scene: refSchema.optional().describe("Only instances in this scene ref"),
      limit: z.number().int().min(1).max(20).optional(),
    },
    annotations: { readOnlyHint: true },
  }, async ({ query, type, scene, limit }) => run(async ({ index }) => {
    if (scene) resolveRef(index, scene);
    const matches = findEntries(index, query, { type, scene, limit });
    return json({ query, matches: matches.map(({ entry, score }) => ({ ...summary(entry), score })), note: matches.length > 1 && matches[0].score === matches[1].score ? "Several equally good matches: ask the user which one they mean." : undefined });
  }));

  server.registerTool("list", {
    title: "List scenes, assets or instances",
    description: "List scenes, assets (optionally by kind) or the instances of one scene, with refs.",
    inputSchema: {
      type: z.enum(["scenes", "assets", "instances"]),
      kind: z.string().max(64).optional().describe("Asset kind filter, e.g. light, material, terrain"),
      scene: refSchema.optional().describe("Scene ref, required for instances"),
      offset: z.number().int().min(0).optional(),
      limit: z.number().int().min(1).max(200).optional(),
    },
    annotations: { readOnlyHint: true },
  }, async ({ type, kind, scene, offset = 0, limit = 100 }) => run(async ({ index }) => {
    let entries: IndexEntry[];
    if (type === "scenes") entries = index.scenes;
    else if (type === "assets") entries = index.assets.filter(asset => !kind || asset.kind === kind);
    else {
      if (!scene) throw new RefError("Pass a scene ref to list its instances.");
      resolveRef(index, scene);
      entries = index.instances.filter(instance => instance.scene === scene);
    }
    return json({ total: entries.length, offset, items: entries.slice(offset, offset + limit).map(summary) });
  }));

  server.registerTool("describe", {
    title: "Describe a ref",
    description: "Full detail for a ref: transforms, components, parent/children and asset for instances; definition and referrers for assets; the hierarchy for scenes.",
    inputSchema: { ref: refSchema, world: z.boolean().optional().describe("Instances only: also compute world transforms with the Rust engine (slower)") },
    annotations: { readOnlyHint: true },
  }, async ({ ref, world }) => run(async ({ project, index }) => json(await describe(project, index, resolveRef(index, ref), world === true))));

  server.registerTool("view", {
    title: "Render a scene",
    description: "Render a scene with the native Rust renderer at gameplay tick zero from its authored camera and return the image. Takes several seconds.",
    inputSchema: { scene: refSchema.describe("Scene ref to render") },
    annotations: { readOnlyHint: true },
  }, async ({ scene }) => run(async ({ project, index }) => {
    const entry = resolveRef(index, scene);
    if (entry.type !== "scene") throw new RefError(`${scene} is not a scene ref.`);
    const png = await renderProjectScene(project, entry.path);
    return { content: [{ type: "image" as const, data: png.toString("base64"), mimeType: "image/png" }, { type: "text" as const, text: `${entry.name} (${entry.ref}) rendered at tick zero.` }] };
  }));

  server.registerResource("project", "emergent://project", { title: "Project overview", mimeType: "application/json" },
    async uri => { const { project, index } = await context(); return { contents: [{ uri: uri.href, mimeType: "application/json", text: JSON.stringify(overview(project, index), null, 2) }] }; });

  server.registerResource("guide", "emergent://guide", { title: "Project authoring guide (AGENTS.md)", mimeType: "text/markdown" },
    async uri => { const project = await readProjectSnapshot(); return { contents: [{ uri: uri.href, mimeType: "text/markdown", text: await readFile(path.join(project.root, "AGENTS.md"), "utf8").catch(() => "This project has no AGENTS.md.") }] }; });

  server.registerResource("capabilities", "emergent://capabilities", { title: "Engine capability catalogue", description: "available / preview / planned engine capabilities; planned entries are not callable APIs", mimeType: "application/toml" },
    async uri => { const project = await readProjectSnapshot(); const root = await foundationRoot(project); return { contents: [{ uri: uri.href, mimeType: "application/toml", text: await readFile(path.join(root, "docs/capabilities/engine-capabilities.toml"), "utf8") }] }; });

  server.registerResource("scene", new ResourceTemplate("emergent://scene/{id}", {
    list: async () => { const { index } = await context(); return { resources: index.scenes.map(scene => ({ uri: `emergent://scene/${encodeURIComponent(scene.id)}`, name: scene.name, mimeType: "application/json" })) }; },
  }), { title: "Scene hierarchy", mimeType: "application/json" },
  async (uri, { id }) => { const { project, index } = await context(); const entry = resolveRef(index, `scene:${decodeURIComponent(String(id))}`); return { contents: [{ uri: uri.href, mimeType: "application/json", text: JSON.stringify(await describe(project, index, entry, false), null, 2) }] }; });

  return server;
}

async function context() { const project = await readProjectSnapshot(); return { project, index: await projectIndex(project) }; }

type ToolResult = { content: ({ type: "text"; text: string } | { type: "image"; data: string; mimeType: string })[]; isError?: boolean };
async function run(action: (state: { project: ProjectSnapshot; index: ProjectIndex }) => Promise<ToolResult>): Promise<ToolResult> {
  try { return await action(await context()); }
  catch (error) { return { isError: true, content: [{ type: "text", text: error instanceof Error ? error.message : "The Emergent project request failed." }] }; }
}
function json(value: unknown): ToolResult { return { content: [{ type: "text", text: JSON.stringify(value, null, 2) }] }; }

function overview(project: ProjectSnapshot, index: ProjectIndex) {
  const kinds: Record<string, number> = {};
  for (const asset of index.assets) kinds[asset.kind] = (kinds[asset.kind] ?? 0) + 1;
  return {
    project: { id: project.id, name: project.name, entryScene: index.entryScene },
    scenes: index.scenes.map(scene => ({ ref: scene.ref, name: scene.name, instances: scene.instances })),
    assets: { total: index.assets.length, byKind: kinds },
    diagnostics: index.diagnostics,
  };
}

function summary(entry: IndexEntry) {
  if (entry.type === "instance") return { ref: entry.ref, type: entry.type, name: entry.name, kind: entry.kind, scene: entry.scene, hierarchy: entry.hierarchy, asset: entry.asset, localPosition: entry.translation };
  if (entry.type === "scene") return { ref: entry.ref, type: entry.type, name: entry.name, instances: entry.instances };
  return { ref: entry.ref, type: entry.type, name: entry.name, kind: entry.kind };
}

async function describe(project: ProjectSnapshot, index: ProjectIndex, entry: IndexEntry, world: boolean) {
  if (entry.type === "scene") {
    const instances = index.instances.filter(instance => instance.scene === entry.ref);
    const tree = (parent: string | null): unknown[] => instances.filter(instance => instance.parent === parent).map(instance => ({ ...summary(instance), children: tree(instance.ref) }));
    return { ...summary(entry), path: entry.path, entry: index.entryScene === entry.ref, revision: entry.revision, roots: tree(null) };
  }
  if (entry.type === "asset") {
    const users = index.instances.filter(instance => instance.asset === entry.ref).map(summary);
    const definition = project.files.find(file => file.path === entry.path)?.content ?? null;
    return { ...summary(entry), path: entry.path, files: entry.files, revision: entry.revision, usedBy: users, definition: definition && definition.length <= 16384 ? definition : definition ? `${definition.slice(0, 16384)}\n… truncated` : null };
  }
  return describeInstance(project, index, entry, world);
}

async function describeInstance(project: ProjectSnapshot, index: ProjectIndex, entry: InstanceEntry, world: boolean) {
  const scene = index.scenes.find(item => item.ref === entry.scene)!;
  const placement = await scenePlacement(project, scene.path, world ? "global" : "local");
  const instance = placement.instances.find(item => item.id === entry.id);
  if (!instance) throw new RefError(`${entry.ref} no longer exists in ${scene.ref}; call find again.`);
  return {
    ...summary(entry), parent: entry.parent, children: entry.children, assetPath: entry.assetPath, sceneRevision: placement.revision,
    transform: { local: instance.transform, assetDefaults: instance.defaults, overridden: instance.overridden, world: world ? instance.display ?? null : undefined },
    material: instance.material ? { settings: instance.material, overridden: instance.materialOverridden ?? false } : undefined,
    physics: instance.physics ? { settings: instance.physics, overridden: instance.physicsOverridden ?? false } : undefined,
    camera: instance.camera ? { settings: instance.camera, overridden: instance.cameraOverridden ?? false } : undefined,
    flags: { renderable: instance.renderable, terrain: instance.terrain, raymarched: instance.raymarched, structure: instance.structure, particles: instance.particles },
  };
}
