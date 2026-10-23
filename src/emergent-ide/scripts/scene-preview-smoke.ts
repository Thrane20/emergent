import assert from "node:assert/strict";
import { chromium, expect } from "@playwright/test";

// Requires a running IDE and its linked project's real Rust preview implementation.
// No project assets are changed, and no generation-provider requests are made.
async function main() {
  const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || "chrome", headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    await page.goto(process.env.EMERGENT_IDE_BASE_URL || "http://localhost:3000");
    await expect(page.getByLabel("Scene viewer", { exact: true })).toBeVisible();
    const response = page.waitForResponse((response) => response.url().endsWith("/api/scenes"), { timeout: 120_000 });
    await page.getByRole("button", { name: "Load", exact: true }).click();
    const rendered = await response;
    assert.equal(rendered.status(), 200, rendered.ok() ? "" : await rendered.text());
    assert.ok(rendered.headers()["content-type"].includes("image/png"));
    const frame = page.locator(".native-scene-frame");
    await expect(frame).toBeVisible();
    await expect(frame).toHaveJSProperty("naturalWidth", 1280);
    const previousFrame = await frame.getAttribute("src");
    await page.route("**/api/scenes", (route) => route.fulfill({ status: 400, json: { error: "Unsupported scene builder (smoke)" } }));
    await page.getByRole("button", { name: "Load", exact: true }).click();
    await expect(page.locator(".scene-load-error")).toContainText("Unsupported scene builder");
    await expect(frame).toHaveAttribute("src", previousFrame!);
    console.log("Rust scene UI smoke passed: real Cargo/wgpu PNG displayed; failed reload retains previous frame.");
  } finally { await browser.close(); }
}
main().catch((error) => { console.error(error); process.exitCode = 1; });
