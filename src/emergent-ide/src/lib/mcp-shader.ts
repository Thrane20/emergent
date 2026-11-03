import "server-only";
// MCP shader authoring: the node graph the shader editor shows, edited by node ID with the editor's save path
// (native candidate validation of every scene, revision guard, dry run, journal for history/undo, IDE refresh).
import { createHash } from "node:crypto";
import type { ProjectSnapshot } from "./project";
import { RefError, resolveRef, type ProjectIndex } from "./project-index";
import { saveShader } from "./shader-assets";
import { layoutShaderGraph, orderShaderNodes, shaderArity, shaderGraph, shaderNodeInfo, shaderOps, shaderOutputSockets, shaderTemplate, type ShaderGraph, type ShaderOp } from "./shader-graph";
import { parse } from "smol-toml";

export const SHADER_OUTPUTS = ["red", "green", "blue", "emission", "opacity", "displacement"] as const;
type OutputName = (typeof SHADER_OUTPUTS)[number];
export type ShaderOperation =
  | { op: "add_node"; id?: string; type: ShaderOp; inputs?: string[]; value?: number; parameter?: string }
  | { op: "update_node"; id: string; type?: ShaderOp; value?: number; parameter?: string }
  | { op: "remove_node"; id: string }
  | { op: "connect"; from: string; to?: string; input?: number | string; output?: OutputName }
  | { op: "set_parameter"; id: string; name?: string; default?: number; min?: number; max?: number }
  | { op: "remove_parameter"; id: string };

export function shaderCatalogue() {
  return {
    limits: { nodes: 32, parameters: 16, values: "finite, within ±10000" },
    nodes: shaderOps.map(op => ({ type: op, ...shaderNodeInfo[op], fields: op === "constant" ? ["value"] : op === "parameter" ? ["parameter (an exposed parameter id)"] : [] })),
    outputs: shaderOutputSockets.map(([label]) => ({ name: label.toLowerCase(), description: { Red: "colour red 0..1", Green: "colour green 0..1", Blue: "colour blue 0..1", Emission: "glow multiplier 0..100", Opacity: "0..1 (needs transparent: true to blend)", Displacement: "metres along the mesh normal; must not depend on Fresnel" }[label] })),
    templates: ["force-field", "water", "dissolve"],
    notes: "Every value is a scalar. Each node has one output; connect it to any number of inputs or surface outputs. Position is object-space metres, UV 0..1, time seconds. No textures, scene depth or custom code. Apply a shader through a material: set_material { shader: <shader asset ref>, cast_shadows: false, shader_parameters: { id: value } }.",
  };
}

const revision = (content: string) => createHash("sha256").update(content).digest("hex");
export function describeShader(project: ProjectSnapshot, path: string, content: string) {
  const graph = shaderGraph(parse(content));
  const usedBy = project.files.filter(file => file.path !== path && file.content?.includes(path)).map(file => file.path);
  return {
    revision: revision(content), name: graph.name, transparent: graph.transparent, parameters: graph.parameters,
    nodes: graph.nodes.map(({ id, op, inputs, value, parameter }) => ({
      id, type: op, ...(op === "constant" ? { value } : {}), ...(parameter ? { parameter } : {}),
      inputs: Object.fromEntries(inputs.map((source, i) => [shaderNodeInfo[op].inputs[i], source])),
    })),
    outputs: Object.fromEntries(SHADER_OUTPUTS.map((name, i) => [name, i < 3 ? graph.outputs.color[i] : graph.outputs[name as "emission" | "opacity" | "displacement"]])),
    usedBy,
  };
}

function inputIndex(type: ShaderOp, input: number | string | undefined) {
  const names = shaderNodeInfo[type].inputs;
  const index = typeof input === "number" ? input : input === undefined && names.length === 1 ? 0 : names.findIndex(name => name.toLowerCase() === String(input).toLowerCase());
  if (!Number.isInteger(index) || index < 0 || index >= names.length) throw new Error(`${type} inputs are ${names.join(", ") || "none"}; name one (or its 0-based index).`);
  return index;
}

/** Applies node-ID operations in order; the result still has to pass the full contract and native validation. */
export function applyShaderOperations(start: ShaderGraph, operations: ShaderOperation[]) {
  let g = structuredClone(start);
  const added: string[] = [];
  for (const [i, edit] of operations.entries()) {
    try {
      const node = (id: string) => { const found = g.nodes.find(n => n.id === id); if (!found) throw new Error(`No node ${id}.`); return found; };
      if (edit.op === "add_node") {
        if (!shaderOps.includes(edit.type)) throw new Error(`Unknown node type ${edit.type}.`);
        const id = edit.id ?? (() => { for (let n = 1; ; n++) { const id = `${edit.type.replace("_", "-")}-${n}`; if (!g.nodes.some(node => node.id === id)) return id; } })();
        if (g.nodes.some(n => n.id === id)) throw new Error(`Duplicate node ID ${id}.`);
        const inputs = edit.inputs ?? Array(shaderArity(edit.type)).fill("");
        if (inputs.length !== shaderArity(edit.type)) throw new Error(`${edit.type} takes ${shaderArity(edit.type)} inputs.`);
        g.nodes.push({ id, op: edit.type, inputs, value: edit.value ?? 0, ...(edit.type === "parameter" ? { parameter: edit.parameter ?? edit.id } : {}) });
        added.push(id);
      } else if (edit.op === "update_node") {
        const n = node(edit.id);
        if (edit.type && edit.type !== n.op) {
          if (!shaderOps.includes(edit.type)) throw new Error(`Unknown node type ${edit.type}.`);
          n.inputs = Array.from({ length: shaderArity(edit.type) }, (_, j) => n.inputs[j] ?? "");
          n.op = edit.type;
          if (edit.type !== "parameter") delete n.parameter;
        }
        if (edit.value !== undefined) n.value = edit.value;
        if (edit.parameter !== undefined) n.parameter = edit.parameter;
      } else if (edit.op === "remove_node") {
        node(edit.id);
        g.nodes = g.nodes.filter(n => n.id !== edit.id);
        const clear = (v: string) => v === edit.id ? "" : v;
        for (const n of g.nodes) n.inputs = n.inputs.map(clear);
        g.outputs = { ...g.outputs, color: g.outputs.color.map(clear), emission: clear(g.outputs.emission), opacity: clear(g.outputs.opacity), displacement: clear(g.outputs.displacement) };
      } else if (edit.op === "connect") {
        node(edit.from);
        if (edit.output !== undefined) {
          const at = SHADER_OUTPUTS.indexOf(edit.output);
          if (at < 0) throw new Error(`Surface outputs are ${SHADER_OUTPUTS.join(", ")}.`);
          if (at < 3) g.outputs.color[at] = edit.from; else g.outputs[edit.output as "emission" | "opacity" | "displacement"] = edit.from;
        } else {
          if (!edit.to) throw new Error("connect needs to (a node ID) and input, or output (a surface output).");
          const target = node(edit.to);
          if (target.id === edit.from) throw new Error("A node can't feed itself.");
          target.inputs[inputIndex(target.op, edit.input)] = edit.from;
          g.nodes = orderShaderNodes(g.nodes);
        }
      } else if (edit.op === "set_parameter") {
        const existing = g.parameters.find(p => p.id === edit.id);
        const next = { id: edit.id, name: edit.name ?? existing?.name ?? edit.id, default: edit.default ?? existing?.default ?? 0, min: edit.min ?? existing?.min ?? 0, max: edit.max ?? existing?.max ?? 1 };
        g.parameters = existing ? g.parameters.map(p => p.id === edit.id ? next : p) : [...g.parameters, next];
      } else if (edit.op === "remove_parameter") {
        if (g.nodes.some(n => n.parameter === edit.id)) throw new Error(`Parameter ${edit.id} is used by a parameter node; remove or change that node first.`);
        g.parameters = g.parameters.filter(p => p.id !== edit.id);
      }
    } catch (error) { throw new RefError(`Shader operation ${i + 1} (${edit.op}): ${error instanceof Error ? error.message : error}`); }
  }
  g = layoutShaderGraph({ ...g, nodes: orderShaderNodes(g.nodes) });
  const open = g.nodes.flatMap(n => n.inputs.flatMap((source, j) => g.nodes.some(m => m.id === source) ? [] : [`${n.id}.${shaderNodeInfo[n.op].inputs[j]}`]));
  const outputs = SHADER_OUTPUTS.filter((_, j) => !g.nodes.some(n => n.id === (j < 3 ? g.outputs.color[j] : g.outputs[SHADER_OUTPUTS[j] as "emission" | "opacity" | "displacement"])));
  if (open.length || outputs.length) throw new RefError(`Unconnected: ${[...open, ...outputs.map(o => `output ${o}`)].join(", ")}. Connect every input and surface output.`);
  try { return { graph: shaderGraph(g), added }; } catch (error) { throw new RefError(error instanceof Error ? error.message : String(error)); }
}

function targetShader(project: ProjectSnapshot, index: ProjectIndex, ref: string, expected: string) {
  const entry = resolveRef(index, ref);
  if (entry.type !== "asset" || entry.kind !== "shader") throw new RefError(`${ref} is not a shader asset. Use find with kind shader.`);
  const content = project.files.find(file => file.path === entry.path)?.content;
  if (!content) throw new RefError(`${ref} has no readable file.`);
  if (revision(content) !== expected) throw new RefError(`${ref} changed since you described it. Describe it again before editing.`);
  return { entry, graph: shaderGraph(parse(content)) };
}

async function publish(project: ProjectSnapshot, tool: string, summary: string, graph: ShaderGraph, dryRun: boolean | undefined, current?: { path: string; revision: string }) {
  const saved = await saveShader(project, { path: current?.path, revision: current?.revision, graph }, { tool, summary, dryRun }).catch(error => { throw new RefError(error instanceof Error ? error.message : String(error)); });
  const ref = `asset:${saved.graph.id}`;
  return {
    applied: saved.applied ?? false, dryRun: dryRun === true, changeId: saved.changeId ?? null, ref, revision: saved.revision, path: saved.path, diff: saved.diff ?? "",
    summary: dryRun ? `Would ${summary}. Validated by the engine against every scene; nothing written.` : saved.applied ? `${summary}.` : `${summary}: no change.`,
  };
}

export async function createShader(project: ProjectSnapshot, input: { name: string; template?: "force-field" | "water" | "dissolve"; transparent?: boolean; operations?: ShaderOperation[]; dryRun?: boolean }) {
  const base = shaderTemplate(input.template ?? "water");
  // Without a template, start from the smallest valid graph: plain white, opaque, no glow or displacement.
  const start: ShaderGraph = input.template ? base : { schema_version: 1, id: "new", name: input.name, transparent: false, parameters: [], nodes: [{ id: "zero", op: "constant", inputs: [], value: 0 }, { id: "one", op: "constant", inputs: [], value: 1 }], outputs: { color: ["one", "one", "one"], emission: "zero", opacity: "one", displacement: "zero" } };
  const { graph, added } = applyShaderOperations({ ...start, name: input.name.trim(), transparent: input.transparent ?? start.transparent }, input.operations ?? []);
  return { ...await publish(project, "create_shader", `create shader ${graph.name}`, graph, input.dryRun), nodes: added };
}

export async function editShader(project: ProjectSnapshot, index: ProjectIndex, input: { ref: string; revision: string; name?: string; transparent?: boolean; operations?: ShaderOperation[]; dryRun?: boolean }) {
  const { entry, graph: current } = targetShader(project, index, input.ref, input.revision);
  const { graph, added } = applyShaderOperations({ ...current, name: input.name?.trim() ?? current.name, transparent: input.transparent ?? current.transparent }, input.operations ?? []);
  return { ...await publish(project, "edit_shader", `edit shader ${graph.name} (${entry.ref})`, graph, input.dryRun, { path: entry.path, revision: input.revision }), nodes: added };
}
