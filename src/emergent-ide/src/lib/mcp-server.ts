import "server-only";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { McpServer, ResourceTemplate } from "@modelcontextprotocol/sdk/server/mcp.js";
import { parse } from "smol-toml";
import { z } from "zod";
import { readProjectSnapshot, type ProjectSnapshot } from "./project";
import { findEntries, projectIndex, RefError, resolveRef, type IndexEntry, type InstanceEntry, type ProjectIndex } from "./project-index";
import { foundationRoot } from "./project-code-context";
import { componentRegistry, gameTool, renderProjectScene } from "./scene-preview";
import { scenePlacement } from "./scene-placement";
import { behaviourCatalogues, createBrain, describeBrain, describeLocomotion, editBrain, previewBrainSummary, previewLocomotionSummary, writeLocomotion, type BrainOperation } from "./mcp-behaviour";
import { createUi, describeUi, editUi, previewUi } from "./mcp-ui";
import { createShader, describeShader, editShader, SHADER_OUTPUTS, shaderCatalogue, type ShaderOperation } from "./mcp-shader";
import { shaderOps } from "./shader-graph";
import { uiCatalogue } from "./ui-assets";
import { generateAudioClip } from "./audio-jobs";
import { assignIds, changeHistory, deleteInstance, placeAsset, renameInstance, reparentInstance, setComponent, setObjectComponent, setEnvironment, setTransform, setWind, undoChange, type PlaceInput, type TransformInput } from "./project-ops";

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
- Units are metres and degrees; +Y is up.

Behaviour (what objects do and how they move):
- Components on an instance are the behaviour hooks (list_component_types): emergent.brain (where it wants to go/look and what it signals), emergent.locomotion (how it physically moves: wheels, drone, aircraft, projectile), emergent.group (membership other brains target, e.g. "enemies"), character controller (WASD), spin, follow, look_at, follow_camera, light, emergent.animator (plays a skinned model's clip by name; its motion field takes comma-separated .bvh or rigged .glb files whose clips are retargeted onto a humanoid model; blend "idle:0, walk, run" blends clips by how fast the object moves, played in place so locomotion drives movement; turn_rate turns the model to face its movement, foot_ik plants its feet on physics ground and look_at turns its head towards an instance; the biped locomotion type pairs with it; the standard template ships assets/models/mannequin.glb with assets/mocap idle/walk/run takes; describe the model asset for its joints, humanoid mapping and clips), emergent.audio_source (plays a clip from the object: mode loop/one_shot, volume, min/max distance, Doppler), emergent.audio_listener (hears the scene; without one the active camera listens) and project-defined types. describe an instance to see its components.
- generate_audio makes a sound effect, music or speech clip with fal. It costs money: ask the user first.
- In set_component fields, brains, locomotion profiles and audio clips (find type asset, kind audio) may be given as asset refs and target objects as instance refs; they are stored as paths and instance IDs.
- Brains are project assets (find type asset, kind brain). list_brain_nodes gives every node type and field. A brain is a tree: composites (Blend, Priority, Selector, Sequence, Utility) combine children; decorators (conditions, scores, Altitude, Boost, On signal) gate one child; steering leaves output movement; Look & aim nodes output where to face; Signal sends a named signal others hear next tick. Priority picks per channel: movement from the first branch that moves, look from the first that looks, signal from the first that signals. Prefer group names over object IDs so a brain works in any scene.
- Create with create_brain (nodes listed parent-first; give ids so children can name their parent), change with edit_brain (add/update/remove/move/reorder by node id, using the revision from describe), and check behaviour with preview_brain (runs the engine and returns numbers: spacing, speed, distance to targets).
- Several brains can run on one object: the Brain component's stack field ("Also run") lists further brains; movement adds up, the first look wins, every signal is sent.
- Brain modes switch behaviour at run time (park, taxi, take off, hold course, mission…): set the Brain component's modes field to a list of {name (a-z0-9_-), brain, active (at start), on_next (started when it finishes), on_previous (started when aborted), keep_running} instead of brain/stack (set those to null). Active modes run together; earlier modes steer first. Next mode / Previous mode / Switch mode nodes change modes (under On signal they obey other objects). Aircraft modes use the Taxi, Take off, Hold course and Land nodes.
- Locomotion profiles are project assets (kind locomotion); list_locomotion_types gives types and fields. Create or change them with write_locomotion; preview_locomotion drives one along a course (waypoints, orbit, stop, launch, circuit) and reports speed, height, steer/bank and landing. fixed_wing_physics is a rigid-body aircraft (needs a dynamic box physics body with friction 0): brain nodes take_off and land (onto a rectangular map zone, or the nearest of a group) fly it; its preview reports phases and touchdown. A brain supplies intent (velocity, target, brake, hover, boost, altitude) and the profile carries it out within its limits; give the Brain component a max_speed at least the profile's.
- UIs are project assets (kind ui). Use list_ui_widgets for the native widget catalogue, create_ui for a parent-first node tree, describe for node IDs and revision, and edit_ui for widget fields or hierarchy. Only panels hold children. x/y/w/h are lengths such as 8px, 30% or 100%-16px; colours are linear RGB. preview_ui returns paged canvas rectangles and action labels after native validation. These are authored layouts; in-game UI rendering, placement and action dispatch are not implemented yet. Use these tools for UI authoring rather than the embedded code fallback.
- A car's wheels: on its emergent.locomotion component set steering_wheels (front: steer and roll) and rolling_wheels (roll only) to its wheel child instance refs, plus wheel_radius. Model the car facing -Z.
- Locomotion, follow camera, character controller and follow each move the object, so use one; spin, look_at and a looking brain each turn it, so use one. None of them work on a dynamic physics body.
- Shaders are project assets (kind shader): a node graph of at most 32 scalar nodes feeding a surface output (red, green, blue, emission, opacity, displacement), with up to 16 exposed parameters. list_shader_nodes gives node types, their named inputs and the outputs. describe a shader for its nodes, connections, revision and the files that use it. create_shader (optionally from a template) and edit_shader change it by node ID (add_node, update_node, remove_node, connect, set_parameter, remove_parameter); every input and output must end up connected, and saves are validated by the engine against every scene. Apply one with set_material { shader: <shader ref>, cast_shadows: false, shader_parameters: { id: value } }.
- Airports are scene-map items tagged with emergent.metadata values { aeroway: … } (OpenStreetMap values): aerodrome (circle/rect zone; items inside belong to it; pattern_height in full-size metres; scale 0.01-10, default 1, shrinks circuits, pattern height, holding fix, spacing and approach gates for miniature airports, while each aircraft's speed still sets its turn size), runway (rect zone landed along its local -Z; direction forward|reverse|both, circuit left|right, active true), threshold (rect zone over a runway end: landings aim from its outer edge and take-offs line up in it; when any exist only those ends are used, overriding direction), taxiway (waypoint lines; they need not be linked to each other: lines join where they cross or where an end is within 15 m of another line), holding_position (waypoint by a runway end; joins the nearest taxiway line if unlinked), parking_position (stand: rect zone or waypoint; joins the nearest taxiway line if unlinked), tower (any object, usually a rect zone: makes the airport sequence its traffic), circuit (a drawn traffic pattern: a linked waypoint path tagged aeroway circuit and leg entry|downwind|base|final, ending on final before a threshold box; it replaces the generated circuit for that runway end. Each waypoint may set emergent.waypoint entry_from/entry_to and exit_from/exit_to, compass degrees clockwise, e.g. 350 to 10, and altitude in metres above the runway; aircraft manoeuvre to cross each point within its entry range and leave within its exit range), holding_fix (optional waypoint). Physical aircraft brains use depart_airport (taxi to the holding point, wait for the tower, take off, climb) and arrive_airport (join the sequence, hold if the circuit is full, land, vacate, park on a free stand), targeting the aerodrome or a group of them. Towers land into the wind on runways tagged direction both (the default); set it with set_wind.
- Behaviour runs only in Run (the editor view is paused at tick zero); view shows tick zero.`;

const refSchema = z.string().min(3).max(320).describe("A durable ref returned by find/list, e.g. instance:main/pyramid");

/** One server per request: the endpoint is stateless and all project state is re-read and cached by revision. */
export function createMcpServer(readProject = readProjectSnapshot) {
  const server = new McpServer({ name: "emergent", version: "0.1.0" }, { instructions: INSTRUCTIONS });

  const context = async () => { const project = await readProject(); return { project, index: await projectIndex(project) }; };
  const run = (action: Parameters<typeof runWithContext>[0]) => runWithContext(action, context);

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
    edit(`set_${component}`, `Set ${component}`, `Change ${component} fields on one scene instance (an instance override; the shared asset is unchanged). Fields: ${hints}. Pass null to reset to the asset defaults.${component === "material" ? " A shader material: shader (a shader asset ref), cast_shadows false, shader_parameters { id: value }." : ""}`, {
      ref: refSchema, revision: revisionSchema, settings: z.record(z.string(), z.unknown()).nullable(), dryRun: dryRunSchema,
    }, (project, args) => setComponent(project, { ...(args as { ref: string; revision: string; settings: unknown; dryRun?: boolean }), component }));
  }
  server.registerTool("list_component_types", { title: "List component types", description: "Component types that can be set on scene instances, with each field's kind, range, unit and which object variants accept it.", inputSchema: {}, annotations: { readOnlyHint: true } },
    async () => run(async ({ project }) => json({ types: await componentRegistry(project) })));
  edit("set_component", "Set a component", "Add or change a registry component (see list_component_types) on one scene instance. fields: only the fields to change (null resets one to the asset value), or null to remove the component override. enabled false switches the component off (an off light emits nothing).", {
    ref: refSchema, revision: revisionSchema, type: z.string().min(3).max(96), fields: z.record(z.string(), z.unknown()).nullable(), enabled: z.boolean().optional(), dryRun: dryRunSchema,
  }, (project, args) => setObjectComponent(project, args as { ref: string; revision: string; type: string; fields: Record<string, unknown> | null; enabled?: boolean; dryRun?: boolean }));
  const behaviour = (name: string, title: string, description: string, inputSchema: z.ZodRawShape, action: (project: ProjectSnapshot, index: ProjectIndex, args: Record<string, unknown>) => Promise<unknown>, readOnly = false) =>
    server.registerTool(name, { title, description, inputSchema, annotations: readOnly ? { readOnlyHint: true } : { readOnlyHint: false, destructiveHint: false, idempotentHint: false } }, async (args: Record<string, unknown>) => run(async ({ project, index }) => json(await action(project, index, args))));
  const fieldsSchema = z.record(z.string(), z.unknown()).describe("Field values by name; refs allowed for brains, profiles and objects");
  const uiId = z.string().regex(/^[A-Za-z0-9_-]{1,64}$/);
  const uiName = z.string().trim().min(1).max(64);
  const canvasSchema = z.tuple([z.number().int().min(1).max(4096), z.number().int().min(1).max(4096)]);
  const uiFields = z.record(z.string(), z.unknown()).describe("Widget fields from list_ui_widgets; null clears an override. id/type/parent are node properties, not fields.");
  const uiNode = z.object({ id: uiId.optional(), type: z.string().min(1).max(64), parent: uiId.nullable().optional(), fields: uiFields.optional() }).strict();
  const uiOperation = z.discriminatedUnion("op", [
    uiNode.extend({ op: z.literal("add") }),
    z.object({ op: z.literal("update"), id: uiId, fields: uiFields }).strict(),
    z.object({ op: z.literal("remove"), id: uiId }).strict(),
    z.object({ op: z.literal("move"), id: uiId, parent: uiId }).strict(),
    z.object({ op: z.literal("reorder"), id: uiId, by: z.union([z.literal(-1), z.literal(1)]) }).strict(),
  ]);
  behaviour("list_ui_widgets", "List UI widget types", "Native widget catalogue with container rules, field kinds/defaults/ranges, anchors and categories. UI-1 authoring only.", {}, async project => uiCatalogue(project), true);
  behaviour("create_ui", "Create a UI asset", "Create a UI from nodes listed parent-first. Exactly one root; only panels hold children. Give nodes IDs so children can name their parent. Validated by Rust, journalled and undoable; dryRun writes nothing.", {
    name: uiName, description: z.string().max(500).optional(), canvas: canvasSchema, nodes: z.array(uiNode).min(1).max(512), dryRun: dryRunSchema,
  }, (project, _index, args) => createUi(project, args as Parameters<typeof createUi>[1]));
  behaviour("edit_ui", "Edit a UI by widget ID", "Change name/description/canvas or add, update, remove (subtree), move and reorder widgets. Root cannot be removed/moved. Fields null clears overrides. Needs the revision from describe. Rust-validated, undoable, supports dryRun.", {
    ref: refSchema, revision: z.string().regex(/^[0-9a-f]{64}$/), name: uiName.optional(), description: z.string().max(500).optional(), canvas: canvasSchema.optional(), operations: z.array(uiOperation).max(64).optional(), dryRun: dryRunSchema,
  }, (project, index, args) => editUi(project, index, args as Parameters<typeof editUi>[2]));
  behaviour("preview_ui", "Inspect a UI layout", "Read paged widget rectangles in canvas pixels and action labels after Rust validation. Uses the designer's parity-tested layout; does not render or dispatch actions in game.", {
    ref: refSchema, offset: z.number().int().min(0).optional(), limit: z.number().int().min(1).max(200).optional(),
  }, (project, index, args) => previewUi(project, index, args as Parameters<typeof previewUi>[2]), true);
  const shaderOperation = z.discriminatedUnion("op", [
    z.object({ op: z.literal("add_node"), id: z.string().regex(/^[A-Za-z0-9_-]{1,64}$/).optional(), type: z.enum(shaderOps), inputs: z.array(z.string().max(64)).max(3).optional().describe("Source node IDs in input order; or connect later"), value: z.number().optional(), parameter: z.string().max(64).optional() }).strict(),
    z.object({ op: z.literal("update_node"), id: z.string().max(64), type: z.enum(shaderOps).optional(), value: z.number().optional(), parameter: z.string().max(64).optional() }).strict(),
    z.object({ op: z.literal("remove_node"), id: z.string().max(64) }).strict(),
    z.object({ op: z.literal("connect"), from: z.string().max(64), to: z.string().max(64).optional(), input: z.union([z.number().int().min(0).max(2), z.string().max(16)]).optional().describe("Input name (A, B, X, Low, High, Radians) or 0-based index"), output: z.enum(SHADER_OUTPUTS).optional() }).strict(),
    z.object({ op: z.literal("set_parameter"), id: z.string().regex(/^[A-Za-z0-9_-]{1,64}$/), name: z.string().min(1).max(64).optional(), default: z.number().optional(), min: z.number().optional(), max: z.number().optional() }).strict(),
    z.object({ op: z.literal("remove_parameter"), id: z.string().max(64) }).strict(),
  ]);
  behaviour("list_shader_nodes", "List shader node types", "Shader node types with their named inputs, the surface outputs, limits and templates.", {}, async () => shaderCatalogue(), true);
  behaviour("create_shader", "Create a shader", "Create a shader asset from a template (force-field, water, dissolve) or a plain white surface, then apply operations in order. Validated by the engine; journalled and undoable; dryRun writes nothing. Returns its asset ref.", {
    name: z.string().trim().min(1).max(64), template: z.enum(["force-field", "water", "dissolve"]).optional(), transparent: z.boolean().optional(), operations: z.array(shaderOperation).max(64).optional(), dryRun: dryRunSchema,
  }, (project, _index, args) => createShader(project, args as { name: string; template?: "force-field" | "water" | "dissolve"; transparent?: boolean; operations?: ShaderOperation[]; dryRun?: boolean }));
  behaviour("edit_shader", "Edit a shader by node ID", "Rename it, set transparent, or apply operations: add_node {type, id?, inputs?, value?, parameter?}, update_node {id, type?, value?, parameter?}, remove_node {id}, connect {from, to, input} or {from, output}, set_parameter {id, name?, default?, min?, max?} (creates it if new), remove_parameter {id}. Needs the revision from describe. Every input and output must be connected afterwards. Validated by the engine against every scene; undoable; supports dryRun.", {
    ref: refSchema, revision: z.string().regex(/^[0-9a-f]{64}$/), name: z.string().trim().min(1).max(64).optional(), transparent: z.boolean().optional(), operations: z.array(shaderOperation).max(64).optional(), dryRun: dryRunSchema,
  }, (project, index, args) => editShader(project, index, args as { ref: string; revision: string; name?: string; transparent?: boolean; operations?: ShaderOperation[]; dryRun?: boolean }));
  behaviour("list_brain_nodes", "List brain node types", "Every node type a brain can use, by heading, with role (composite, decorator, steering, reference), status (planned types cannot be used) and fields.", {}, async project => (await behaviourCatalogues(project)).brainNodes, true);
  behaviour("list_locomotion_types", "List locomotion types", "Every locomotion type (slide, wheeled, drone, fixed wing, ballistic, rocket, and planned ones) with manoeuvres and tuning fields.", {}, async project => (await behaviourCatalogues(project)).locomotionTypes, true);
  const nodeSchema = z.object({ id: z.string().regex(/^[A-Za-z0-9_.-]{1,64}$/).optional(), type: z.string().min(1).max(64), parent: z.string().max(64).nullable().optional(), fields: fieldsSchema.optional() });
  behaviour("create_brain", "Create a brain", "Create a project brain asset from nodes listed parent-first (the first node without a parent is the root). Validated by the engine before it is written. Returns its asset ref.", {
    name: z.string().min(1).max(64), description: z.string().max(500).optional(), nodes: z.array(nodeSchema).min(1).max(256), dryRun: dryRunSchema,
  }, (project, index, args) => createBrain(project, index, args as Parameters<typeof createBrain>[2]));
  const operationSchema = z.object({ op: z.enum(["add", "update", "remove", "move", "reorder"]), id: z.string().max(64).optional(), type: z.string().max(64).optional(), parent: z.string().max(64).nullable().optional(), fields: fieldsSchema.optional(), by: z.union([z.literal(-1), z.literal(1)]).optional() });
  behaviour("edit_brain", "Edit a brain", "Change a brain by node id: add {type, parent, id?, fields}, update {id, fields (null resets a field)}, remove {id} (and its subtree), move {id, parent}, reorder {id, by: -1|1}; optionally rename or re-describe it. Needs the revision from describe. Validated before writing; undoable.", {
    ref: refSchema, revision: z.string().regex(/^[0-9a-f]{64}$/), name: z.string().min(1).max(64).optional(), description: z.string().max(500).optional(), operations: z.array(operationSchema).max(64).optional(), dryRun: dryRunSchema,
  }, (project, index, args) => editBrain(project, index, args as { ref: string; revision: string; name?: string; description?: string; operations?: BrainOperation[]; dryRun?: boolean }));
  behaviour("preview_brain", "Preview a brain", "Run a saved brain in the engine on a flat arena (agents in one group, targets circling in another, optional obstacles) and summarise the motion: centroid, speed, spread, distance to targets and closest pair over time.", {
    ref: refSchema, agents: z.number().int().min(1).max(256).optional(), targets: z.number().int().min(0).max(32).optional(), obstacles: z.number().int().min(0).max(64).optional(),
    target_speed: z.number().min(0).max(20).optional(), seconds: z.number().min(1).max(30).optional(), agent_group: z.string().max(64).optional(), target_group: z.string().max(64).optional(), max_speed: z.number().min(0.1).max(50).optional(), seed: z.number().int().min(1).optional(),
  }, (project, index, args) => previewBrainSummary(project, index, args as Parameters<typeof previewBrainSummary>[2]), true);
  behaviour("generate_audio", "Generate an audio clip", "PAID: one fal request per call. Ask the user before calling, then pass confirm_paid: true. Generates a sound effect, music or speech (text-to-speech), waits up to about 2.5 minutes, and saves it as a project clip (asset kind audio). mode sound: text describes the sound, duration 0.5-22 s, loop for a seamless loop. mode music: text describes the music, duration 5-300 s, instrumental. mode speech: text is the line to speak, voice is a preset name (ElevenLabs: Rachel, Aria, Roger, Sarah, George, Charlie …). Returns the clip ref for emergent.audio_source.", {
    mode: z.enum(["sound", "music", "speech"]), text: z.string().min(3).max(2000), name: z.string().min(1).max(64), duration: z.number().min(0.5).max(300).optional(), voice: z.string().max(64).optional(), loop: z.boolean().optional(), instrumental: z.boolean().optional(), model: z.string().max(180).optional(), confirm_paid: z.literal(true),
  }, (project, _index, args) => generateAudioClip(project, args as Parameters<typeof generateAudioClip>[1]));
  behaviour("write_locomotion", "Create or change a locomotion profile", "Without ref: create a profile (name and type required). With ref and revision from describe: change its name, description, type (fields the new type lacks are dropped) or fields (null resets one). Validated by the engine; undoable.", {
    ref: refSchema.optional(), revision: z.string().regex(/^[0-9a-f]{64}$/).optional(), name: z.string().min(1).max(64).optional(), description: z.string().max(500).optional(), type: z.string().max(64).optional(), fields: fieldsSchema.optional(), dryRun: dryRunSchema,
  }, (project, index, args) => writeLocomotion(project, index, args as Parameters<typeof writeLocomotion>[2]));
  behaviour("preview_locomotion", "Preview a locomotion profile", "Drive a saved profile along a course in the engine (waypoints, orbit, stop, launch, circuit; projectiles default to launch, physical aircraft to circuit: take off, fly the pattern and land on a 900 m runway; up to 300 s for aircraft, 60 s otherwise) and summarise, including the flight phases and touchdown point for physical aircraft: distance, top speed, height range, steer/bank/pitch extremes, braking and the final state.", {
    ref: refSchema, course: z.enum(["waypoints", "orbit", "stop", "launch", "circuit"]).optional(), seconds: z.number().min(1).max(300).optional(),
  }, (project, index, args) => previewLocomotionSummary(project, index, args as { ref: string; course?: string; seconds?: number }), true);
  edit("set_environment", "Set scene environment", "Change a scene's ambient light, background or skybox. Fields: ambient_color [r,g,b], ambient_intensity, background_color [r,g,b], skybox (an image-library skybox path or null), skybox_rotation (degrees), skybox_intensity.", {
    scene: refSchema, revision: revisionSchema, settings: z.record(z.string(), z.unknown()), dryRun: dryRunSchema,
  }, (project, args) => setEnvironment(project, args as { scene: string; revision: string; settings: unknown; dryRun?: boolean }));
  edit("set_wind", "Set scene wind", "Set the scene's uniform wind: speed in m/s and the compass bearing it blows FROM (0 = from -Z/north, 90 = from +X/east). Physical aircraft fly through it (crabbing in crosswinds, shorter rolls into headwinds) and towers land into it. wind null removes it (calm).", {
    scene: refSchema, revision: revisionSchema, wind: z.object({ speed: z.number().min(0).max(100), from: z.number().min(0).max(360) }).nullable(), dryRun: dryRunSchema,
  }, (project, args) => setWind(project, args as { scene: string; revision: string; wind: unknown; dryRun?: boolean }));
  edit("undo", "Undo a change", "Revert the most recent MCP change, or the given changeId. Refused if any of its files changed since.", { changeId: z.string().uuid().optional(), dryRun: dryRunSchema },
    (project, args) => undoChange(project, args as { changeId?: string; dryRun?: boolean }), true);
  edit("assign_ids", "Assign durable IDs", "Give a durable ID to every file reported as missing-id (adds an id to TOML definitions, or a media.asset.json descriptor for loose media). Use dryRun first.", { dryRun: dryRunSchema },
    (project, args) => assignIds(project, args as { dryRun?: boolean }));
  server.registerTool("history", { title: "Change history", description: "Recent MCP changes with changeIds, newest first.", inputSchema: { limit: z.number().int().min(1).max(100).optional() }, annotations: { readOnlyHint: true } },
    async ({ limit }) => run(async ({ project }) => json({ changes: await changeHistory(project, limit) })));

  server.registerResource("project", "emergent://project", { title: "Project overview", mimeType: "application/json" },
    async uri => { const { project, index } = await context(); return { contents: [{ uri: uri.href, mimeType: "application/json", text: JSON.stringify(overview(project, index), null, 2) }] }; });

  server.registerResource("guide", "emergent://guide", { title: "Project authoring guide (AGENTS.md)", mimeType: "text/markdown" },
    async uri => { const project = await readProject(); return { contents: [{ uri: uri.href, mimeType: "text/markdown", text: await readFile(path.join(project.root, "AGENTS.md"), "utf8").catch(() => "This project has no AGENTS.md.") }] }; });

  server.registerResource("capabilities", "emergent://capabilities", { title: "Engine capability catalogue", description: "available / preview / planned engine capabilities; planned entries are not callable APIs", mimeType: "application/toml" },
    async uri => { const project = await readProject(); const root = await foundationRoot(project); return { contents: [{ uri: uri.href, mimeType: "application/toml", text: await readFile(path.join(root, "docs/capabilities/engine-capabilities.toml"), "utf8") }] }; });

  server.registerResource("behaviour-catalogue", "emergent://behaviour", { title: "Brain node and locomotion type catalogues", mimeType: "application/json" },
    async uri => { const project = await readProject(); return { contents: [{ uri: uri.href, mimeType: "application/json", text: JSON.stringify(await behaviourCatalogues(project), null, 2) }] }; });

  server.registerResource("scene", new ResourceTemplate("emergent://scene/{id}", {
    list: async () => { const { index } = await context(); return { resources: index.scenes.map(scene => ({ uri: `emergent://scene/${encodeURIComponent(scene.id)}`, name: scene.name, mimeType: "application/json" })) }; },
  }), { title: "Scene hierarchy", mimeType: "application/json" },
  async (uri, { id }) => { const { project, index } = await context(); const entry = resolveRef(index, `scene:${decodeURIComponent(String(id))}`); return { contents: [{ uri: uri.href, mimeType: "application/json", text: JSON.stringify(await describe(project, index, entry, false), null, 2) }] }; });

  return server;
}

type ToolResult = { content: ({ type: "text"; text: string } | { type: "image"; data: string; mimeType: string })[]; isError?: boolean };
async function runWithContext(action: (state: { project: ProjectSnapshot; index: ProjectIndex }) => Promise<ToolResult>, context: () => Promise<{ project: ProjectSnapshot; index: ProjectIndex }>): Promise<ToolResult> {
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
    if (definition && entry.kind === "brain") return { ...summary(entry), path: entry.path, ...describeBrain(definition), usedBy: brainUsers(index, project, entry.path) };
    if (definition && entry.kind === "locomotion") return { ...summary(entry), path: entry.path, ...describeLocomotion(definition), usedBy: brainUsers(index, project, entry.path) };
    if (definition && entry.kind === "shader") return { ...summary(entry), path: entry.path, ...describeShader(project, entry.path, definition) };
    if (definition && entry.kind === "ui") return { ...summary(entry), path: entry.path, ...describeUi(definition), usedBy: users };
    if (definition && entry.kind === "model") return { ...summary(entry), path: entry.path, files: entry.files, revision: entry.revision, usedBy: users, model: await inspectModel(project, definition) };
    return { ...summary(entry), path: entry.path, files: entry.files, revision: entry.revision, usedBy: users, definition: definition && definition.length <= 16384 ? definition : definition ? `${definition.slice(0, 16384)}\n… truncated` : null };
  }
  return describeInstance(project, index, entry, world);
}

/** The engine's view of a model asset's GLB: size, skeleton joints and clip names (for emergent.animator). */
async function inspectModel(project: ProjectSnapshot, definition: string) {
  try {
    const glb = (parse(definition) as { model?: unknown }).model;
    if (typeof glb !== "string") return { error: "Model asset names no .glb." };
    return parse(await gameTool(project, ["--inspect-model", glb]));
  } catch (error) { return { error: error instanceof Error ? error.message.slice(0, 2000) : "Model inspection failed." }; }
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
    components: (instance.components ?? []).map(component => ({ type: component.type, enabled: component.enabled, fields: refsFor(index, component.fields) })),
    flags: { renderable: instance.renderable, terrain: instance.terrain, raymarched: instance.raymarched, structure: instance.structure, particles: instance.particles },
  };
}

/** Asset paths in component fields shown as refs alongside the stored value. */
function refsFor(index: ProjectIndex, fields: Record<string, unknown>) {
  const ref = (value: unknown): unknown => typeof value === "string" ? index.assets.find(asset => asset.path === value)?.ref ?? value : Array.isArray(value) ? value.map(ref) : value;
  return Object.fromEntries(Object.entries(fields).map(([name, value]) => [name, ref(value)]));
}

/** Instances whose components name this brain or profile, in any scene. */
function brainUsers(index: ProjectIndex, project: ProjectSnapshot, assetPath: string) {
  const users: { scene: string; instance: string; component: string }[] = [];
  for (const scene of index.scenes) {
    let entities: unknown[] = [];
    try { entities = (parse(project.files.find(file => file.path === scene.path)?.content ?? "") as { entities?: unknown[] }).entities ?? []; } catch { continue; }
    for (const entity of entities as { id?: unknown; components?: { type?: unknown; [field: string]: unknown }[] }[]) {
      for (const component of entity.components ?? []) {
        // Lists hold paths (stack) or tables of them (brain modes).
        const values = Object.values(component).flatMap(value => Array.isArray(value) ? value.flatMap(item => item && typeof item === "object" ? Object.values(item) : [item]) : [value]);
        if (values.includes(assetPath) && typeof entity.id === "string") users.push({ scene: scene.ref, instance: `instance:${scene.id}/${entity.id}`, component: String(component.type) });
      }
    }
  }
  return users;
}
