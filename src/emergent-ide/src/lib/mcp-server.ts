import "server-only";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { McpServer, ResourceTemplate } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { readProjectSnapshot, type ProjectSnapshot } from "./project";
import { findEntries, projectIndex, RefError, resolveRef, type IndexEntry, type InstanceEntry, type ProjectIndex } from "./project-index";
import { foundationRoot } from "./project-code-context";
import { componentRegistry, renderProjectScene } from "./scene-preview";
import { scenePlacement } from "./scene-placement";
import { assignIds, changeHistory, deleteInstance, placeAsset, renameInstance, reparentInstance, setComponent, setObjectComponent, setEnvironment, setTransform, undoChange, type PlaceInput, type TransformInput } from "./project-ops";

const INSTRUCTIONS = `Emergent project server. It exposes the linked game project's scenes, scene instances and assets.

Identity rules:
- Every object has a durable ref: asset:<id>, scene:<id> or instance:<scene id>/<instance id>. Refs never contain file paths or runtime handles.
- Names are NOT unique. To act on something the user names ("the pyramid"), call find first, then use the returned ref.
- If find returns several plausible matches, ask the user which one they mean, using each match's hierarchy and scene. Never guess.
- Never invent a ref. A ref that fails to resolve was deleted or renamed: call find again.

Use describe for full detail and view to see a rendered scene at gameplay tick zero.

Editing:
- Every edit needs the scene "revision" from describe (of the instance or the scene). Each successful edit returns the new revision; use it for the next edit in that scene. A stale revision is refused; describe again.
- Edits change authored scene files (never a running game). They are validated by the Rust engine, journalled with a changeId, and appear live in the open IDE.
- Pass dryRun: true to preview the diff and validation without writing.
- Component edits (set_material, set_physics, set_camera, set_environment) take only the fields to change; null resets the instance to its asset defaults. Colours are linear RGB in 0..1.
- Registry components (list_component_types; e.g. emergent.light for a light's colour, intensity, range, cone and shadows) are set with set_component: pass only the fields to change, a field value of null to reset it, enabled to switch the component off or on, or fields null to remove the override. A light's direction is its rotation (set_transform).
- undo reverts the most recent MCP change (or a given changeId) and refuses if the files changed since. history lists recent changes.
- Units are metres and degrees; +Y is up.`;

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

  const revisionSchema = z.string().regex(/^[0-9a-f]{64}$/).describe("The scene revision from describe or from the previous edit's result");
  const dryRunSchema = z.boolean().optional().describe("Validate and return the diff without writing");
  const vectorSchema = z.tuple([z.number(), z.number(), z.number()]);
  const edit = (name: string, title: string, description: string, inputSchema: z.ZodRawShape, action: (project: ProjectSnapshot, args: Record<string, unknown>) => Promise<unknown>, destructive = false) =>
    server.registerTool(name, { title, description, inputSchema, annotations: { readOnlyHint: false, destructiveHint: destructive, idempotentHint: false } }, async (args: Record<string, unknown>) => run(async ({ project }) => json(await action(project, args))));

  edit("set_transform", "Move, rotate or scale", "Change an instance's position, rotation (degrees) and/or scale. Omitted parts keep their current value. space local (default) is relative to the parent; global is world space.", {
    ref: refSchema, revision: revisionSchema, translation: vectorSchema.optional(), rotation_degrees: vectorSchema.optional(), scale: vectorSchema.optional(), space: z.enum(["local", "global"]).optional(), dryRun: dryRunSchema,
  }, (project, args) => setTransform(project, args as unknown as TransformInput));
  edit("rename", "Rename an instance", "Set an instance's display name in its scene (1-64 characters). The ref does not change.", { ref: refSchema, revision: revisionSchema, name: z.string().min(1).max(64), dryRun: dryRunSchema },
    (project, args) => renameInstance(project, args as { ref: string; revision: string; name: string; dryRun?: boolean }));
  edit("reparent", "Change an instance's parent", "Move an instance under another instance in the same scene, or to the scene root (parent null). Keeps the local transform, so it may move in world space.", { ref: refSchema, revision: revisionSchema, parent: refSchema.nullable(), dryRun: dryRunSchema },
    (project, args) => reparentInstance(project, args as { ref: string; revision: string; parent: string | null; dryRun?: boolean }));
  edit("delete", "Delete an instance", "Remove an instance and all of its descendants from its scene. The shared asset is not deleted. Undoable with undo.", { ref: refSchema, revision: revisionSchema, dryRun: dryRunSchema },
    (project, args) => deleteInstance(project, args as { ref: string; revision: string; dryRun?: boolean }), true);
  edit("place_asset", "Place an asset in a scene", "Add a new instance of an entity asset to a scene, optionally under a parent, with a name and transform (omitted parts use the asset defaults). Returns the new instance ref.", {
    asset: refSchema, scene: refSchema, revision: revisionSchema, parent: refSchema.nullable().optional(), name: z.string().min(1).max(64).optional(), translation: vectorSchema.optional(), rotation_degrees: vectorSchema.optional(), scale: vectorSchema.optional(), dryRun: dryRunSchema,
  }, (project, args) => placeAsset(project, args as unknown as PlaceInput));
  for (const component of ["material", "physics", "camera"] as const) {
    const hints = { material: "shading (flat|smooth|unlit), albedo [r,g,b] linear 0..1, specular, shininess, emission [r,g,b], emission_intensity, bump_strength, cast_shadows, receive_shadows", physics: "enabled, body (static|dynamic), mass, friction, restitution, gravity_scale, collider {shape box|terrain, center, half_extents}", camera: "projection (perspective|isometric), field_of_view_degrees, view_size, near, far" }[component];
    edit(`set_${component}`, `Set ${component}`, `Change ${component} fields on one scene instance (an instance override; the shared asset is unchanged). Fields: ${hints}. Pass null to reset to the asset defaults.`, {
      ref: refSchema, revision: revisionSchema, settings: z.record(z.string(), z.unknown()).nullable(), dryRun: dryRunSchema,
    }, (project, args) => setComponent(project, { ...(args as { ref: string; revision: string; settings: unknown; dryRun?: boolean }), component }));
  }
  server.registerTool("list_component_types", { title: "List component types", description: "Component types that can be set on scene instances, with each field's kind, range, unit and which object variants accept it.", inputSchema: {}, annotations: { readOnlyHint: true } },
    async () => run(async ({ project }) => json({ types: await componentRegistry(project) })));
  edit("set_component", "Set a component", "Add or change a registry component (see list_component_types) on one scene instance. fields: only the fields to change (null resets one to the asset value), or null to remove the component override. enabled false switches the component off (an off light emits nothing).", {
    ref: refSchema, revision: revisionSchema, type: z.string().min(3).max(96), fields: z.record(z.string(), z.unknown()).nullable(), enabled: z.boolean().optional(), dryRun: dryRunSchema,
  }, (project, args) => setObjectComponent(project, args as { ref: string; revision: string; type: string; fields: Record<string, unknown> | null; enabled?: boolean; dryRun?: boolean }));
  edit("set_environment", "Set scene environment", "Change a scene's ambient light, background or skybox. Fields: ambient_color [r,g,b], ambient_intensity, background_color [r,g,b], skybox (an image-library skybox path or null), skybox_rotation (degrees), skybox_intensity.", {
    scene: refSchema, revision: revisionSchema, settings: z.record(z.string(), z.unknown()), dryRun: dryRunSchema,
  }, (project, args) => setEnvironment(project, args as { scene: string; revision: string; settings: unknown; dryRun?: boolean }));
  edit("undo", "Undo a change", "Revert the most recent MCP change, or the given changeId. Refused if any of its files changed since.", { changeId: z.string().uuid().optional(), dryRun: dryRunSchema },
    (project, args) => undoChange(project, args as { changeId?: string; dryRun?: boolean }), true);
  edit("assign_ids", "Assign durable IDs", "Give a durable ID to every file reported as missing-id (adds an id to TOML definitions, or a media.asset.json descriptor for loose media). Use dryRun first.", { dryRun: dryRunSchema },
    (project, args) => assignIds(project, args as { dryRun?: boolean }));
  server.registerTool("history", { title: "Change history", description: "Recent MCP changes with changeIds, newest first.", inputSchema: { limit: z.number().int().min(1).max(100).optional() }, annotations: { readOnlyHint: true } },
    async ({ limit }) => run(async ({ project }) => json({ changes: await changeHistory(project, limit) })));

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
    return { ...summary(entry), path: entry.path, entry: index.entryScene === entry.ref, revision: (await scenePlacement(project, entry.path)).revision, roots: tree(null) };
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
    ...summary(entry), parent: entry.parent, children: entry.children, assetPath: entry.assetPath, revision: placement.revision,
    transform: { local: instance.transform, assetDefaults: instance.defaults, overridden: instance.overridden, world: world ? instance.display ?? null : undefined },
    material: instance.material ? { settings: instance.material, overridden: instance.materialOverridden ?? false } : undefined,
    physics: instance.physics ? { settings: instance.physics, overridden: instance.physicsOverridden ?? false } : undefined,
    camera: instance.camera ? { settings: instance.camera, overridden: instance.cameraOverridden ?? false } : undefined,
    flags: { renderable: instance.renderable, terrain: instance.terrain, raymarched: instance.raymarched, structure: instance.structure, particles: instance.particles },
  };
}
