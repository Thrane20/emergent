import assert from "node:assert/strict";
import { chromium, expect } from "@playwright/test";

// Requires a running IDE and its linked project's native inspection worker.
async function main() {
  const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || "chrome", headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    await page.goto(process.env.EMERGENT_IDE_BASE_URL || "http://localhost:3000");
    const frame = page.getByLabel("Live Rust inspection viewport", { exact: true });
    await expect.poll(async()=>Number(await frame.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(1);
    assert.equal(await frame.getAttribute("data-tick"),"0");
    await expect(page.getByRole("button",{name:"Inspect",exact:true})).toHaveCount(0);
    const previous = await frame.getAttribute("data-session");
    const stopped = page.waitForResponse(r=>r.url().endsWith("/api/inspection")&&r.request().method()==="DELETE");
    await page.getByRole("button",{name:"Source",exact:true}).click();
    assert.equal((await stopped).status(),200);
    await expect(frame).toHaveCount(0);
    await page.getByRole("button",{name:"Scene",exact:true}).click();
    await expect.poll(async()=>Number(await frame.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(1);
    assert.notEqual(await frame.getAttribute("data-session"),previous);
    for (let i=0;i<2;i++) {
      const old=await frame.getAttribute("data-session");
      await page.getByRole("button",{name:"Source",exact:true}).click();
      await page.getByRole("button",{name:"Scene",exact:true}).click();
      await expect.poll(async()=>Number(await frame.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(1);
      assert.notEqual(await frame.getAttribute("data-session"),old);
    }
    await page.route("**/api/inspection",route=>route.request().method()==="POST"?route.fulfill({status:400,json:{error:"Unsupported scene builder (smoke)"}}):route.continue());
    await page.getByRole("button",{name:"Load",exact:true}).click();
    await expect(page.locator(".scene-load-error")).toContainText("Unsupported scene builder");
    await expect(page.getByRole("button",{name:"Load",exact:true})).toBeEnabled();
    console.log("Scene tab smoke passed: automatic native inspection, stopped on exit, fresh session on return, visible failed reload and retry.");
  } finally { await browser.close(); }
}
main().catch(error=>{console.error(error);process.exitCode=1;});
