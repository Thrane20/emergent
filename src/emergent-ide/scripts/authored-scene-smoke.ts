import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { readFile, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium, expect } from "@playwright/test";

// Uses a running IDE. Only uniquely named smoke assets are created and removed.
async function main() {
  const root = path.resolve(process.env.EMERGENT_PROJECT_PATH || "../../projects/first_project");
  const id = `loader-smoke-${randomUUID().slice(0,8)}`;
  const scenePath = `assets/scenes/${id}.scene.toml`;
  let assetPath: string | undefined;
  await writeFile(path.join(root,scenePath),`schema_version = 1\nid = "${id}"\nentities = []\n`,{flag:"wx"});
  const browser = await chromium.launch({ channel:process.env.PLAYWRIGHT_CHANNEL || "chrome",headless:true });
  try {
    const page = await browser.newPage({viewport:{width:1440,height:1000}});
    await page.goto(process.env.EMERGENT_IDE_BASE_URL || "http://localhost:3000");
    await page.getByLabel("Scene viewer",{exact:true}).selectOption(scenePath);
    await page.getByRole("button",{name:"Add asset",exact:false}).click();
    const modal = page.getByRole("dialog");
    await modal.locator("summary").filter({hasText:"Objects"}).click();
    await modal.getByRole("button",{name:/^Cube\b/}).click();
    await modal.getByRole("textbox",{name:"Display name",exact:true}).fill(id);
    await expect(modal.getByRole("checkbox",{name:"Include in selected scene"})).toBeChecked();
    const created = page.waitForResponse(r => r.url().endsWith("/api/assets") && r.request().method() === "POST");
    await modal.getByRole("button",{name:"Create asset",exact:true}).click();
    const response = await created;
    const asset = await response.json();
    assert.equal(response.status(),201,JSON.stringify(asset));
    assetPath = asset.path;
    assert.equal(asset.sceneError,undefined,asset.sceneError);
    assert.equal(assetPath,`assets/entities/${asset.id}.entity.toml`);
    assert.ok((await readFile(path.join(root,scenePath),"utf8")).includes(assetPath!));
    async function load() {
      const result = page.waitForResponse(r => r.url().endsWith("/api/inspection") && r.request().method()==="POST",{timeout:120000});
      await page.getByRole("button",{name:"Load",exact:true}).click();
      return result;
    }
    let rendered = await load();
    assert.equal(rendered.status(),200,rendered.ok() ? "" : await rendered.text());
    const frame = page.getByLabel("Live Rust inspection viewport",{exact:true});
    await expect.poll(async()=>Number(await frame.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(1);
    const before = await frame.screenshot();
    const originalFrame = await frame.getAttribute("data-session");
    const target = path.join(root,assetPath!);
    const original = await readFile(target,"utf8");
    const changed = original.replace("translation = [0.0, 0.0, 0.0]","translation = [2.0, 0.0, 0.0]")
      .replace("half_extent = 0.5","half_extent = 1.0").replace("color = [1.0, 1.0, 1.0, 1.0]","color = [1.0, 0.0, 0.0, 1.0]");
    await writeFile(target,changed);
    rendered = await load();
    assert.equal(rendered.status(),200);
    await expect(frame).not.toHaveAttribute("data-session",originalFrame!);
    await expect.poll(async()=>Number(await frame.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(1);
    assert.notDeepEqual(await frame.screenshot(),before,"asset edits must change the Rust-rendered frame");
    await expect(page.locator(".scene-load-error")).toHaveCount(0);
    await writeFile(target,changed.replace("half_extent = 1.0","half_extent = -1.0"));
    rendered = await load();
    assert.equal(rendered.status(),200);
    await expect(page.locator(".inspection-error")).toContainText(assetPath!,{timeout:120000});
    console.log("Authored scene smoke passed: IDE creation/inclusion, real Rust asset edit render, invalid asset reports a native inspection error.");
  } finally {
    await browser.close();
    await unlink(path.join(root,scenePath));
    if (assetPath) await unlink(path.join(root,assetPath));
  }
}
main().catch(error => { console.error(error); process.exitCode=1; });
