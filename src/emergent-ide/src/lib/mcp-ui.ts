import "server-only";
import { randomUUID } from "node:crypto";
import { removeNode, setNodeField, shiftNode } from "./brains";
import type { ComponentValue } from "./components";
import { publishAsset, validateAsset } from "./mcp-behaviour";
import type { ProjectSnapshot } from "./project";
import { RefError, resolveRef, type ProjectIndex } from "./project-index";
import { UI_DIRECTORY, readUi, uiCatalogue, uiRevision } from "./ui-assets";
import { addWidget, moveWidget, parseUi, serializeUi, uiActions, uiLayout, type UiDocument } from "./ui";

export type UiNodeInput = { id?: string; type: string; parent?: string | null; fields?: Record<string, unknown> };
export type UiOperation =
  | ({ op: "add" } & UiNodeInput)
  | { op: "update"; id: string; fields: Record<string, unknown> }
  | { op: "remove"; id: string }
  | { op: "move"; id: string; parent: string }
  | { op: "reorder"; id: string; by: -1 | 1 };

function targetUi(project: ProjectSnapshot, index: ProjectIndex, ref: string) {
  const entry = resolveRef(index, ref);
  if (entry.type !== "asset" || entry.kind !== "ui") throw new RefError(`${ref} is not a UI asset. Use find or list with kind ui.`);
  return { entry, ...readUi(project, entry.path) };
}

async function operations(project: ProjectSnapshot, ui: UiDocument, edits: UiOperation[]) {
  const catalogue = await uiCatalogue(project), added: string[] = [];
  for (const [i, edit] of edits.entries()) {
    try {
      if (edit.op === "add") {
        const id = edit.id ?? `n-${randomUUID()}`;
        if (ui.nodes.some(node => node.id === id)) throw new Error(`Duplicate widget ID ${id}.`);
        // MCP must reject missing/non-container parents rather than use the designer's selection fallback.
        if (ui.nodes.length && !catalogue.widgets.find(spec => spec.id === ui.nodes.find(node => node.id === edit.parent)?.type)?.container) throw new Error("Choose an existing panel parent.");
        if (!ui.nodes.length && edit.parent) throw new Error("The first widget must be the root, without a parent.");
        ui = addWidget(ui, catalogue, edit.type, edit.parent ?? undefined, id);
        added.push(id);
      } else {
        const node = ui.nodes.find(node => node.id === edit.id);
        if (!node) throw new Error(`No widget ${edit.id}.`);
        if ((edit.op === "remove" || edit.op === "move") && !node.parent) throw new Error("The root cannot be removed or moved.");
        if (edit.op === "remove") ui = removeNode(ui, edit.id);
        else if (edit.op === "move") ui = moveWidget(ui, catalogue, edit.id, edit.parent);
        else if (edit.op === "reorder") ui = shiftNode(ui, edit.id, edit.by);
      }
      if (edit.op === "add" || edit.op === "update") {
        const id = edit.op === "add" ? added.at(-1)! : edit.id;
        for (const [name, value] of Object.entries(edit.fields ?? {})) {
          if (["id", "type", "parent", "__proto__", "constructor", "prototype"].includes(name)) throw new Error(`${name} is not an editable widget field.`);
          ui = setNodeField(ui, id, name, value === null ? undefined : value as ComponentValue);
        }
      }
    } catch (error) { throw new RefError(`UI operation ${i + 1} (${edit.op}): ${error instanceof Error ? error.message : error}`); }
  }
  return { ui, added };
}

export async function createUi(project: ProjectSnapshot, input: { name: string; description?: string; canvas: [number, number]; nodes: UiNodeInput[]; dryRun?: boolean }) {
  const id = randomUUID();
  const { ui, added } = await operations(project, { id, name: input.name.trim(), description: input.description ?? "", canvas: input.canvas, nodes: [] }, input.nodes.map(node => ({ ...node, op: "add" })));
  const result = await publishAsset(project, "create_ui", `create UI ${ui.name} (asset:${id})`, `${UI_DIRECTORY}/${id}.ui.toml`, null, serializeUi(ui), "--validate-ui", input.dryRun, `asset:${id}`);
  return { ...result, nodes: added };
}

export async function editUi(project: ProjectSnapshot, index: ProjectIndex, input: { ref: string; revision: string; name?: string; description?: string; canvas?: [number, number]; operations?: UiOperation[]; dryRun?: boolean }) {
  const current = targetUi(project, index, input.ref);
  if (current.revision !== input.revision) throw new RefError(`${input.ref} changed since you described it. Describe it again before editing.`);
  const { ui, added } = await operations(project, {
    ...current.ui, name: input.name?.trim() ?? current.ui.name, description: input.description ?? current.ui.description, canvas: input.canvas ?? current.ui.canvas,
  }, input.operations ?? []);
  const result = await publishAsset(project, "edit_ui", `edit UI ${ui.name} (${current.entry.ref})`, current.path, project.files.find(file => file.path === current.path)!.content!, serializeUi(ui), "--validate-ui", input.dryRun, current.entry.ref);
  return { ...result, nodes: added };
}

export function describeUi(content: string) {
  const ui = parseUi(content);
  return { ui, actions: uiActions(ui), revision: uiRevision(content) };
}

export async function previewUi(project: ProjectSnapshot, index: ProjectIndex, input: { ref: string; offset?: number; limit?: number }) {
  const { entry, path, ui, revision } = targetUi(project, index, input.ref);
  await validateAsset(project, "--validate-ui", path, project.files.find(file => file.path === path)!.content!);
  const layout = uiLayout(ui), offset = input.offset ?? 0, limit = input.limit ?? 100;
  return {
    ref: entry.ref, revision, canvas: ui.canvas, actions: uiActions(ui), total: ui.nodes.length, offset,
    nodes: ui.nodes.slice(offset, offset + limit).map(node => ({ ...node, rect: layout.get(node.id) })),
    note: "Canvas layout in pixels, using the designer's Rust/TS parity-tested layout after native validation. No in-game rendering or action dispatch.",
  };
}
