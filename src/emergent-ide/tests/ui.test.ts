import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { parse } from "smol-toml";
import { addWidget, isEmptyUiDraft, moveWidget, newlyCreatedUiPath, parseLength, parseUi, serializeUi, uiActions, uiCreationContext, uiLayout, uiProblems, type UiDocument, type UiSummary, type WidgetCatalogue } from "../src/lib/ui";

test("new UI generation accepts the starter panel and protects unsaved widget edits", () => {
  const ui: UiDocument = {id:"draft",name:"Flight HUD",description:"Altitude display",canvas:[800,600],nodes:[{id:"root",type:"panel",fields:{padding:10}}]};
  assert.equal(isEmptyUiDraft(undefined,ui),true);
  assert.equal(isEmptyUiDraft("assets/ui/hud.ui.toml",ui),false);
  assert.equal(isEmptyUiDraft(undefined,{...ui,nodes:[]}),false);
  assert.equal(isEmptyUiDraft(undefined,{...ui,nodes:[{...ui.nodes[0],fields:{padding:20}}]}),false);
  assert.equal(isEmptyUiDraft(undefined,{...ui,nodes:[...ui.nodes,{id:"label",type:"label",parent:"root",fields:{text:"Altitude"}}]}),false);
  assert.match(uiCreationContext(ui),/Flight HUD/);
  assert.match(uiCreationContext(ui),/800,600/);
  assert.match(uiCreationContext(ui),/create exactly one new project UI/);
});

test("generated UI selection requires exactly one new valid asset", () => {
  const summary=(path:string,error?:string):UiSummary=>({path,id:path,name:path,description:"",nodes:1,error});
  const existing=summary("old"),created=summary("new");
  assert.equal(newlyCreatedUiPath([existing],[existing,created]),"new");
  assert.equal(newlyCreatedUiPath([existing],[existing]),undefined);
  assert.equal(newlyCreatedUiPath([],[summary("invalid","bad UI")]),undefined);
  assert.equal(newlyCreatedUiPath([],[existing,created]),undefined);
});

const root = new URL("../../../", import.meta.url);
const options = readFileSync(new URL("templates/standard/assets/ui/options.ui.toml", root), "utf8");
const expected = parse(readFileSync(new URL("tests/fixtures/ui-options-layout.toml", root), "utf8")) as Record<string, number[]>;

test("preview layout matches the Rust fixture", () => {
  const ui = parseUi(options), rects = uiLayout(ui);
  assert.equal(rects.size, Object.keys(expected).length);
  for (const [id, [x, y, w, h]] of Object.entries(expected)) assert.deepEqual(rects.get(id), { x, y, w, h }, id);
});

test("lengths parse like Rust", () => {
  assert.deepEqual(parseLength("100%-16px"), { percent: 100, px: -16 });
  assert.deepEqual(parseLength("-8px"), { percent: 0, px: -8 });
  for (const bad of ["", "px", "10em", "5%%", "1-2", "1e999px", "1e39%"] ) assert.equal(parseLength(bad), undefined, bad);
});

test("widget edits preserve the tree and reject cycles and invalid fields", () => {
  const catalogue: WidgetCatalogue = { categories: [], anchors: [], widgets: [
    { id: "panel", label: "Panel", category: "Layout", container: true, description: "", fields: [] },
    { id: "button", label: "Button", category: "Controls", container: false, description: "", fields: [] },
  ] };
  const ui = { id: "test", name: "Test", description: "", canvas: [100, 100] as [number, number], nodes: [{ id: "root", type: "panel", fields: {} }] };
  const panel = addWidget(ui, catalogue, "panel", "root", "child");
  const button = addWidget(panel, catalogue, "button", "child", "button");
  assert.throws(() => moveWidget(button, catalogue, "child", "button"), /inside itself/);
  assert.throws(() => moveWidget(button, catalogue, "child", "missing"), /Only panels/);
  const moved = moveWidget(button, catalogue, "button", "root");
  assert.equal(moved.nodes[2].parent, "root");
  assert.equal(button.nodes[2].parent, "child");
  const invalid = { ...moved, nodes: [...moved.nodes, { id: "bad", type: "button", parent: "root", fields: { action: "bad action", w: "wide" } }] };
  assert.ok(uiProblems(invalid, catalogue).has("bad"));
  assert.deepEqual(uiActions({ ...ui, nodes: [{ id: "a", type: "button", fields: { action: "test.click" } }, { id: "b", type: "button", fields: { action: "test.click" } }] }), ["test.click"]);
});

test("UI documents round-trip", () => {
  const ui = parseUi(options);
  assert.deepEqual(parseUi(serializeUi(ui)), ui);
  for (const canvas of ["[0,480]", "[640.5,480]", "[4097,480]", "[640]"]) {
    assert.throws(() => parseUi(`schema_version=1\nid='test'\ncanvas=${canvas}`), /canvas/);
  }
});

test("nested horizontal, vertical and grid layout matches the native shared fixture", () => {
  const source=readFileSync(new URL("tests/fixtures/ui-nested-layout.ui.toml",root),"utf8");
  const expected=parse(readFileSync(new URL("tests/fixtures/ui-nested-layout-rects.toml",root),"utf8")) as Record<string,number[]>;
  const ui=parseUi(source), rects=uiLayout(ui);
  for(const [id,[x,y,w,h]] of Object.entries(expected))assert.deepEqual(rects.get(id),{x,y,w,h},id);
  assert.deepEqual(parseUi(serializeUi(ui)),ui);
  for(const mode of ["horizontal","vertical"]){
    const ui=parseUi(`schema_version=1\nid='stretch'\ncanvas=[100,50]\n[[nodes]]\nid='root'\ntype='panel'\nlayout='${mode}'\nalign_cross='stretch'\n[[nodes]]\nid='a'\nparent='root'\ntype='button'`);
    assert.deepEqual(uiLayout(ui).get("a"),{x:0,y:0,w:mode==="horizontal"?24:100,h:mode==="horizontal"?50:24});
  }
});

test("grid spans and auto content sizes match native layout",()=>{
 const ui=parseUi("schema_version=1\nid='span'\ncanvas=[120,120]\n[[nodes]]\nid='r'\ntype='grid'\ncolumns=3\n[[nodes]]\nid='a'\nparent='r'\ntype='label'\ncol_span=2\nrow_span=2\n[[nodes]]\nid='b'\nparent='r'\ntype='label'\n[[nodes]]\nid='c'\nparent='r'\ntype='label'");
 const rects=uiLayout(ui);assert.deepEqual(rects.get("a"),{x:0,y:0,w:80,h:120});assert.deepEqual(rects.get("b"),{x:80,y:0,w:40,h:60});assert.deepEqual(rects.get("c"),{x:80,y:60,w:40,h:60});
 const auto=parseUi("schema_version=1\nid='auto'\ncanvas=[100,50]\n[[nodes]]\nid='r'\ntype='panel'\n[[nodes]]\nid='row'\nparent='r'\ntype='horizontal'\nw='auto'\nh='auto'\npadding=2\ngap=3\n[[nodes]]\nid='a'\nparent='row'\ntype='button'\nw='10px'\nh='8px'\n[[nodes]]\nid='b'\nparent='row'\ntype='button'\nw='20px'\nh='12px'");
 assert.deepEqual(uiLayout(auto).get("row"),{x:0,y:0,w:37,h:16});
});
