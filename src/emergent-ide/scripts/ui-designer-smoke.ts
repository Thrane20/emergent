import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { setDefaultResultOrder } from "node:dns";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { setTimeout } from "node:timers/promises";
import { chromium, expect } from "@playwright/test";
import { parse, stringify } from "smol-toml";

// Run after EMERGENT_NEXT_DIST_DIR=.next-ui-build pnpm build. All writes use a disposable project.
async function main() {
  setDefaultResultOrder("ipv4first");
  const port = 3001, base = `http://localhost:${port}`;
  const probe = net.createServer(); probe.listen(port, "127.0.0.1"); await once(probe, "listening");
  await new Promise<void>((resolve, reject) => probe.close(error => error ? reject(error) : resolve()));
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-ui-browser-"));
  const root = path.join(directory, "game"), data = path.join(directory, "data"), foundation = path.resolve(process.cwd(), "../..");
  let server: ReturnType<typeof spawn> | undefined;
  let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined;
  let output = "";
  try {
    await cp(path.join(foundation, "templates/standard"), root, { recursive: true });
    const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8"));
    (cargo.dependencies as Record<string, { path: string }>).emergent.path = foundation; cargo.workspace = {};
    await writeFile(path.join(root, "Cargo.toml"), stringify(cargo));
    await cp(path.join(foundation, "Cargo.lock"), path.join(root, "Cargo.lock"));
    await mkdir(data);
    await writeFile(path.join(data, "ai-provider.json"), JSON.stringify({ provider: "anthropic", providers: { anthropic: { apiKey: "smoke-test-key" } } }));
    server = spawn(process.execPath, ["node_modules/next/dist/bin/next", "start", "--hostname", "127.0.0.1", "--port", String(port)], {
      env: { ...process.env, EMERGENT_PROJECT_PATH: root, EMERGENT_IDE_DATA_DIR: data, EMERGENT_NEXT_DIST_DIR: process.env.EMERGENT_NEXT_DIST_DIR ?? ".next-ui-build" },
      stdio: ["ignore", "pipe", "pipe"],
    });
    server.stdout?.on("data", bytes => { output = (output + bytes).slice(-12000); });
    server.stderr?.on("data", bytes => { output = (output + bytes).slice(-12000); });
    let ready = false;
    for (let attempt = 0; attempt < 120; attempt++) {
      if (server.exitCode !== null) throw new Error(output);
      if (await fetch(`${base}/ui`).then(response => response.ok, () => false)) { ready = true; break; }
      await setTimeout(250);
    }
    assert.ok(ready, output);
    browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL ?? "chrome", headless: true, args:["--host-resolver-rules=MAP localhost 127.0.0.1"] });
    const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } }), errors: string[] = [];
    page.setDefaultTimeout(15000);
    page.on("pageerror", error => errors.push(error.message));
    await page.route("**/api/fal-balance*", route => route.fulfill({ json: { state: "no-key", balance: 0, currency: "USD", message: "Smoke test", checkedAt: null } }));
    // Real native AI validation is covered by ui-assets.test.ts; no provider request leaves this browser.
    await page.route("**/api/chat", async route => {
      const request = route.request().postDataJSON();
      if(request.scope===`project:${request.projectId}`){
        assert.match(request.message,/new, unsaved UI with only the starter panel/);
        const response=await page.request.put(`${base}/api/ui`,{data:{projectId:request.projectId,ui:{id:crypto.randomUUID(),name:"Generated flight HUD",description:"Smoke generation",canvas:[640,480],nodes:[{id:"root",type:"panel",fields:{padding:10}},{id:"altitude",parent:"root",type:"label",fields:{text:"Altitude: 5 m",h:"32px"}}]}}});
        assert.ok(response.ok(),await response.text());
        const saved=await response.json();
        await route.fulfill({json:{text:"Created flight HUD",applied:true,files:[saved.path]}});
        return;
      }
      assert.match(request.scope, /^file:assets\/ui\//);
      const list = await (await page.request.get(`${base}/api/ui?projectId=${request.projectId}`)).json();
      const asset = list.uis.find((item: { path: string }) => `file:${item.path}` === request.scope);
      const open = await (await page.request.get(`${base}/api/ui?projectId=${request.projectId}&path=${asset.path}`)).json();
      const response = await page.request.put(`${base}/api/ui`, { data: { projectId: request.projectId, ...open, ui: { ...open.ui, name: "AI smoke UI" } } });
      assert.ok(response.ok(), await response.text());
      await route.fulfill({ json: { text: "Renamed the UI", applied: true, files: [asset.path] } });
    });
    let chatRequests=0;
    page.on("request",request=>{if(request.url()===`${base}/api/chat`)chatRequests++;});
    await page.goto(`${base}/ui`);
    await expect(page.getByRole("heading", { name: "UI Designer", exact: true })).toBeVisible();
    await expect(page.locator(".settings-topbar .brand")).toBeInViewport({ ratio: 1 });
    const manifest=parse(await readFile(path.join(root,"emergent.toml"),"utf8"));
    const library=await (await page.request.get(`${base}/api/ui?projectId=${manifest.id}`)).json();
    assert.ok(library.catalogue.widgets.length>0);
    await expect(page.locator(".brain-library-node")).toHaveCount(library.catalogue.widgets.length, { timeout: 120000 });
    console.log("Loaded native widget catalogue.");
    await expect(page.getByRole("navigation", { name: "IDE sections" }).getByRole("link", { name: "UI Designer", exact: true })).toHaveAttribute("aria-current", "page");
    await page.getByRole("button", { name: "New blank UI", exact: true }).click();
    await page.getByLabel("UI name", { exact: true }).fill("Browser smoke UI");
    const chat=page.getByRole("region",{name:"Project AI chat"});
    const prompt=chat.getByRole("textbox"),send=chat.getByRole("button",{name:"↑",exact:true});
    await prompt.fill("Create a flight HUD with an altitude label.");
    await expect(send).toBeEnabled();await send.click();
    await expect(page.getByLabel("UI name",{exact:true})).toHaveValue("Generated flight HUD",{timeout:120000});
    await expect(page.getByRole("button",{name:"Open Generated flight HUD",exact:true})).toBeVisible();
    await expect(page.locator('.ui-canvas [data-node="altitude"]')).toBeVisible();
    assert.equal(chatRequests,1);
    await page.getByRole("button",{name:"New blank UI",exact:true}).click();
    await page.getByLabel("UI name",{exact:true}).fill("Browser smoke UI");
    const draft="Create a cockpit options panel.\nUse large buttons, clear labels, and a dark background.";
    for(const viewport of [{width:1366,height:768},{width:1280,height:720}]){
      await page.setViewportSize(viewport);
      await chat.scrollIntoViewIfNeeded();
      await expect(chat).toBeInViewport({ratio:0.99});
      await expect(chat.locator(".chat-header")).toBeInViewport({ratio:1});
      await expect(prompt).toBeInViewport({ratio:1});
      await expect(send).toBeInViewport({ratio:1});
      await expect(prompt).toBeEnabled();
      await prompt.fill(draft);await prompt.press("End");await prompt.press("Shift+Enter");await prompt.pressSequentially("Keep text readable.");
      await expect(prompt).toHaveValue(`${draft}\nKeep text readable.`);
      await expect(send).toBeEnabled();
    }
    const retainedDraft=await prompt.inputValue();
    await page.screenshot({path:"/tmp/emergent-ui-designer-chat-720.png"});
    await page.setViewportSize({width:1600,height:1100});
    await page.locator(".brain-library-node").filter({ hasText: /^Button/ }).click();
    await expect(send).toBeDisabled();await prompt.press("Enter");
    assert.equal(chatRequests,1);
    await page.getByLabel("Button text", { exact: true }).fill("APPLY");
    await page.getByLabel("Button width", { exact: true }).fill("160px");
    await page.getByLabel("Button height", { exact: true }).fill("32px");
    await page.getByLabel("Button x", { exact: true }).fill("20px");
    await page.getByLabel("Button y", { exact: true }).fill("20px");
    await page.getByLabel("Button action", { exact: true }).fill("graphics.apply");
    console.log("Created and edited button.");
    await expect(page.getByRole("button", { name: "graphics.apply", exact: true })).toBeVisible();
    // A round trip back to the drag origin must restore its original offsets.
    const button = page.locator('.ui-canvas [data-node^="button_"]');
    const box = (await button.boundingBox())!;
    await page.mouse.move(box.x + 20, box.y + box.height / 2); await page.mouse.down();
    await page.mouse.move(box.x + 50, box.y + box.height / 2); await page.mouse.move(box.x + 20, box.y + box.height / 2); await page.mouse.up();
    await expect(page.getByLabel("Button x", { exact: true })).toHaveValue("20px");
    await page.getByRole("button", { name: "World", exact: true }).click(); await expect(page.locator(".ui-stage.world")).toBeVisible();
    await page.getByRole("button", { name: "Screen", exact: true }).click();
    const firstSave = page.waitForResponse(response => response.url() === `${base}/api/ui` && response.request().method() === "PUT");
    await page.getByRole("button", { name: "Save UI", exact: true }).click();
    const firstResponse = await firstSave; assert.ok(firstResponse.ok(), await firstResponse.text());
    await expect(page.getByRole("status")).toHaveText("Saved and validated by the engine.", { timeout: 120000 });
    await expect(prompt).toHaveValue(retainedDraft);
    await expect(send).toBeEnabled();
    await page.reload(); await expect(page.getByLabel("UI name", { exact: true })).toHaveValue("Browser smoke UI");
    console.log("Saved and reloaded UI.");
    await page.locator('.ui-outline-row>button').filter({ hasText: /button_/ }).click();
    await expect(page.getByLabel("Button action", { exact: true })).toHaveValue("graphics.apply");
    // Clearing an optional action removes the override instead of saving an invalid empty label.
    await page.getByLabel("Button action", { exact: true }).fill("");
    const clearedSave = page.waitForResponse(response => response.url() === `${base}/api/ui` && response.request().method() === "PUT");
    await page.getByRole("button", { name: "Save UI", exact: true }).click();
    const clearedResponse = await clearedSave; assert.ok(clearedResponse.ok(), await clearedResponse.text());
    await expect(page.getByRole("status")).toHaveText("Saved and validated by the engine.", { timeout: 120000 });
    await page.getByPlaceholder("Describe what to build for Browser smoke UI…").fill("Rename this UI");
    await page.getByTitle("Send request to Codex", { exact: true }).click();
    await expect(page.getByLabel("UI name", { exact: true })).toHaveValue("AI smoke UI", { timeout: 120000 });
    const token=await readFile(path.join(data,"mcp-token"),"utf8").catch(()=>"");
    // Initialise the authenticated HTTP endpoint so it creates this disposable server's token.
    if(!token)await page.request.post(`${base}/api/mcp`,{data:{}});
    const bearer=(await readFile(path.join(data,"mcp-token"),"utf8")).trim();
    let mcpId=0;
    const mcp=async(name:string,args:Record<string,unknown>)=>{
      const response=await page.request.post(`${base}/api/mcp`,{headers:{Authorization:`Bearer ${bearer}`,Accept:"application/json, text/event-stream"},data:{jsonrpc:"2.0",id:++mcpId,method:"tools/call",params:{name,arguments:args}}});
      assert.ok(response.ok(),await response.text());const result=(await response.json()).result;assert.ok(!result.isError,result.content[0].text);return JSON.parse(result.content[0].text);
    };
    const matches=await mcp("find",{query:"AI smoke UI",type:"asset"}),ref=matches.matches[0].ref;
    let current=await mcp("describe",{ref});
    await mcp("edit_ui",{ref,revision:current.revision,name:"External MCP UI"});
    await expect(page.getByLabel("UI name",{exact:true})).toHaveValue("External MCP UI");
    await expect(page.getByRole("button",{name:"Open External MCP UI",exact:true})).toBeVisible();
    await page.getByLabel("UI name", { exact: true }).fill("Unsaved UI");
    current=await mcp("describe",{ref});
    await mcp("edit_ui",{ref,revision:current.revision,description:"Changed externally while the local draft is dirty"});
    await expect(page.locator("main").getByRole("alert")).toHaveText(/Your unsaved edits are kept/);
    await expect(page.getByLabel("UI name",{exact:true})).toHaveValue("Unsaved UI");
    await prompt.fill("Adjust the spacing after saving.");
    await expect(send).toBeDisabled();
    page.once("dialog", dialog => dialog.dismiss());
    await page.getByRole("link", { name: "Project map ↗", exact: true }).click(); await expect(page).toHaveURL(/\/ui\?/);
    await page.screenshot({ path: "/tmp/emergent-ui-designer-desktop.png" });
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    await prompt.scrollIntoViewIfNeeded();await expect(prompt).toBeInViewport({ratio:1});
    await expect(prompt).toBeEnabled();await prompt.fill("Keep this readable on mobile.");
    await page.screenshot({ path: "/tmp/emergent-ui-designer-mobile.png", fullPage: true });
    assert.deepEqual(errors, []);
    console.log("UI Designer smoke passed: unsaved starter generation and automatic opening, native catalogue/save/reload, widget editing, drag, action clearing, world preview, mocked chat reload, external MCP refresh and dirty-draft protection, request retained through first save, desktop/mobile. No paid calls.");
  } catch (error) { if(browser)await browser.contexts()[0]?.pages()[0]?.screenshot({path:"/tmp/emergent-ui-designer-failure.png",fullPage:true});console.error(output);throw error; }
  finally {
    await browser?.close();
    if (server && server.exitCode === null) { const stopped = once(server, "exit"); server.kill("SIGTERM"); await stopped; }
    await rm(directory, { recursive: true, force: true });
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
