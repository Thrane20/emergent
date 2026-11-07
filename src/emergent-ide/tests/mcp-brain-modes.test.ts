import assert from "node:assert/strict";
import test from "node:test";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { parse, stringify } from "smol-toml";
import { POST } from "../src/app/api/mcp/route";
import { mcpToken } from "../src/lib/mcp-auth";

test("MCP sets brain modes by asset ref, describes them and lists the brains' users, validated by the engine", { timeout: 300000 }, async t => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-mcp-modes-"));
  const root = path.join(directory, "game"), foundation = path.resolve("../..");
  const oldRoot = process.env.EMERGENT_PROJECT_PATH, oldData = process.env.EMERGENT_IDE_DATA_DIR;
  t.after(async () => {
    if (oldRoot === undefined) delete process.env.EMERGENT_PROJECT_PATH; else process.env.EMERGENT_PROJECT_PATH = oldRoot;
    if (oldData === undefined) delete process.env.EMERGENT_IDE_DATA_DIR; else process.env.EMERGENT_IDE_DATA_DIR = oldData;
    await rm(directory, { recursive: true, force: true });
  });
  await cp(path.join(foundation, "templates/standard"), root, { recursive: true });
  const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8"));
  (cargo.dependencies as Record<string, { path: string }>).emergent.path = foundation; cargo.workspace = {};
  (cargo.package as Record<string, unknown>).name = "emergent-mcp-modes-fixture";
  await writeFile(path.join(root, "Cargo.toml"), stringify(cargo));
  await cp(path.join(foundation, "Cargo.lock"), path.join(root, "Cargo.lock"));
  process.env.EMERGENT_PROJECT_PATH = root; process.env.EMERGENT_IDE_DATA_DIR = path.join(directory, "data");
  let id = 0;
  const tool = async (name: string, args: unknown = {}, error = false) => {
    const response = await POST(new Request("http://127.0.0.1:3001/api/mcp", {
      method: "POST", headers: { "Content-Type": "application/json", Accept: "application/json, text/event-stream", host: "127.0.0.1:3001", Authorization: `Bearer ${await mcpToken()}` },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++id, method: "tools/call", params: { name, arguments: args } }),
    }));
    const result = (await response.json()).result, text = result.content[0].text;
    assert.equal(result.isError === true, error, text);
    return text.startsWith("{") ? JSON.parse(text) : text;
  };

  const nodes = (await tool("list_brain_nodes")).nodes as { type: string }[];
  for (const node of ["taxi", "hold_course", "next_mode", "previous_mode", "switch_mode"]) assert.ok(nodes.some(item => item.type === node), node);
  const park = await tool("create_brain", { name: "Park", nodes: [{ type: "brake" }] });
  const roam = await tool("create_brain", { name: "Roam", nodes: [{ type: "wander" }] });
  let revision = (await tool("describe", { ref: "instance:main/cube" })).revision;

  // Brain and modes together are refused by the engine, and the scene stays as it was.
  const scenePath = path.join(root, "assets/scenes/main.scene.toml"), before = await readFile(scenePath, "utf8");
  const modes = [{ name: "park", brain: park.ref, active: true, on_next: "roam" }, { name: "roam", brain: roam.ref }];
  const refused = await tool("set_component", { ref: "instance:main/cube", revision, type: "emergent.brain", fields: { brain: park.ref, modes } }, true);
  assert.match(refused, /either brain and stack, or modes/);
  assert.equal(await readFile(scenePath, "utf8"), before);

  const set = await tool("set_component", { ref: "instance:main/cube", revision, type: "emergent.brain", fields: { modes } });
  assert.equal(set.applied, true);
  revision = set.revision;
  const cube = await tool("describe", { ref: "instance:main/cube" });
  const brain = cube.components.find((component: { type: string }) => component.type === "emergent.brain");
  assert.deepEqual(brain.fields.modes.map((mode: { name: string; brain: string; on_next?: string }) => [mode.name, mode.brain.endsWith(".brain.toml"), mode.on_next]), [["park", true, "roam"], ["roam", true, undefined]]);
  const users = (await tool("describe", { ref: roam.ref })).usedBy as { instance: string }[];
  assert.deepEqual(users.map(user => user.instance), ["instance:main/cube"], "a brain used only by a mode is still in use");

  // A hand-over to a mode that does not exist is refused with the engine's reason.
  const broken = await tool("set_component", { ref: "instance:main/cube", revision, type: "emergent.brain", fields: { modes: [{ ...modes[0], on_next: "fly" }, modes[1]] } }, true);
  assert.match(broken, /unknown mode 'fly'/);
});
