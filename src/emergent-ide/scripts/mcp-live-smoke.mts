// MCP-2 live check: an AI edit through the stdio bridge appears in an open IDE page without a manual reload.
// Run against an isolated IDE (EMERGENT_IDE_URL) whose linked project is a temporary copy.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import path from "node:path";
import { chromium, expect } from "@playwright/test";

// Next dev only serves its client runtime to allowed origins such as localhost, so the page URL is separate from the bridge URL.
const page_url = process.env.EMERGENT_PAGE_URL ?? "http://localhost:3001";
const bridge = spawn("node", [path.join(import.meta.dirname, "mcp-stdio.mjs")], { env: process.env, stdio: ["pipe", "pipe", "inherit"] });
let buffer = ""; const waiting = new Map<number, (message: { result: { isError?: boolean; content: { text: string }[] } }) => void>();
bridge.stdout.on("data", chunk => { buffer += chunk; let i; while ((i = buffer.indexOf("\n")) >= 0) { const message = JSON.parse(buffer.slice(0, i)); buffer = buffer.slice(i + 1); waiting.get(message.id)?.(message); } });
let id = 0;
const rpc = (method: string, params: unknown) => new Promise<{ result: { isError?: boolean; content: { text: string }[] } }>(resolve => { const n = ++id; waiting.set(n, resolve); bridge.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: n, method, params }) + "\n"); });
const tool = async (name: string, args: unknown) => { const { result } = await rpc("tools/call", { name, arguments: args }); assert.ok(!result.isError, result.content[0].text); return JSON.parse(result.content[0].text); };

const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || "chrome", headless: true });
try {
  await rpc("initialize", { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "live-smoke", version: "1" } });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  await page.goto(page_url);
  const frame = page.getByLabel("Live Rust inspection viewport", { exact: true });
  await frame.waitFor({ timeout: 600_000 }); // The first inspection of a fresh project copy builds its release worker.
  await expect.poll(async () => Number(await frame.getAttribute("data-frame")), { timeout: 180_000 }).toBeGreaterThan(1);
  await page.waitForTimeout(1500);
  // A camera inside geometry renders one flat colour; require real scene content before testing edits.
  const colours = await page.evaluate(() => {
    const canvas = document.querySelector("canvas[aria-label='Live Rust inspection viewport']") as HTMLCanvasElement;
    const pixels = canvas.getContext("2d")!.getImageData(0, 0, canvas.width, canvas.height).data, seen = new Set<number>();
    for (let i = 0; i < pixels.length; i += 16) seen.add((pixels[i] << 16) | (pixels[i + 1] << 8) | pixels[i + 2]);
    return seen.size;
  });
  assert.ok(colours > 64, `inspection shows real scene content (${colours} distinct colours)`);
  const before = await frame.screenshot();
  await page.screenshot({ path: "/tmp/emergent-mcp-live-before.png" });

  // Transform: pushed straight into the running inspection world, no session restart.
  const session = await page.evaluate(() => document.querySelector("[aria-label='Live Rust inspection viewport']")?.closest("[data-session]")?.getAttribute("data-session") ?? null);
  let { revision } = await tool("describe", { ref: "instance:main/pyramid" });
  const moved = await tool("set_transform", { ref: "instance:main/pyramid", revision, translation: [0, 2.5, 0] });
  assert.equal(moved.live, true, "transform edits are pushed into the running inspection world");
  await expect(page.getByRole("status").filter({ hasText: "AI: set local transform" })).toBeVisible({ timeout: 15_000 });
  await expect.poll(async () => Buffer.compare(await frame.screenshot(), before) !== 0, { timeout: 20_000 }).toBe(true);
  const afterMove = await frame.screenshot();
  await page.screenshot({ path: "/tmp/emergent-mcp-live-moved.png" });
  if (session) assert.equal(await page.evaluate(() => document.querySelector("[aria-label='Live Rust inspection viewport']")?.closest("[data-session]")?.getAttribute("data-session") ?? null), session, "no restart for live edits");

  // Material: not live-pushable, so the page restarts inspection itself.
  revision = moved.revision;
  const red = await tool("set_material", { ref: "instance:main/pyramid", revision, settings: { albedo: [1, 0, 0] } });
  assert.equal(red.live, false);
  await expect(page.getByRole("status").filter({ hasText: "AI: set material albedo" })).toBeVisible({ timeout: 15_000 });
  await expect.poll(async () => Buffer.compare(await frame.screenshot(), afterMove) !== 0, { timeout: 180_000 }).toBe(true);
  await page.waitForTimeout(2000);
  await page.screenshot({ path: "/tmp/emergent-mcp-live-red.png" });

  // The scene graph reflects an AI rename after refresh.
  revision = red.revision;
  await tool("rename", { ref: "instance:main/pyramid", revision, name: "AI pyramid" });
  await expect(page.getByRole("region", { name: "Scene instances" }).getByText("AI pyramid")).toBeVisible({ timeout: 20_000 });
  console.log("MCP live smoke passed. Screenshots: /tmp/emergent-mcp-live-{before,moved,red}.png");
} finally {
  await browser.close();
  bridge.stdin.end();
}
