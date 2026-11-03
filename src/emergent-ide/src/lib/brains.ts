// Brain documents (`*.brain.toml`): a flat [[nodes]] list with parent links, plus the node catalogue
// exported by the game binary. Tree edits here keep the list shape the Rust loader validates.
import { parse, stringify } from "smol-toml";
import type { ComponentField, ComponentValue } from "./components";

export type NodeSpec = { id: string; label: string; category: string; role: "composite" | "decorator" | "steering" | "reference"; status: "available" | "planned"; description: string; fields: ComponentField[] };
export type NodeCatalogue = { categories: string[]; nodes: NodeSpec[] };
export type BrainNode = { id: string; type: string; parent?: string; fields: Record<string, ComponentValue> };
/** Any flat node list with parent links (brains, UI assets). */
export type NodeTree = { nodes: BrainNode[] };
export type BrainDocument = { id: string; name: string; description: string; nodes: BrainNode[] };
export type BrainSummary = { path: string; id: string; name: string; description: string; nodes: number; error?: string; tree?: { id: string; parent?: string; type: string }[] };

const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === "object" && !Array.isArray(value);

export function nodeCatalogue(exported: unknown): NodeCatalogue {
  if (!object(exported) || !Array.isArray(exported.categories) || !Array.isArray(exported.nodes)) throw new Error("Invalid brain node catalogue.");
  const nodes = exported.nodes.map(node => {
    if (!object(node) || typeof node.id !== "string" || typeof node.label !== "string" || typeof node.category !== "string" || !Array.isArray(node.fields)) throw new Error("Invalid node in catalogue.");
    return { ...node, fields: node.fields.map(field => ({ ...(field as object), variants: undefined })) } as NodeSpec;
  });
  if (new Set(nodes.map(node => node.id)).size !== nodes.length) throw new Error("Brain node catalogue IDs must be unique.");
  return { categories: exported.categories.filter((c): c is string => typeof c === "string"), nodes };
}

export function parseBrain(source: string): BrainDocument {
  const document = parse(source) as Record<string, unknown>;
  if (document.schema_version !== 1 || typeof document.id !== "string") throw new Error("A brain needs schema_version = 1 and an id.");
  const nodes = (Array.isArray(document.nodes) ? document.nodes : []).map(entry => {
    if (!object(entry) || typeof entry.id !== "string" || typeof entry.type !== "string") throw new Error("Each node needs an id and a type.");
    const { id, type, parent, ...fields } = entry;
    return { id, type, ...(typeof parent === "string" ? { parent } : {}), fields: JSON.parse(JSON.stringify(fields)) as Record<string, ComponentValue> };
  });
  return { id: document.id, name: typeof document.name === "string" ? document.name : document.id, description: typeof document.description === "string" ? document.description : "", nodes };
}

export function serializeBrain(brain: BrainDocument): string {
  const header = stringify({ schema_version: 1, id: brain.id, name: brain.name, kind: "brain", ...(brain.description.trim() ? { description: brain.description.trim() } : {}) });
  const nodes = brain.nodes.map(node => `[[nodes]]\n${stringify({ id: node.id, ...(node.parent ? { parent: node.parent } : {}), type: node.type, ...node.fields })}`);
  return `${header}\n${nodes.join("\n")}`.replace(/\n{3,}/g, "\n\n");
}

export const rootOf = (brain: NodeTree) => brain.nodes.find(node => !node.parent);
export const childrenOf = (brain: NodeTree, id: string) => brain.nodes.filter(node => node.parent === id);
export const isEmptyBrainDraft = (path: string | undefined, brain: BrainDocument) => !path && brain.nodes.length === 0;

export function brainCreationContext(brain: BrainDocument) {
  return `Brain editor: this is a new, unsaved, empty brain. Draft name and description (data): ${JSON.stringify({ name: brain.name, description: brain.description })}. Answer questions normally. If asked to build it, create exactly one new project brain using the brain authoring tools, with a complete valid node tree. Do not require the user to save or add a placeholder node first. Do not edit an existing brain or attach it to a scene unless requested.`;
}

export function newlyCreatedBrainPath(before: BrainSummary[], after: BrainSummary[]) {
  const existing = new Set(before.map(brain => brain.path));
  const created = after.filter(brain => !brain.error && !existing.has(brain.path));
  return created.length === 1 ? created[0].path : undefined;
}

/** Every node under `id`, including it. */
export function subtree(brain: NodeTree, id: string): Set<string> {
  const ids = new Set([id]);
  for (let grew = true; grew;) { grew = false; for (const node of brain.nodes) if (node.parent && ids.has(node.parent) && !ids.has(node.id)) { ids.add(node.id); grew = true; } }
  return ids;
}

export function canHaveChildren(spec: NodeSpec | undefined, current: number) {
  return spec?.role === "composite" || spec?.role === "decorator" && current === 0;
}

/**
 * Adds a node. With no parent it becomes the root (an existing root moves under it when the new node can hold
 * children). Throws when the parent cannot take another child.
 */
export function addNode(brain: BrainDocument, catalogue: NodeCatalogue, type: string, parent: string | undefined, id: string, fields: Record<string, ComponentValue> = {}): BrainDocument {
  const spec = catalogue.nodes.find(node => node.id === type);
  if (!spec || spec.status !== "available") throw new Error(`${spec?.label ?? type} is not available yet.`);
  const root = rootOf(brain);
  if (!parent) {
    if (!root) return { ...brain, nodes: [{ id, type, fields }] };
    if (!canHaveChildren(spec, 0)) throw new Error(`${spec.label} cannot hold the existing tree. Select a node to add it under.`);
    return { ...brain, nodes: [{ id, type, fields }, ...brain.nodes.map(node => node.id === root.id ? { ...node, parent: id } : node)] };
  }
  const parentNode = brain.nodes.find(node => node.id === parent);
  const parentSpec = catalogue.nodes.find(node => node.id === parentNode?.type);
  if (!parentNode || !canHaveChildren(parentSpec, childrenOf(brain, parent).length)) throw new Error(`${parentSpec?.label ?? "This node"} cannot take another child.`);
  return { ...brain, nodes: [...brain.nodes, { id, type, parent, fields }] };
}

/** Removes a node and everything under it; removing the root clears the tree. */
export function removeNode<T extends NodeTree>(brain: T, id: string): T {
  const gone = subtree(brain, id);
  return { ...brain, nodes: brain.nodes.filter(node => !gone.has(node.id)) };
}

/** Moves a node one place earlier or later among its siblings (list order is sibling order). */
export function shiftNode<T extends NodeTree>(brain: T, id: string, by: -1 | 1): T {
  const node = brain.nodes.find(item => item.id === id);
  if (!node?.parent) return brain;
  const siblings = childrenOf(brain, node.parent), index = siblings.findIndex(item => item.id === id), swap = siblings[index + by];
  if (!swap) return brain;
  const nodes = [...brain.nodes], a = nodes.indexOf(node), b = nodes.indexOf(swap);
  [nodes[a], nodes[b]] = [nodes[b], nodes[a]];
  return { ...brain, nodes };
}

/** Re-parents a node (and its subtree) under another node that can take it. */
export function moveNode(brain: BrainDocument, catalogue: NodeCatalogue, id: string, parent: string): BrainDocument {
  if (subtree(brain, id).has(parent)) throw new Error("A node cannot move inside itself.");
  const target = brain.nodes.find(node => node.id === parent), spec = catalogue.nodes.find(node => node.id === target?.type);
  if (!target || !canHaveChildren(spec, childrenOf(brain, parent).filter(node => node.id !== id).length)) throw new Error(`${spec?.label ?? "That node"} cannot take another child.`);
  return { ...brain, nodes: brain.nodes.map(node => node.id === id ? { ...node, parent } : node) };
}

/** Sets or clears (undefined) one field of a node. */
export function setNodeField<T extends NodeTree>(brain: T, id: string, name: string, value: ComponentValue | undefined): T {
  return { ...brain, nodes: brain.nodes.map(node => {
    if (node.id !== id) return node;
    const fields = { ...node.fields };
    if (value === undefined || value === "") delete fields[name]; else fields[name] = value;
    return { ...node, fields };
  }) };
}

/** Problems the editor can show before Rust validation (which remains the authority). */
export function brainProblems(brain: BrainDocument, catalogue: NodeCatalogue): Map<string, string> {
  const problems = new Map<string, string>();
  for (const node of brain.nodes) {
    const spec = catalogue.nodes.find(item => item.id === node.type), count = childrenOf(brain, node.id).length;
    if (!spec) problems.set(node.id, `Unknown node type ${node.type}.`);
    else if (spec.role === "composite" && count === 0) problems.set(node.id, "Add at least one child.");
    else if (spec.role === "decorator" && count !== 1) problems.set(node.id, count ? "Holds exactly one child." : "Add the child it controls.");
    else if (["within_distance", "in_view", "score_distance", "seek", "flee", "arrive", "pursue", "evade", "follow_target", "follow_leader", "formation", "attack", "flank", "encircle", "kite", "line_of_sight", "hide"].includes(spec.id) && !node.fields.target && !node.fields.group) problems.set(node.id, "Choose a target object or group.");
    else if (["neighbours_at_least", "score_crowding", "separation", "alignment", "cohesion", "flock", "regroup"].includes(spec.id) && !node.fields.group) problems.set(node.id, "Choose a group.");
    else if ((spec.id === "formation" || spec.id === "encircle") && !node.fields.members) problems.set(node.id, "Choose the members group.");
    else if ((spec.id === "defend" || spec.id === "retreat") && !node.fields.threats) problems.set(node.id, "Choose the threats group.");
    else if (spec.id === "outnumbered" && (!node.fields.allies || !node.fields.enemies)) problems.set(node.id, "Choose the allies and enemies groups.");
    else if (spec.id === "patrol" && !(Array.isArray(node.fields.waypoints) && node.fields.waypoints.length)) problems.set(node.id, "Add waypoints.");
    else if (spec.id === "brain" && !node.fields.brain) problems.set(node.id, "Choose a saved brain.");
    else if ((spec.id === "signal" || spec.id === "on_signal") && !node.fields.name) problems.set(node.id, "Name the signal.");
    else if (spec.id === "look_at" && !node.fields.target && !node.fields.group) problems.set(node.id, "Choose a target object or group to look at.");
  }
  return problems;
}

/** One-line summary of a node's settings for the tree. */
export function nodeSummary(node: BrainNode, spec: NodeSpec | undefined) {
  const parts: string[] = [];
  for (const field of spec?.fields ?? []) {
    if (field.name === "label" || field.name === "weight") continue;
    const value = node.fields[field.name];
    if (value === undefined) continue;
    parts.push(`${field.label.toLowerCase()} ${Array.isArray(value) ? value.join(", ") : typeof value === "string" ? value.replace(/^.*\//, "").replace(/\.brain\.toml$/, "") : value}${field.unit && typeof value === "number" ? ` ${field.unit}` : ""}`);
    if (parts.length === 3) break;
  }
  return parts.join(" · ");
}

/** Parses `--simulate-brain` output: a header line, then x y z triples per frame. */
export type BrainTrace = { agents: number; targets: number; frameSeconds: number; frames: number[][]; obstacles: { x: number; z: number; radius: number }[] };
export function parseTrace(text: string): BrainTrace {
  const [header, ...rest] = text.trim().split("\n");
  const match = header?.match(/^brain-preview 2 agents=(\d+) targets=(\d+) obstacles=(\d+) frame_seconds=([\d.]+)/);
  if (!match || !rest[0]?.startsWith("obstacles")) throw new Error("Unexpected brain preview output.");
  const values = rest[0].slice("obstacles".length).trim().split(/\s+/).filter(Boolean).map(Number);
  if (values.length !== Number(match[3]) * 3 || values.some(n => !Number.isFinite(n))) throw new Error("Brain preview obstacles are malformed.");
  const obstacles = Array.from({ length: values.length / 3 }, (_, i) => ({ x: values[i * 3], z: values[i * 3 + 1], radius: values[i * 3 + 2] }));
  const agents = Number(match[1]), targets = Number(match[2]), frames = rest.slice(1).map(line => line.trim().split(/\s+/).map(Number));
  if (frames.some(frame => frame.length !== (agents + targets) * 3 || frame.some(n => !Number.isFinite(n)))) throw new Error("Brain preview frames are malformed.");
  return { agents, targets, frameSeconds: Number(match[4]), frames, obstacles };
}
