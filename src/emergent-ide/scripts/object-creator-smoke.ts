import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { chromium, expect, type Browser } from "@playwright/test";
import { ObjectJobs } from "../src/lib/object-jobs";
import { referenceImage, triangleGlb } from "../tests/object-fixture";
import { readProjectSnapshot } from "../src/lib/project";
import { saveStatementEvaluation } from "../src/lib/statements";

// Exercises the production UI and real save/routes in an isolated temporary game.
// Only generation submission/observation is mocked; no paid provider calls occur.
async function main() {
  const temporary = await mkdtemp(path.join(os.tmpdir(), "emergent-object-ui-"));
  const root = path.join(temporary, "game"), data = path.join(temporary, "data");
  await mkdir(path.join(root, "assets"), { recursive: true });
  await mkdir(path.join(root, "src"));
  await writeFile(path.join(root, "emergent.toml"), 'id = "object-smoke"\nname = "Object Smoke"\nentry_scene = "assets/main.scene.toml"\nasset_roots = ["assets"]\n');
  await writeFile(path.join(root, "assets/main.scene.toml"), 'id = "main"\n');
  await mkdir(data, { recursive: true });
  await writeFile(path.join(data, "ai-provider.json"), JSON.stringify({ baseUrl: "https://api.openai.com/v1", model: "test-model", apiKey: "mock-key-not-sent-to-provider" }));
  const project = { id: "object-smoke", root, assetRoots: ["assets"] };
  let submissions = 0;
  const jobs = new ObjectJobs(data, async () => ({
    upload: async () => "https://fal.media/mock.png",
    submit: async () => { submissions++; return "mock-provider-request"; },
    status: async () => "completed", result: async () => triangleGlb(),
  }));
  const socket = net.createServer();
  await new Promise<void>((resolve) => socket.listen(0, "127.0.0.1", resolve));
  const port = (socket.address() as net.AddressInfo).port;
  await new Promise<void>((resolve) => socket.close(() => resolve()));
  const base = `http://localhost:${port}`;
  const start = () => spawn("pnpm", ["exec", "next", "start", "--hostname", "127.0.0.1", "--port", String(port)], { env: { ...process.env, EMERGENT_PROJECT_PATH: root, EMERGENT_IDE_DATA_DIR: data, FAL_KEY: "" }, stdio: "pipe" });
  let server = start();
  let logs = "";
  const watch = () => { server.stdout?.on("data", (b) => { logs += b; }); server.stderr?.on("data", (b) => { logs += b; }); };
  watch();
  async function ready() {
    for (let i = 0; i < 100; i++) {
      try { if ((await fetch(base + "/api/settings/objects")).ok) return; } catch { /* Starting. */ }
      if (server.exitCode !== null) throw new Error(logs);
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
    throw new Error("IDE did not start: " + logs);
  }
  async function stop() {
    const child = server;
    if (child.exitCode !== null) return;
    child.kill("SIGTERM");
    await new Promise<void>((resolve) => child.once("exit", () => resolve()));
  }
  let browser: Browser | undefined;
  try {
    browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || "chrome", headless: true });
    await ready();
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    let automaticChecks = 0;
    await page.route("**/api/statements/evaluate", async (route) => {
      automaticChecks++;
      const body = route.request().postDataJSON();
      const previousProject = process.env.EMERGENT_PROJECT_PATH;
      process.env.EMERGENT_PROJECT_PATH = root;
      try {
        const selectedProject = await readProjectSnapshot();
        const statements = await saveStatementEvaluation(selectedProject, body.subject, body.id, { implemented: false, confidence: "high", evidence: "The fixture scene has no rotation." });
        await route.fulfill({ json: { statements } });
      } finally { if (previousProject === undefined) delete process.env.EMERGENT_PROJECT_PATH; else process.env.EMERGENT_PROJECT_PATH = previousProject; }
    });
    await page.goto(base);
    await page.locator(".statement-composer textarea").fill("This scene slowly rotates");
    await page.locator(".statement-composer button").click();
    await expect(page.locator(".statement-item")).toContainText("Not implemented");
    assert.equal(automaticChecks, 1);
    await expect(page.locator(".statement-item").getByRole("button", { name: "Implement", exact: true })).toBeEnabled();
    await page.route("**/api/statements/implement", (route) => route.fulfill({ status: 400, json: { error: "Candidate failed cargo check; project files were not changed." } }));
    await page.locator(".statement-item").getByRole("button", { name: "Implement", exact: true }).click();
    await expect(page.locator(".statement-error")).toContainText("project files were not changed");
    await page.unroute("**/api/statements/evaluate");
    await page.route("**/api/statements/evaluate", (route) => route.fulfill({ status: 502, json: { error: "AI provider unavailable (smoke)" } }));
    await page.locator(".statement-composer textarea").fill("Another saved intention");
    await page.locator(".statement-composer button").click();
    await expect(page.locator(".statement-error")).toContainText("AI provider unavailable");
    const intentions = JSON.parse(await readFile(path.join(root, "emergent.statements.json"), "utf8"));
    assert.equal(intentions.statements.length, 2);
    await page.goto(base + "/settings#object-generation");
    const section = page.locator("#object-generation");
    await section.getByLabel("fal API key").fill("mock-test-key-never-sent-to-fal");
    await section.getByRole("button", { name: "Save provider" }).click();
    await expect(section.getByText("Object generation settings saved.")).toBeVisible();
    const publicSettings = await (await fetch(base + "/api/settings/objects")).json();
    assert.equal(JSON.stringify(publicSettings).includes("mock-test-key"), false);
    await page.goto(base + "/objects");
    await expect(page.locator(".object-layout")).toHaveCSS("display", "grid");
    await page.route("**/api/objects", async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      const form = await new Response(new Uint8Array(route.request().postDataBuffer()!), { headers: { "Content-Type": route.request().headers()["content-type"] } }).formData();
      const job = await jobs.create(project, String(form.get("id")), String(form.get("name")), Buffer.from(await (form.get("image") as File).arrayBuffer()));
      await route.fulfill({ json: job, status: 201 });
    });
    await page.route("**/api/objects/*", async (route) => {
      if (route.request().method() !== "POST" || route.request().postDataJSON().action !== "refresh") return route.continue();
      const id = new URL(route.request().url()).pathname.split("/").at(-1)!;
      await route.fulfill({ json: await jobs.refresh(project, id) });
    });
    await page.getByLabel("Object name").fill("Smoke Rover");
    await page.locator('input[type="file"]').setInputFiles({ name: "reference.png", mimeType: "image/png", buffer: referenceImage });
    await page.getByRole("button", { name: "Generate object" }).click();
    await page.waitForFunction(() => (document.querySelector("model-viewer") as HTMLElement & { loaded?: boolean })?.loaded, { timeout: 30_000 });
    await page.getByRole("button", { name: "Reset view" }).click();
    await page.getByRole("button", { name: "Save to project" }).click();
    await expect(page.getByRole("button", { name: "Saved ✓" })).toBeVisible();
    assert.equal(submissions, 1);
    const saved = JSON.parse(await readFile(path.join(root, "assets/models/smoke-rover/object.asset.json"), "utf8"));
    assert.equal(saved.model, "assets/models/smoke-rover/model.glb");
    const screenshot = path.join(os.tmpdir(), "emergent-object-creator-smoke.png");
    await page.screenshot({ path: screenshot, fullPage: true });
    await stop(); server = start(); watch(); await ready();
    await page.goto(base + "/objects");
    await page.waitForFunction(() => (document.querySelector("model-viewer") as HTMLElement & { loaded?: boolean })?.loaded);
    await expect(page.getByRole("button", { name: "Saved ✓" })).toBeVisible();
    await page.goto(base);
    await page.getByRole("button", { name: "model.glb", exact: true }).click();
    await page.waitForFunction(() => (document.querySelector("model-viewer") as HTMLElement & { loaded?: boolean })?.loaded);
    await page.getByRole("button", { name: "object.asset.json", exact: true }).click();
    await page.waitForFunction(() => (document.querySelector("model-viewer") as HTMLElement & { loaded?: boolean })?.loaded);
    assert.deepEqual(errors, []);
    console.log("IDE UI smoke passed: automatic statement checking/failure persistence, Implement action, settings, mocked generation, real GLB preview/save, server restart, project selection. Screenshot: " + screenshot);
  } finally { await browser?.close(); await stop(); await rm(temporary, { recursive: true, force: true }); }
}
main().catch((error) => { console.error(error); process.exitCode = 1; });
