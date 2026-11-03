import "server-only";
import { parse } from "smol-toml";
import { randomUUID } from "node:crypto";
import { buildProjectPrompt, chatStatementSubject, validateChatRequest } from "./ai-chat";
import { readProjectBinding, type ProjectSnapshot } from "./project";
import { projectCodeContext } from "./project-code-context";
import { projectScenes } from "./project-scenes";
import { applyChanges, checkCandidate, prepareChanges } from "./statement-files";
import { implementationResponse, statementResponse } from "./statement-ai";
import { statementsFor } from "./statements";
import { mcpChat } from "./mcp-chat";

const state = globalThis as typeof globalThis & { emergentStatementImplementation?: boolean };
export async function implementChat(project: ProjectSnapshot, input: ReturnType<typeof validateChatRequest>, respond?: typeof statementResponse) {
  if (input.projectId !== project.id) throw new Error("The chat request does not match the linked project.");
  if (state.emergentStatementImplementation) throw new Error("Another AI change is being implemented. Wait for it to finish.");
  state.emergentStatementImplementation = true;
  try {
    return respond
      ? await implementFileChanges(project, input, respond)
      : await mcpChat(project, input, fresh => implementFileChanges(fresh, input));
  } finally { state.emergentStatementImplementation = false; }
}

async function implementFileChanges(project: ProjectSnapshot, input: ReturnType<typeof validateChatRequest>, respond = statementResponse) {
    const scene = input.scenePath ?? project.entryScene;
    if (!projectScenes(project).some(file => file.path === scene)) throw new Error("Choose a scene from the linked project.");
    const subject = chatStatementSubject(project, input.scope);
    const statements = subject ? await statementsFor(project, subject) : [];
    const scoped = buildProjectPrompt(project, input.scope, input.message, input.history, statements);
    const context = await projectCodeContext(project, { subject: subject ?? `file:${scene}`, text: input.message });
    const newIds: string[] = Array.from({length:16},()=>randomUUID());
    const prompt = [scoped,
      "Return structured JSON with summary, reuse and files. Carry out edit requests now; do not merely suggest a snippet or say this chat is read-only. Answer questions in summary with files=[].",
      "Inspect the supplied project/foundation files and search results first. Reuse existing assets, behaviours and public APIs. Include actual inspected paths and symbols in reuse for changes; questions may use reuse=[]. Never invent an engine capability.",
      "Allowed replacements: project src/**/*.rs, tests/**/*.rs and asset-root TOML. No engine edits, manifests/dependencies, build scripts, deletion, shell commands or unrelated filesystem/network access. At most 8 files, 128 KiB each and 256 KiB total. Preserve unrelated content, comments, objects and behaviours. Return complete replacement files.",
      "A host-controlled isolated Cargo check and scene validation run before publication. Do not claim files were applied or checks passed; the host reports those outcomes.",
      "Keep durable authored IDs separate from display names and runtime SlotMap keys. For new assets/instances use supplied UUIDs as IDs, with name as a display label on entity assets. Names may repeat. Never rename an existing ID to resolve a display-name collision.",
      "For a new instance at the world centre, place a root instance (no parent) with translation [0,0,0]; parented translations are local. Use the scene currently selected below unless the request explicitly identifies another scene. Reuse a matching primitive asset when appropriate. Behaviours must take effect, not only be declared.",
      "Object behaviour is authored as components on scene instances. First reuse a built-in component type (emergent.character_controller, emergent.spin, emergent.follow, emergent.light; see foundation/src/component.rs) by adding [[entities.components]] with type and only the fields that differ from the defaults to the instance in its scene TOML. If none fits, add a project component type in src/components.rs: describe its fields in registry(), build its runtime in ProjectComponents::attach and advance it in fixed_update, then attach it in the scene with type = \"project.<name>\". Never hard-code instance IDs in game.rs.",
      "Brains (*.brain.toml under assets/brains) are steering trees: schema_version = 1, id, name, kind = \"brain\", description, then a flat [[nodes]] list with id, type, parent (absent only on the single root) and that node type's fields. Use only node types and fields from node_catalogue() in foundation/src/brain.rs (composite nodes need children, decorators exactly one child, steering nodes none; planned nodes are not allowed). Prefer group names over object IDs so a brain works in any scene; keep existing node IDs; give new nodes short new IDs. A brain is applied to an object with [[entities.components]] type = \"emergent.brain\" and brain = its path; give objects that brains should find an emergent.group component.",
      "UIs (*.ui.toml under assets/ui) are flat widget lists: schema_version = 1, id, name, kind = \"ui\", description, canvas = [w, h] px, then [[nodes]] with id, type, parent (absent only on the single root) and that widget's fields from widget_catalogue() in foundation/src/ui.rs. Panels and horizontal, vertical and grid layout containers hold children; layouts nest freely. Containers support padding, gap, align_main (start/center/end), align_cross (start/center/end/stretch), and grid columns (integer 1–64); w/h accept px or percentages. Grid children support integer col_span/row_span; w/h can be auto for intrinsic content size. Flow layouts manage child positions; x/y/w/h are lengths like 8px, 30% or 100%-16px; a panel with stack = true lists children top to bottom. Give controls an action label (dotted, e.g. graphics.apply) and keep existing node IDs.",
      "Audio clips (assets/audio/<id>/<name>.audio.toml beside a .wav/.mp3/.ogg/.flac file) are made in the Audio editor or imported, never written by hand: schema_version = 1, id, name, kind = \"audio\", description, source (file name in the same folder), category (sfx, music, voice or ui), gain_db, optional trim = [start, end] and loop = [start, end] seconds (loop measured from the trim start), fade_in, fade_out. Play one from an object with the emergent.audio_source component (clip = the .audio.toml path, mode = \"loop\" or \"one_shot\", autoplay, volume, pitch, spatial 0..1, min_distance, max_distance, rolloff, doppler); emergent.audio_listener (gain) marks who hears, otherwise the active camera listens. Use a Point for a sound at a fixed spot.",
      "Locomotion profiles (*.locomotion.toml under assets/locomotion) say how an object moves: schema_version = 1, id, name, kind = \"locomotion\", description, type (from type_catalogue() in foundation/src/locomotion.rs; planned types are not allowed) and that type's fields at the top level. Attach one with [[entities.components]] type = \"emergent.locomotion\" and profile = its path; the object's brain then supplies intent (velocity, target, brake, hover, boost, altitude) and the profile carries it out. Locomotion cannot be combined with a dynamic physics body, a character controller, follow or spin.",
      `ACTIVE SCENE ${scene}\nAVAILABLE NEW DURABLE IDS ${newIds.join(", ")}`,
      `SEARCH RESULTS\n${context.search.join("\n")}`,
      ...Array.from(context.files,([path,content])=>`FILE ${path}\n${content}`),
    ].join("\n\n");
    const proposal = await implementationResponse(prompt, context, respond);
    const changes = await prepareChanges(project, proposal, context);
    for (const file of changes.filter(file=>/\.(entity|scene)\.toml$/.test(file.path))) {
      const after = parse(file.content), before = file.before === null ? null : parse(file.before);
      if (before && after.id !== before.id) throw new Error(`Preserve the existing authored ID in ${file.path}. No files were applied.`);
      if (!before && (typeof after.id !== "string" || !newIds.includes(after.id))) throw new Error(`New assets must use a supplied unique ID: ${file.path}. No files were applied.`);
      if (!before && file.path.endsWith(".entity.toml") && (typeof after.name !== "string" || !after.name.trim())) throw new Error(`New entity assets need a display name: ${file.path}. No files were applied.`);
      if (file.path.endsWith(".scene.toml") && Array.isArray(after.entities)) {
        const oldIds = new Set(Array.isArray(before?.entities) ? before.entities.map(entry=>entry && typeof entry==="object" && "id" in entry ? entry.id : undefined) : []);
        for (const entry of after.entities) if (entry && typeof entry==="object" && "id" in entry && !oldIds.has(entry.id) && (typeof entry.id!=="string" || !newIds.includes(entry.id))) throw new Error(`New scene instances must use supplied unique IDs: ${file.path}. No files were applied.`);
      }
    }
    if (changes.length) {
      const scenes = [...new Set([...projectScenes(project).map(file=>file.path), ...changes.filter(file=>file.path.endsWith(".scene.toml")).map(file=>file.path)])];
      if (scenes.length > 127) throw new Error("Chat validation supports at most 127 authored scenes.");
      await checkCandidate(project, changes, context, scenes);
      const binding = await readProjectBinding();
      if (binding.id !== project.id || binding.root !== project.root) throw new Error("The linked project changed during generation. No files were applied.");
      await prepareChanges(project, proposal, context);
      await applyChanges(project, changes);
    }
    return { text: changes.length ? `${proposal.summary}\n\nApplied ${changes.length} file change${changes.length===1?"":"s"}; Cargo and scene validation passed.` : proposal.summary, files: changes.map(file=>file.path), applied: changes.length > 0 };
}
