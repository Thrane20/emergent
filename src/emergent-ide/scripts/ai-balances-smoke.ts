import assert from "node:assert/strict";
import { chromium, expect } from "@playwright/test";
import type { AIBalance } from "../src/lib/ai-balance-types";

async function main() {
  const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL ?? "chrome", headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    const errors: string[] = [];
    page.on("pageerror", error => errors.push(error.message));
    let reads = 0, refreshes = 0, configured = true, failed = false;
    const entries: AIBalance[] = [
      { provider: "anthropic", label: "Anthropic", state: "unsupported", balance: null, billingUrl: "https://platform.claude.com/settings/billing", message: "Open billing to check remaining credit.", checkedAt: null },
      { provider: "openai-codex", label: "OpenAI", state: "unsupported", balance: null, billingUrl: "https://platform.openai.com/settings/organization/billing/overview", message: "Open billing to check remaining credit.", checkedAt: null },
      { provider: "vercel", label: "Vercel", state: "unchecked", balance: null, billingUrl: "https://vercel.com/ai-gateway", message: "Refresh to check credit.", checkedAt: null },
    ];
    await page.route("**/api/fal-balance*", route => route.fulfill({ json: { state: "ready", balance: 8.76, currency: "USD", message: "fal credit", checkedAt: null } }));
    await page.route("**/api/ai-balances", async route => {
      const refresh = route.request().method() === "POST";
      if (refresh) refreshes++; else reads++;
      await route.fulfill({ json: { balances: configured ? entries.map(entry => entry.provider !== "vercel" || !refresh ? entry : {
        ...entry, state: failed ? "unavailable" : "ready", balance: failed ? null : 4.99, checkedAt: "2026-11-03T00:00:00Z", message: failed ? "Unable to read balance. Refresh to retry." : "Available Gateway credit in USD.",
      }) : [] } });
    });
    await page.goto(`${process.env.EMERGENT_NAV_TEST_URL ?? "http://localhost:3001"}/settings`);
    const group = page.getByRole("group", { name: "AI balances", exact: true });
    const refresh = page.getByRole("button", { name: "Refresh AI balances", exact: true });
    const vercel = page.getByRole("link", { name: "Vercel balance", exact: true });
    await expect(group).toBeVisible(); await expect(vercel).toContainText("—");
    await expect(page.getByRole("link", { name: "OpenAI balance", exact: true })).toContainText("Billing");
    await expect(page.getByRole("link", { name: "Anthropic balance", exact: true })).toContainText("Billing");
    assert.equal(refreshes, 0); assert.equal(reads, 1);
    assert.ok(await group.evaluate(el => Boolean(el.closest("header")?.querySelector(".fal-balance"))));
    await page.clock.install(); await page.clock.fastForward(180_000);
    await page.evaluate(() => { window.dispatchEvent(new Event("focus")); document.dispatchEvent(new Event("visibilitychange")); });
    assert.equal(refreshes, 0, "time/focus/visibility must never refresh AI balances");
    await refresh.click(); await expect(vercel).toContainText("$4.99"); assert.equal(refreshes, 1);
    await expect(vercel).toHaveAttribute("title", /Checked/);
    await page.screenshot({ path: "/tmp/emergent-ai-balances-desktop.png", fullPage: true });
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    await expect(refresh).toBeInViewport({ ratio: 1 });
    await expect(page.getByRole("button", { name: "fal balance", exact: true })).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: "/tmp/emergent-ai-balances-mobile.png" });
    failed = true; await refresh.click(); await expect(vercel).toContainText("Unavailable"); assert.equal(refreshes, 2);
    failed = false; await refresh.click(); await expect(vercel).toContainText("$4.99"); assert.equal(refreshes, 3);
    await page.evaluate(() => window.dispatchEvent(new Event("ai-settings-changed")));
    await expect(vercel).toContainText("—"); assert.equal(reads, 2); assert.equal(refreshes, 3);
    configured = false;
    await page.evaluate(() => window.dispatchEvent(new Event("ai-settings-changed")));
    await expect(group).toHaveCount(0); assert.equal(refreshes, 3);
    assert.deepEqual(errors, []);
    console.log("PASS shared AI header: configured providers, manual-only refresh, no timer/focus requests, errors/retry, settings invalidation, no-key hiding, desktop/mobile layout. Provider responses mocked; no paid requests.");
  } finally { await browser.close(); }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
