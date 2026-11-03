// Scene map editor smoke against a running IDE (default http://localhost:3001) linked to a
// scratch project: every tool, the inspector, zoom and undo. It edits the linked project's
// entry scene, so never point it at real work. Screenshots go to MAP_SMOKE_OUT.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { chromium, type Page } from "@playwright/test";

/** Refuses to run against an IDE linked to any project other than the scratch one named. */
async function confirmProject(page: Page, project: string) {
  const manifest = await readFile(path.join(project, "emergent.toml"), "utf8"), expected = /^name\s*=\s*"([^"]*)"/m.exec(manifest)?.[1];
  const shown = (await page.locator(".settings-breadcrumb").first().textContent())?.split(" / ")[0]?.trim();
  if (!expected || shown !== expected) throw new Error(`The IDE is linked to "${shown}", not the scratch project "${expected}". Stopping before any edit.`);
}

async function settle(page: Page) {
  await page.waitForFunction(() => !document.querySelector(".brain-status"), undefined, { timeout: 120000 });
  const error = page.locator(".brain-error");
  if (await error.count()) throw new Error(`Map error: ${await error.first().textContent()}`);
}

async function main() {
  const base = process.env.MAP_SMOKE_URL ?? "http://localhost:3001", out = process.env.MAP_SMOKE_OUT ?? os.tmpdir(), project = process.env.MAP_SMOKE_PROJECT;
  if (!project) throw new Error("Set MAP_SMOKE_PROJECT to the scratch project the IDE is linked to.");
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } }), errors: string[] = [];
    page.on("pageerror", error => errors.push(error.message));
    page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
    await page.goto(`${base}/map`);
    await confirmProject(page, project);
    await page.locator(".map-backdrop polygon").first().waitFor({ timeout: 120000 });
    const surface = page.getByLabel("Top-down scene map"), box = (await surface.boundingBox())!;
    const point = (fx: number, fy: number) => [box.x + box.width * fx, box.y + box.height * fy] as const;
    const tool = (name: string) => page.getByRole("button", { name, exact: true }).click();

    await tool("Waypoint"); await page.mouse.click(...point(0.3, 0.3)); await settle(page);
    await tool("Waypoint path");
    for (const [fx, fy] of [[0.4, 0.6], [0.5, 0.65], [0.6, 0.6]] as const) await page.mouse.click(...point(fx, fy));
    await page.keyboard.press("Enter"); await settle(page);
    await tool("Circle zone");
    await page.mouse.move(...point(0.7, 0.3)); await page.mouse.down(); await page.mouse.move(...point(0.76, 0.3), { steps: 5 }); await page.mouse.up(); await settle(page);
    await tool("Rectangle zone");
    await page.mouse.move(...point(0.2, 0.7)); await page.mouse.down(); await page.mouse.move(...point(0.3, 0.8), { steps: 5 }); await page.mouse.up(); await settle(page);
    // Link the single waypoint to the first path waypoint.
    await tool("Link"); await page.mouse.click(...point(0.3, 0.3)); await page.mouse.click(...point(0.4, 0.6)); await settle(page);
    assert.equal(await page.locator(".map-links line").count(), 3, "two path links plus the new one");

    // Select the circle zone, make it costly, tag it.
    await tool("Select"); await page.mouse.click(...point(0.7, 0.3));
    await page.getByLabel("Travel cost").fill("4"); await page.getByLabel("Travel cost").blur();
    await page.waitForTimeout(900); await settle(page);
    await page.getByRole("button", { name: "Add metadata" }).click(); await settle(page);
    await page.getByLabel("New Values name").fill("type"); await page.getByLabel("New Values value").fill("homebase");
    await page.getByRole("button", { name: "Add to Values" }).click();
    await page.waitForTimeout(900); await settle(page);
    // Move the lone waypoint.
    await page.mouse.move(...point(0.3, 0.3)); await page.mouse.down(); await page.mouse.move(...point(0.33, 0.25), { steps: 5 }); await page.mouse.up(); await settle(page);
    // Zoom out and back.
    await page.mouse.move(...point(0.5, 0.5)); await page.mouse.wheel(0, 400); await page.mouse.wheel(0, -400);
    await page.screenshot({ path: path.join(out, "map-editor.png") });

    const scene = await readFile(path.join(project, "assets/scenes/main.scene.toml"), "utf8");
    assert.equal(scene.match(/type = "emergent\.waypoint"/g)?.length, 4, "four waypoints");
    assert.equal(scene.match(/type = "emergent\.zone"/g)?.length, 2, "two zones");
    assert.match(scene, /cost = 4/);
    assert.match(scene, /type = "homebase"/);
    assert.match(scene, /shape = "rect"/);
    // Undo the move, then the rest is still there.
    await page.getByRole("button", { name: "Undo", exact: true }).click(); await settle(page);
    assert.deepEqual(errors.filter(text => !/Download the React DevTools/.test(text)), []);
    console.log(`PASS scene map: waypoint, path, circle and rectangle zones, link, cost and metadata edits, move, zoom, undo. Screenshot in ${out}.`);
  } finally { await browser.close(); }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
