import assert from "node:assert/strict";
import test from "node:test";
import { cp, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse, stringify } from "smol-toml";
import { POST } from "../src/app/api/mcp/route";
import { mcpToken } from "../src/lib/mcp-auth";
import { mcpChat } from "../src/lib/mcp-chat";
import { readProjectSnapshot } from "../src/lib/project";
import { subscribeProjectEvents, type ProjectEvent } from "../src/lib/project-events";

test("HTTP and embedded MCP clients author UIs with native validation, layout, revisions, dry runs and undo", { timeout: 240000 }, async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-mcp-ui-"));
  const root = path.join(directory, "game"), foundation = path.resolve("../..");
  const oldRoot = process.env.EMERGENT_PROJECT_PATH, oldData = process.env.EMERGENT_IDE_DATA_DIR;
  const events: ProjectEvent[] = [], unsubscribe = subscribeProjectEvents(event => events.push(event));
  t.after(async () => {
    unsubscribe();
    if (oldRoot === undefined) delete process.env.EMERGENT_PROJECT_PATH; else process.env.EMERGENT_PROJECT_PATH = oldRoot;
    if (oldData === undefined) delete process.env.EMERGENT_IDE_DATA_DIR; else process.env.EMERGENT_IDE_DATA_DIR = oldData;
    await rm(directory, { recursive: true, force: true });
  });
  await cp(path.join(foundation, "templates/standard"), root, { recursive: true });
  const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8"));
  (cargo.dependencies as Record<string, { path: string }>).emergent.path = foundation; cargo.workspace = {};
  await writeFile(path.join(root, "Cargo.toml"), stringify(cargo));
  await cp(path.join(foundation, "Cargo.lock"), path.join(root, "Cargo.lock"));
  process.env.EMERGENT_PROJECT_PATH = root; process.env.EMERGENT_IDE_DATA_DIR = path.join(directory, "data");
  let id = 0;
  const rpc = async (method: string, params: unknown) => {
    const response = await POST(new Request("http://127.0.0.1:3001/api/mcp", {
      method: "POST", headers: { "Content-Type": "application/json", Accept: "application/json, text/event-stream", host: "127.0.0.1:3001", Authorization: `Bearer ${await mcpToken()}` },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++id, method, params }),
    }));
    assert.equal(response.status, 200);
    return (await response.json()).result;
  };
  const tool = async (name: string, args: unknown = {}, error = false) => {
    const result = await rpc("tools/call", { name, arguments: args });
    const text = result.content[0].text;
    assert.equal(result.isError === true, error, text);
    return text.startsWith("{") ? JSON.parse(text) : text;
  };
  const tools = await rpc("tools/list", {});
  for (const name of ["list_ui_widgets", "create_ui", "edit_ui", "preview_ui"]) assert.ok(tools.tools.some((tool: { name: string }) => tool.name === name));
  const catalogue = await tool("list_ui_widgets"); assert.equal(catalogue.widgets.length, 11);
  assert.ok(catalogue.anchors.includes("bottom_right"));
  const found = await tool("find", { query: "Options", type: "asset" }); assert.equal(found.matches[0].kind, "ui");
  const listed = await tool("list", { type: "assets", kind: "ui" }); assert.equal(listed.total, 1);
  const described = await tool("describe", { ref: found.matches[0].ref }); assert.deepEqual(described.ui.canvas, [640, 480]);
  const samplePreview = await tool("preview_ui", { ref: found.matches[0].ref, offset: 1, limit: 2 });
  assert.equal(samplePreview.nodes.length, 2); assert.equal(samplePreview.offset, 1);
  assert.equal(samplePreview.nodes[0].id, "tabs");
  assert.match(samplePreview.note, /No in-game rendering/);

  const input = { name: "MCP UI", canvas: [200, 100], nodes: [
    { id: "root", type: "panel", fields: { padding: 10 } },
    { id: "left", type: "panel", parent: "root", fields: { w: "50%", stack: true, gap: 4 } },
    { id: "a", type: "button", parent: "left", fields: { h: "20px", text: "APPLY", action: "do.apply" } },
    { id: "b", type: "label", parent: "left", fields: { text: "BEFORE" } },
    { id: "right", type: "panel", parent: "root", fields: { x: "50%", w: "50%" } },
  ] };
  const dry = await tool("create_ui", { ...input, dryRun: true });
  assert.equal(dry.applied, false); assert.equal(dry.changeId, null);
  await assert.rejects(stat(path.join(root, dry.path)), { code: "ENOENT" });
  assert.equal(events.length, 0); assert.deepEqual((await tool("history")).changes, []);
  const created = await tool("create_ui", input);
  assert.equal(created.applied, true); assert.match(created.ref, /^asset:/);
  const initial = await readFile(path.join(root, created.path), "utf8");
  assert.equal(events.at(-1)?.paths[0], created.path);
  const preview = await tool("preview_ui", { ref: created.ref });
  assert.deepEqual(preview.nodes.find((node: { id: string }) => node.id === "left").rect, { x: 10, y: 10, w: 90, h: 80 });
  assert.deepEqual(preview.actions, ["do.apply"]);
  const edit = { ref: created.ref, revision: created.revision, canvas: [300, 120], operations: [
    { op: "add", id: "c", type: "label", parent: "left", fields: { text: "AFTER", action: "invalid-on-label" } },
    { op: "update", id: "c", fields: { text: "CHANGED", action: null, w: null } },
    { op: "add", id: "d", type: "label", parent: "left" },
    { op: "reorder", id: "d", by: -1 },
    { op: "move", id: "a", parent: "right" },
    { op: "remove", id: "b" },
  ] };
  await tool("edit_ui", { ...edit, dryRun: true }); assert.equal(await readFile(path.join(root, created.path), "utf8"), initial);
  const edited = await tool("edit_ui", edit);
  const detail = await tool("describe", { ref: created.ref });
  assert.deepEqual(detail.ui.canvas, [300, 120]);
  assert.equal(detail.ui.nodes.find((node: { id: string }) => node.id === "a").parent, "right");
  assert.deepEqual(detail.ui.nodes.filter((node: { parent?: string }) => node.parent === "left").map((node: { id: string }) => node.id), ["d", "c"]);
  assert.equal(detail.ui.nodes.find((node: { id: string }) => node.id === "c").fields.action, undefined);
  const good = await readFile(path.join(root, created.path), "utf8");
  await tool("edit_ui", { ref: created.ref, revision: created.revision, name: "Stale" }, true);
  const invalidOperations = [
    [{ op: "update", id: "c", fields: { unknown: 1 } }],
    [{ op: "update", id: "c", fields: { id: "identity-replacement" } }],
    [{ op: "update", id: "gone", fields: { text: "missing" } }],
    [{ op: "move", id: "left", parent: "left" }],
    [{ op: "move", id: "left", parent: "c" }],
    [{ op: "remove", id: "root" }],
    [{ op: "add", type: "label", parent: "missing" }],
    [{ op: "add", id: "c", type: "label", parent: "left" }],
    [{ op: "move", id: "c" }],
  ];
  const changes = (await tool("history")).changes.length;
  const noChange = await tool("edit_ui", { ref: created.ref, revision: edited.revision });
  assert.equal(noChange.applied, false); assert.equal(noChange.changeId, null);
  for (const operations of invalidOperations) {
    await tool("edit_ui", { ref: created.ref, revision: edited.revision, operations }, true);
    assert.equal(await readFile(path.join(root, created.path), "utf8"), good);
  }
  await tool("create_ui", { ...input, nodes: [...input.nodes, input.nodes[1]] }, true);
  await tool("create_ui", { ...input, nodes: Array.from({ length: 513 }, () => input.nodes[0]) }, true);
  assert.equal((await tool("history")).changes.length, changes);

  // Embedded chat discovers and calls the same tools without its private file-edit fallback.
  const project = await readProjectSnapshot(); let step = 0;
  const chat = await mcpChat(project, { projectId: project.id, scope: `file:${created.path}`, scenePath: project.entryScene, message: "Rename this UI", history: [] },
    async () => { throw new Error("UI edits must not use the file fallback"); },
    (_settings, instructions, _prompt, tools) => {
      assert.match(instructions, /create_ui/); assert.ok(tools.some(tool => tool.name === "edit_ui"));
      return async results => {
        if (step++ === 0) return { text: "", calls: [{ id: "ui_edit", name: "edit_ui", arguments: { ref: created.ref, revision: edited.revision, name: "Embedded UI" } }] };
        assert.equal(results[0].isError, false); return { text: "Renamed through MCP", calls: [] };
      };
    });
  assert.equal(chat.applied, true); assert.match(chat.text, /Completed MCP changes/);
  const renamed = await tool("describe", { ref: created.ref }); assert.equal(renamed.ui.name, "Embedded UI");
  await tool("undo", {}); assert.equal(await readFile(path.join(root, created.path), "utf8"), good);
  await writeFile(path.join(root, created.path), good + "\n# external change\n");
  await tool("undo", { changeId: edited.changeId }, true);
  assert.match(await readFile(path.join(root, created.path), "utf8"), /external change/);
  await writeFile(path.join(root, created.path), good);
  await tool("undo", { changeId: edited.changeId }); assert.equal(await readFile(path.join(root, created.path), "utf8"), initial);
  await tool("undo", { changeId: created.changeId });
  await tool("describe", { ref: created.ref }, true);
  await tool("preview_ui", { ref: created.ref }, true);
  await assert.rejects(stat(path.join(root, created.path)), { code: "ENOENT" });
});
