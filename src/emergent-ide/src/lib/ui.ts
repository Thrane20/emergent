// UI documents (`*.ui.toml`): a flat [[nodes]] widget list with parent links, plus the widget catalogue
// exported by the game binary. `uiLayout` mirrors `UiDefinition::layout` in src/ui.rs; the shared fixture
// tests/fixtures/ui-options-layout.toml keeps the two equal.
import { parse, stringify } from "smol-toml";
import { childrenOf, subtree, type BrainNode } from "./brains";
import type { ComponentField, ComponentValue } from "./components";

export type WidgetSpec = { id: string; label: string; category: string; container: boolean; description: string; fields: ComponentField[] };
export type WidgetCatalogue = { categories: string[]; anchors: string[]; widgets: WidgetSpec[] };
export type UiNode = BrainNode;
export type UiDocument = { id: string; name: string; description: string; canvas: [number, number]; nodes: UiNode[] };
export type UiSummary = { path: string; id: string; name: string; description: string; nodes: number; error?: string; canvas?: [number, number]; boxes?: { type: string; rect: Rect }[] };
export type Rect = { x: number; y: number; w: number; h: number };

export function isEmptyUiDraft(path: string | undefined, ui: UiDocument) {
  const root = ui.nodes[0];
  return !path && ui.nodes.length === 1 && root.id === "root" && root.type === "panel" && !root.parent && Object.keys(root.fields).length === 1 && root.fields.padding === 10;
}

export function uiCreationContext(ui: UiDocument) {
  return `UI editor: this is a new, unsaved UI with only the starter panel. Draft metadata (data): ${JSON.stringify({ name: ui.name, description: ui.description, canvas: ui.canvas })}. Answer questions normally. If asked to build it, create exactly one new project UI using the UI authoring tools, with a complete valid widget tree. Do not require the user to save a placeholder first. Do not edit an existing UI or attach it to a scene unless requested.`;
}

export function newlyCreatedUiPath(before: UiSummary[], after: UiSummary[]) {
  const existing = new Set(before.map(ui => ui.path));
  const created = after.filter(ui => !ui.error && !existing.has(ui.path));
  return created.length === 1 ? created[0].path : undefined;
}

const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === "object" && !Array.isArray(value);
export const ANCHORS = ["top_left", "top", "top_right", "left", "center", "right", "bottom_left", "bottom", "bottom_right"];

export function widgetCatalogue(exported: unknown): WidgetCatalogue {
  if (!object(exported) || !Array.isArray(exported.categories) || !Array.isArray(exported.widgets)) throw new Error("Invalid UI widget catalogue.");
  const widgets = exported.widgets.map(widget => {
    if (!object(widget) || typeof widget.id !== "string" || typeof widget.label !== "string" || typeof widget.category !== "string" || !Array.isArray(widget.fields)) throw new Error("Invalid widget in catalogue.");
    return { ...widget, container: widget.container === true } as WidgetSpec;
  });
  return { categories: exported.categories.filter((c): c is string => typeof c === "string"), anchors: ANCHORS, widgets };
}

export function parseUi(source: string): UiDocument {
  const document = parse(source) as Record<string, unknown>;
  if (document.schema_version !== 1 || typeof document.id !== "string") throw new Error("A UI needs schema_version = 1 and an id.");
  if (document.kind !== undefined && document.kind !== "ui") throw new Error("kind must be ui.");
  if (!Array.isArray(document.canvas) || document.canvas.length !== 2 || !document.canvas.every(n => typeof n === "number" && Number.isInteger(n) && n >= 1 && n <= 4096)) throw new Error("A UI canvas needs two integer sizes from 1 to 4096 px.");
  const canvas = document.canvas as [number, number];
  const nodes = (Array.isArray(document.nodes) ? document.nodes : []).map(entry => {
    if (!object(entry) || typeof entry.id !== "string" || typeof entry.type !== "string") throw new Error("Each node needs an id and a type.");
    const { id, type, parent, ...fields } = entry;
    return { id, type, ...(typeof parent === "string" ? { parent } : {}), fields: JSON.parse(JSON.stringify(fields)) as Record<string, ComponentValue> };
  });
  return { id: document.id, name: typeof document.name === "string" ? document.name : document.id, description: typeof document.description === "string" ? document.description : "", canvas, nodes };
}

export function serializeUi(ui: UiDocument): string {
  const header = stringify({ schema_version: 1, id: ui.id, name: ui.name, kind: "ui", ...(ui.description.trim() ? { description: ui.description.trim() } : {}), canvas: ui.canvas.map(Math.round) });
  const nodes = ui.nodes.map(node => `[[nodes]]\n${stringify({ id: node.id, ...(node.parent ? { parent: node.parent } : {}), type: node.type, ...node.fields })}`);
  return `${header}\n${nodes.join("\n")}`.replace(/\n{3,}/g, "\n\n");
}

/** `a% + b px` as in Rust `Length::parse`; undefined when malformed. */
export function parseLength(text: string): { percent: number; px: number } | undefined {
  const compact = text.replace(/\s+/g, "");
  if (!compact || compact.length > 32) return undefined;
  const term = (t: string) => {
    const percent = t.endsWith("%"), number = percent ? t.slice(0, -1) : t.endsWith("px") ? t.slice(0, -2) : t;
    if (!/^[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$/i.test(number)) return undefined;
    const n = Number(number);
    if (!Number.isFinite(Math.fround(n))) return undefined;
    return percent ? { percent: n, px: 0 } : { percent: 0, px: n };
  };
  let split = -1;
  for (let i = 1; i < compact.length; i++) if ((compact[i] === "+" || compact[i] === "-") && (compact[i - 1] === "%" || compact[i - 1] === "x")) { split = i; break; }
  if (split < 0) return term(compact);
  const a = term(compact.slice(0, split)), b = term(compact.slice(split + 1)), s = compact[split] === "-" ? -1 : 1;
  return a && b ? { percent: a.percent + s * b.percent, px: a.px + s * b.px } : undefined;
}

const resolve = (length: { percent: number; px: number }, size: number) => size * length.percent / 100 + length.px;
const num = (value: ComponentValue | undefined, fallback: number) => typeof value === "number" ? value : fallback;
const length = (value: ComponentValue | undefined, fallback: string) => parseLength(typeof value === "string" ? value : fallback) ?? parseLength(fallback)!;

export const layoutMode = (node: UiNode) => typeof node.fields.layout === "string" ? node.fields.layout : ["horizontal", "vertical", "grid"].includes(node.type) ? node.type : "absolute";

export function gridSlots(children: UiNode[], columns: number): number[][] {
  const occupied=new Set<number>(),slots:number[][]=[];
  for(const node of children){
    const cs=num(node.fields.col_span,1),rs=num(node.fields.row_span,1);
    if(!Number.isInteger(cs)||!Number.isInteger(rs)||cs<1||cs>columns||rs<1||rs>64)throw new Error("Invalid grid span.");
    let found=false;
    for(let at=0;at<columns*512;at++){
      const x=at%columns,y=Math.floor(at/columns);if(x+cs>columns||y+rs>512)continue;
      let free=true;for(let row=y;row<y+rs;row++)for(let col=x;col<x+cs;col++)if(occupied.has(row*columns+col))free=false;
      if(!free)continue;for(let row=y;row<y+rs;row++)for(let col=x;col<x+cs;col++)occupied.add(row*columns+col);
      slots.push([x,y,cs,rs]);found=true;break;
    }
    if(!found)throw new Error("Grid spans exceed 512 rows.");
  }
  return slots;
}

/** Rect of every node by id, in canvas pixels. Unreachable nodes (cycles, orphans) get no rect. */
export function uiLayout(ui: Pick<UiDocument, "canvas" | "nodes">): Map<string, Rect> {
  const rects = new Map<string, Rect>(), root = ui.nodes.find(node => !node.parent);
  if (!root) return rects;
  rects.set(root.id, { x: 0, y: 0, w: ui.canvas[0], h: ui.canvas[1] });
  const natural=new Map<string,{w:number;h:number}>(),order=[root];
  for(let i=0;i<order.length&&i<512;i++)order.push(...childrenOf(ui,order[i].id));
  for(const node of [...order].reverse()){
    const scale=num(node.fields.size,2),text=typeof node.fields.text==="string"?node.fields.text:"";
    let w=Math.max(24,[...text].length*6*scale+12),h=8*scale+8;
    if(["panel","horizontal","vertical","grid"].includes(node.type)){w=2*num(node.fields.padding,0);h=w;}
    const children=childrenOf(ui,node.id),gap=num(node.fields.gap,0),mode=layoutMode(node);
    if(children.length){
      const sizes=children.map(child=>{const n=natural.get(child.id)??{w:0,h:0};return {w:child.fields.w===undefined||child.fields.w==="auto"?n.w:Math.max(0,length(child.fields.w,"0px").px),h:child.fields.h===undefined||child.fields.h==="auto"?n.h:Math.max(0,length(child.fields.h,"0px").px)};});
      if(mode==="absolute"&&node.fields.stack!==true)for(const [i,child] of children.entries()){sizes[i].w+=Math.max(0,length(child.fields.x,"0px").px);sizes[i].h+=Math.max(0,length(child.fields.y,"0px").px);}
      w=Math.max(...sizes.map(s=>s.w));h=Math.max(...sizes.map(s=>s.h));
      if(mode==="horizontal")w=sizes.reduce((a,s)=>a+s.w,0)+gap*(sizes.length-1);
      else if(mode==="vertical"||node.fields.stack===true)h=sizes.reduce((a,s)=>a+s.h,0)+gap*(sizes.length-1);
      else if(mode==="grid"){
        const cols=num(node.fields.columns,2),slots=gridSlots(children,cols),rows=Math.max(...slots.map(s=>s[1]+s[3]));
        w=Math.max(...slots.map((s,i)=>Math.max(0,sizes[i].w-gap*(s[2]-1))/s[2]))*cols+gap*(cols-1);
        h=Math.max(...slots.map((s,i)=>Math.max(0,sizes[i].h-gap*(s[3]-1))/s[3]))*rows+gap*(rows-1);
      }
      w+=2*num(node.fields.padding,0);h+=2*num(node.fields.padding,0);
    }
    natural.set(node.id,{w,h});
  }
  const queue = [root];
  for (let i = 0; i < queue.length && i < 4096; i++) {
    const parent = queue[i], box = rects.get(parent.id)!, pad = num(parent.fields.padding, 0);
    const content = { x: box.x + pad, y: box.y + pad, w: Math.max(0, box.w - 2 * pad), h: Math.max(0, box.h - 2 * pad) };
    const mode = layoutMode(parent), stack = parent.fields.stack === true, gap = num(parent.fields.gap, 0);
    const children = childrenOf(ui, parent.id), n = children.length;
    if (!n) continue;
    const factor = (key: string) => parent.fields[key] === "center" ? .5 : parent.fields[key] === "end" ? 1 : 0;
    const main = factor("align_main"), cross = factor("align_cross"), stretch = parent.fields.align_cross === "stretch";
    const columns = Math.max(1, Math.min(64, Math.floor(num(parent.fields.columns, 2)))), slots = mode === "grid" ? gridSlots(children, columns) : [], rows = slots.length ? Math.max(...slots.map(s=>s[1]+s[3])) : 1;
    const cw = mode === "grid" ? Math.max(0, (content.w - gap * (columns - 1)) / columns) : content.w;
    const ch = mode === "grid" ? Math.max(0, (content.h - gap * (rows - 1)) / rows) : content.h;
    const size = (node: UiNode) => {
      const sw=mode==="grid"?cw*num(node.fields.col_span,1)+gap*(num(node.fields.col_span,1)-1):cw;
      const sh=mode==="grid"?ch*num(node.fields.row_span,1)+gap*(num(node.fields.row_span,1)-1):ch;
      return {w:stretch&&mode==="vertical"?sw:node.fields.w==="auto"?natural.get(node.id)!.w:Math.max(0,resolve(length(node.fields.w,mode==="horizontal"?"24px":"100%"),sw)),h:stretch&&mode==="horizontal"?sh:node.fields.h==="auto"?natural.get(node.id)!.h:Math.max(0,resolve(length(node.fields.h,mode==="vertical"||mode==="absolute"&&stack?"24px":"100%"),sh))};
    };
    const used = children.reduce((total, child) => total + (mode === "horizontal" ? size(child).w : size(child).h), 0) + gap * (n - 1);
    let cursor = mode === "horizontal" ? Math.max(0, content.w - used) * main : mode === "vertical" ? Math.max(0, content.h - used) * main : 0;
    for (const [slot, child] of children.entries()) {
      if (rects.has(child.id)) continue;
      const f = child.fields, anchor = Math.max(0, ANCHORS.indexOf(typeof f.anchor === "string" ? f.anchor : "top_left"));
      const { w, h: height } = size(child), h = stretch && mode === "grid" ? ch*slots[slot][3]+gap*(slots[slot][3]-1) : height;
      let x: number, y: number;
      if (mode === "horizontal") { x = content.x + cursor; y = content.y + Math.max(0, content.h - h) * cross; cursor += w + gap; }
      else if (mode === "vertical") { x = content.x + Math.max(0, content.w - w) * cross; y = content.y + cursor; cursor += h + gap; }
      else if (mode === "grid") { const [col,row,cs,rs]=slots[slot];x=content.x+col*(cw+gap)+Math.max(0,cw*cs+gap*(cs-1)-w)*main;y=content.y+row*(ch+gap)+Math.max(0,ch*rs+gap*(rs-1)-h)*cross; }
      else {
        x = content.x + (anchor % 3) / 2 * (content.w - w) + resolve(length(f.x, "0px"), content.w);
        if (stack) { y = content.y + cursor; cursor += h + gap; }
        else y = content.y + Math.floor(anchor / 3) / 2 * (content.h - h) + resolve(length(f.y, "0px"), content.h);
      }
      rects.set(child.id, { x, y, w, h }); queue.push(child);
    }
  }
  return rects;
}

/** Adds a widget under `parent` (or as the root of an empty UI). Throws when the parent cannot hold widgets. */
export function addWidget(ui: UiDocument, catalogue: WidgetCatalogue, type: string, parent: string | undefined, id: string): UiDocument {
  const spec = catalogue.widgets.find(widget => widget.id === type);
  if (!spec) throw new Error(`Unknown widget ${type}.`);
  if (!ui.nodes.length) return { ...ui, nodes: [{ id, type, fields: {} }] };
  const target = ui.nodes.find(node => node.id === parent) ?? ui.nodes.find(node => !node.parent)!;
  const container = catalogue.widgets.find(widget => widget.id === target.type)?.container ? target : ui.nodes.find(node => node.id === target.parent);
  if (!container) throw new Error("Select a panel or layout container to add widgets into.");
  return { ...ui, nodes: [...ui.nodes, { id, type, parent: container.id, fields: spec.container ? {} : { ...(layoutMode(container) === "absolute" ? { w: "100%", h: "24px" } : {}), ...(spec.fields.some(field => field.name === "text") ? { text: spec.label } : {}) } }] };
}

/** Moves a widget (and its subtree) into another panel. */
export function moveWidget(ui: UiDocument, catalogue: WidgetCatalogue, id: string, parent: string): UiDocument {
  if (subtree(ui, id).has(parent)) throw new Error("A widget cannot move inside itself.");
  const target = ui.nodes.find(node => node.id === parent);
  if (!target || !catalogue.widgets.find(widget => widget.id === target.type)?.container) throw new Error("Only panels or layout containers hold widgets.");
  return { ...ui, nodes: ui.nodes.map(node => node.id === id ? { ...node, parent } : node) };
}

/** Problems the designer can show before Rust validation (which remains the authority). */
export function uiProblems(ui: UiDocument, catalogue: WidgetCatalogue): Map<string, string> {
  const problems = new Map<string, string>();
  for (const node of ui.nodes) {
    const spec = catalogue.widgets.find(widget => widget.id === node.type);
    if (!spec) { problems.set(node.id, `Unknown widget ${node.type}.`); continue; }
    if(layoutMode(node)==="grid")try{gridSlots(childrenOf(ui,node.id),num(node.fields.columns,2));}catch(error){problems.set(node.id,error instanceof Error?error.message:"Invalid grid.");}
    if (!spec.container && childrenOf(ui, node.id).length) problems.set(node.id, `${spec.label} cannot hold widgets.`);
    if (node.fields.columns !== undefined && (typeof node.fields.columns !== "number" || !Number.isInteger(node.fields.columns) || node.fields.columns < 1 || node.fields.columns > 64)) problems.set(node.id, "Grid columns must be an integer from 1 to 64.");
    for (const name of ["x", "y", "w", "h"]) {
      const value = node.fields[name];
      if (value !== undefined && (typeof value !== "string" || !(name!=="x"&&name!=="y"&&value==="auto")&&!parseLength(value))) problems.set(node.id, `${name} must be a length like 8px, 30% or 100%-16px.`);
    }
    for (const name of ["action", "value"]) {
      const value = node.fields[name];
      if (value !== undefined && (typeof value !== "string" || !/^[\w.-]{1,64}$/.test(value))) problems.set(node.id, `${name} may only use letters, digits, '.', '_' and '-'.`);
    }
  }
  if (ui.nodes.length && ui.nodes.filter(node => !node.parent).length !== 1) problems.set(ui.nodes[0].id, "A UI needs exactly one root.");
  return problems;
}

/** Every action label in node order, without repeats. */
export const uiActions = (ui: UiDocument) => [...new Set(ui.nodes.map(node => node.fields.action).filter((a): a is string => typeof a === "string" && !!a))];
