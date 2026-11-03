import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { setTimeout } from "node:timers/promises";
import { chromium, expect } from "@playwright/test";
import { parse, stringify } from "smol-toml";

// Build first: EMERGENT_NEXT_DIST_DIR=.next-prefab-build npm run build.
// All authored writes and journals go to a disposable standard-template project.
async function main() {
  const base = "http://localhost:3001";
  assert.equal(await fetch(base).then(() => true, () => false), false, "Port 3001 must be free");
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-prefab-smoke-"));
  const root = path.join(directory, "game"), data = path.join(directory, "data"), foundation = path.resolve("../..");
  let server: ReturnType<typeof spawn> | undefined, browser: Awaited<ReturnType<typeof chromium.launch>> | undefined;
  let output = "", session: string | null = null, projectId = "";
  try {
    await cp(path.join(foundation, "templates/standard"), root, { recursive: true });
    const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8"));
    (cargo.dependencies as Record<string, { path: string }>).emergent.path = foundation; cargo.workspace = {};
    await writeFile(path.join(root, "Cargo.toml"), stringify(cargo));
    await cp(path.join(foundation, "Cargo.lock"), path.join(root, "Cargo.lock"));
    projectId = parse(await readFile(path.join(root, "emergent.toml"), "utf8")).id as string;
    const scenePath = "assets/scenes/main.scene.toml";
    let scene = await readFile(path.join(root, scenePath), "utf8");
    scene = scene.replace('id = "cube"\nasset = "assets/entities/cube.entity.toml"', 'id = "cube"\nname = "Drone"\nasset = "assets/entities/cube.entity.toml"\n[[entities.components]]\ntype="emergent.brain"\nbrain="assets/drone.brain.toml"');
    for (let rotor = 0; rotor < 4; rotor++) scene += `\n[[entities]]\nid="rotor${rotor}"\nparent="cube"\nasset="assets/entities/cube.entity.toml"\n[entities.transform]\ntranslation=[${rotor % 2 ? 1 : -1},1,${rotor < 2 ? 1 : -1}]\nscale=[0.3,0.1,0.3]\n[[entities.components]]\ntype="emergent.look_at"\ntarget="cube"\n`;
    await writeFile(path.join(root, scenePath), scene);
    await writeFile(path.join(root, "assets/drone.brain.toml"), 'schema_version=1\nid="drone-brain"\nname="Drone brain"\nkind="brain"\n[[nodes]]\nid="seek"\ntype="seek"\ntarget="rotor0"\n');
    await mkdir(path.join(root, "assets/prefabs"));
    for (let n = 0; n < 12; n++) await writeFile(path.join(root, `assets/prefabs/library${n}.prefab.toml`), stringify({ schema_version: 1, id: `library${n}`, name: `Library ${n}`, root: "body", entities: [{ id: "body", asset: "assets/entities/cube.entity.toml" }] }));
    await mkdir(data);
    await writeFile(path.join(data, "ai-provider.json"), JSON.stringify({ provider: "anthropic", providers: { anthropic: { apiKey: "smoke-test-key" } } }));
    server = spawn(process.execPath, ["node_modules/next/dist/bin/next", "start", "--hostname", "127.0.0.1", "--port", "3001"], {
      env: { ...process.env, EMERGENT_PROJECT_PATH: root, EMERGENT_IDE_DATA_DIR: data, EMERGENT_NEXT_DIST_DIR: ".next-prefab-build" }, stdio: ["ignore", "pipe", "pipe"],
    });
    server.stdout?.on("data", bytes => { output = (output + bytes).slice(-12000); });
    server.stderr?.on("data", bytes => { output = (output + bytes).slice(-12000); });
    let ready = false;
    for (let attempt = 0; attempt < 120; attempt++) {
      if (server.exitCode !== null) throw new Error(output);
      if (await fetch(base).then(response => response.ok, () => false)) { ready = true; break; }
      await setTimeout(250);
    }
    assert.ok(ready, output);
    browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL ?? "chrome", headless: true });
    const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
    page.setDefaultTimeout(120000);
    const errors: string[] = []; page.on("pageerror", error => errors.push(error.message));
    await page.route("**/api/fal-balance*", route => route.fulfill({ json: { state: "no-key", balance: 0, currency: "USD", message: "Smoke test", checkedAt: null } }));
    await page.goto(base);
    const cube = page.getByRole("button", { name: "Select instance cube", exact: true });
    await expect(cube).toBeEnabled();
    const canvas = page.getByLabel("Live Rust inspection viewport", { exact: true });
    await expect.poll(async () => Number(await canvas.getAttribute("data-frame")), { timeout: 120000 }).toBeGreaterThan(1);
    session = await canvas.getAttribute("data-session");
    await expect(page.locator(".scene-graph")).toHaveCount(1);
    await expect(cube).toBeEnabled(); await cube.click();
    const controls = page.locator(".prefab-controls");
    console.log("Native inspection ready; root selected");
    const [saved] = await Promise.all([
      page.waitForResponse(r => r.url().endsWith("/api/prefabs") && r.request().method() === "POST"),
      page.getByRole("treeitem", { name: "cube", exact: true }).dragTo(controls.getByRole("region", { name: "Prefab library", exact: true })),
    ]); assert.equal(saved.status(), 200, await saved.text());
    const prefab = await saved.json(); console.log("Captured by drop", prefab.name); assert.equal(prefab.name, "Drone");
    const definition = parse(await readFile(path.join(root, prefab.path), "utf8"));
    assert.equal((definition.entities as unknown[]).length, 5);
    assert.equal((parse(await readFile(path.join(root, scenePath), "utf8")).entities as unknown[]).length, 8);
    const library = controls.getByRole("region", { name: "Prefab library", exact: true });
    const prefabRow = library.getByRole("row", { name: "Prefab Drone", exact: true });
    await expect(prefabRow).toHaveCount(1);
    assert.ok(await library.evaluate(element => element.scrollHeight > element.clientHeight), "Prefab table must scroll internally");
    await expect(controls.getByRole("button",{name:/Prefabs/})).toHaveAttribute("aria-expanded","true");
    await expect(controls.locator("select")).toHaveCount(0);
    const roots: string[] = [];
    for (const [index, name] of ["Drone A", "Drone B"].entries()) {
      await expect(prefabRow).toHaveAttribute("draggable", "true");
      const target = index === 0 ? page.locator(".scene-root") : page.getByRole("treeitem", { name: "cube", exact: true });
      const [placed] = await Promise.all([
        page.waitForResponse(r => r.url().endsWith("/api/scenes/place") && r.request().method() === "POST"),
        prefabRow.dragTo(target),
      ]); assert.equal(placed.status(), 200, await placed.text());
      const rootId = (await placed.json()).ref.split("/").pop(); roots.push(rootId); console.log("Dropped", name);
      await expect(page.getByRole("treeitem", { name: rootId, exact: true })).toHaveAttribute("aria-level", String(index + 1));
      const select = page.getByRole("button", { name: `Select instance ${rootId}`, exact: true });
      await expect(select).toBeEnabled(); await select.click(); await select.press("Enter");
      const input = page.getByLabel(`Rename instance ${rootId}`, { exact: true }); await input.fill(name);
      const [renamed] = await Promise.all([
        page.waitForResponse(r => r.url().endsWith("/api/scenes/placement") && r.request().method() === "PUT"), input.press("Enter"),
      ]); assert.equal(renamed.status(), 200, await renamed.text());
      await expect(input).toHaveCount(0);
    }
    type Member = { id: string; name?: string; parent?: string; components?: { target?: string }[]; brain_bindings?: Record<string, string> };
    const members = parse(await readFile(path.join(root, scenePath), "utf8")).entities as Member[];
    assert.equal(members.length, 18);
    for (const [index, rootId] of roots.entries()) {
      const instance = members.find(item => item.id === rootId)!;
      assert.equal(instance.name, index === 0 ? "Drone A" : "Drone B");
      const children = members.filter(item => item.parent === rootId);
      assert.equal(instance.parent, index === 0 ? undefined : "cube");
      assert.equal(children.length, 4);
      assert.ok(children.some(item => item.id === instance.brain_bindings?.rotor0));
      assert.ok(children.every(item => item.components?.[0].target === rootId));
      for (const child of children) await expect(page.getByRole("treeitem", { name: child.id, exact: true })).toHaveAttribute("aria-level", String(index + 2));
    }
    const current = await (await page.request.get(`${base}/api/scenes/placement?projectId=${encodeURIComponent(projectId)}&path=${scenePath}`)).json();
    const stale = await page.request.post(`${base}/api/prefabs`, { data: { projectId, path: scenePath, root: "cube", name: "Bad", revision: "0".repeat(64) } });
    assert.equal(stale.status(), 400);
    const invalid = await page.request.post(`${base}/api/prefabs`, { data: { projectId, path: scenePath, root: "cube", name: "", revision: current.revision } });
    assert.equal(invalid.status(), 400);
    const crossOrigin = await page.request.post(`${base}/api/prefabs`, { headers: { origin: "https://example.invalid" }, data: { projectId } });
    assert.equal(crossOrigin.status(), 400);
    await expect.poll(async () => Number(await canvas.getAttribute("data-frame")), { timeout: 120000 }).toBeGreaterThan(2);
    await expect(cube).toBeEnabled();
    page.once("dialog", dialog => dialog.accept());
    await page.getByRole("button", { name: "Delete Drone A", exact: true }).click();
    await expect(page.getByRole("treeitem", { name: roots[0], exact: true })).toHaveCount(0);
    await expect(page.getByRole("treeitem", { name: roots[1], exact: true })).toHaveCount(1);
    const afterDelete = parse(await readFile(path.join(root, scenePath), "utf8")).entities as Member[];
    assert.equal(afterDelete.length, 13);
    assert.ok(members.filter(item => item.parent === roots[0]).every(child => !afterDelete.some(item => item.id === child.id)));
    await expect.poll(async () => Number(await canvas.getAttribute("data-frame")), { timeout: 120000 }).toBeGreaterThan(2);
    await expect(cube).toBeEnabled();
    await library.scrollIntoViewIfNeeded();
    // Keyboard creation uses the selected subtree; Enter on its row instantiates at root.
    await page.getByRole("button", { name: `Select instance ${roots[1]}`, exact: true }).click();
    const dropzone = controls.getByRole("region", { name: "Create prefab from scene object", exact: true });
    const [keyboardSaved] = await Promise.all([
      page.waitForResponse(r => r.url().endsWith("/api/prefabs") && r.request().method() === "POST"), dropzone.press("Enter"),
    ]); assert.equal(keyboardSaved.status(), 200, await keyboardSaved.text());
    const keyboardRow = library.getByRole("row", { name: "Prefab Drone B", exact: true });
    await expect(keyboardRow).toHaveAttribute("draggable", "true");
    const [keyboardPlaced] = await Promise.all([
      page.waitForResponse(r => r.url().endsWith("/api/scenes/place") && r.request().method() === "POST"), keyboardRow.press("Enter"),
    ]); assert.equal(keyboardPlaced.status(), 200, await keyboardPlaced.text());
    const keyboardId = (await keyboardPlaced.json()).ref.split("/").pop();
    await expect(page.getByRole("treeitem", { name: keyboardId, exact: true })).toHaveAttribute("aria-level", "1");
    await expect.poll(async () => Number(await canvas.getAttribute("data-frame")), { timeout: 120000 }).toBeGreaterThan(2);
    await expect(cube).toBeEnabled();
    const staleData = await page.evaluateHandle(({ projectId, scenePath }) => { const data = new DataTransfer(); data.setData("application/x-emergent-instance", JSON.stringify({ projectId, scene: scenePath, revision: "stale", id: "cube" })); return data; }, { projectId, scenePath });
    await dropzone.dispatchEvent("drop", { dataTransfer: staleData });
    await expect(controls.getByRole("status")).toContainText("changed since dragging");
    await staleData.dispose();
    await library.scrollIntoViewIfNeeded();
    await expect(page.locator(".scene-external-change")).not.toContainText("AI changed the project");
    await page.screenshot({ path: "/tmp/emergent-prefabs.png", fullPage: true });
    assert.deepEqual(errors, []);
    session = await canvas.getAttribute("data-session");
    console.log("Prefab browser smoke passed: standard-template host, native inspection, drag capture, scrolling table, root/child drops, keyboard capture/placement, five-member graphs, isolated brain/component links, renaming, independent subtree deletion and stale drag/revision/name/origin rejection. Screenshot: /tmp/emergent-prefabs.png");
  } finally {
    if (session) await fetch(`${base}/api/inspection`, { method: "DELETE", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ projectId, id: session }) }).catch(() => {});
    await browser?.close();
    if (server && server.exitCode === null) { server.kill("SIGTERM"); await once(server, "exit"); }
    await rm(directory, { recursive: true, force: true });
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
