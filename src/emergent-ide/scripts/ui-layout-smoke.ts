import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { setTimeout } from "node:timers/promises";
import { chromium, expect } from "@playwright/test";
import { parse, stringify } from "smol-toml";

// Run after EMERGENT_NEXT_DIST_DIR=.next-ui-layout-build pnpm build. All writes use a disposable project.
async function main() {
  const port = 3001, base = `http://localhost:${port}`;
  const probe = net.createServer(); probe.listen(port, "127.0.0.1"); await once(probe, "listening");
  await new Promise<void>((resolve, reject) => probe.close(error => error ? reject(error) : resolve()));
  const directory = await mkdtemp(path.join(os.tmpdir(), "emergent-ui-layout-browser-"));
  const root = path.join(directory, "game"), data = path.join(directory, "data"), foundation = path.resolve(process.cwd(), "../..");
  let server: ReturnType<typeof spawn> | undefined;
  let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined;
  let output = "", session:string|null=null,projectId="";
  try {
    await cp(path.join(foundation, "templates/standard"), root, { recursive: true });
    const cargo = parse(await readFile(path.join(root, "Cargo.toml"), "utf8"));
    (cargo.dependencies as Record<string, { path: string }>).emergent.path = foundation; cargo.workspace = {};
    await writeFile(path.join(root, "Cargo.toml"), stringify(cargo));
    projectId=parse(await readFile(path.join(root,"emergent.toml"),"utf8")).id as string;
    await cp(path.join(foundation, "Cargo.lock"), path.join(root, "Cargo.lock"));
    await cp(path.join(foundation,"tests/fixtures/ui-nested-layout.ui.toml"),path.join(root,"assets/ui/nested.ui.toml"));
    await mkdir(data);
    await writeFile(path.join(data, "ai-provider.json"), JSON.stringify({ provider: "anthropic", providers: { anthropic: { apiKey: "smoke-test-key" } } }));
    server = spawn(process.execPath, ["node_modules/next/dist/bin/next", "start", "--hostname", "127.0.0.1", "--port", String(port)], {
      env: { ...process.env, EMERGENT_PROJECT_PATH: root, EMERGENT_IDE_DATA_DIR: data, EMERGENT_NEXT_DIST_DIR: ".next-ui-layout-build" },
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
    browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL ?? "chrome", headless: true });
    const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } }), errors: string[] = [];
    page.setDefaultTimeout(120000);
    page.on("pageerror", error => errors.push(error.message));
    await page.route("**/api/fal-balance*", route => route.fulfill({ json: { state: "no-key", balance: 0, currency: "USD", message: "Smoke test", checkedAt: null } }));
    await page.goto(`${base}/ui?asset=assets/ui/nested.ui.toml`);
    for(const name of ["Horizontal layout","Vertical layout","Grid layout"])await expect(page.getByRole("button",{name,exact:false}).filter({has:page.locator("b",{hasText:name})})).toBeVisible({timeout:120000});
    const svg=page.locator("svg.ui-canvas");await expect(svg.locator("[data-node]")).toHaveCount(10);
    const expected=parse(await readFile(path.join(foundation,"tests/fixtures/ui-nested-layout-rects.toml"),"utf8"));
    for(const [id,values] of Object.entries(expected).filter(([id])=>["root","row","col","grid"].includes(id))){
      const box=await svg.locator(`[data-node="${id}"]`).evaluate(element=>{const r=(element as SVGGraphicsElement).getBBox();return [r.x,r.y,r.width,r.height];});
      assert.deepEqual(box,values,id);
    }
    await page.getByLabel("Grid layout padding",{exact:true}).fill("12");
    await page.getByLabel("Grid layout grid columns",{exact:true}).fill("3");
    await page.locator(".ui-outline-row").getByRole("button",{name:"row horizontal",exact:true}).click();
    await page.getByLabel("Horizontal layout column span",{exact:true}).fill("2");
    await page.getByLabel("Horizontal layout row span",{exact:true}).fill("2");
    await page.getByLabel("Horizontal layout width",{exact:true}).fill("auto");
    await page.getByLabel("Horizontal layout height",{exact:true}).fill("auto");
    const [saved]=await Promise.all([page.waitForResponse(r=>r.url().endsWith("/api/ui")&&r.request().method()==="PUT",{timeout:120000}),page.getByRole("button",{name:"Save UI",exact:true}).click()]);
    assert.ok(saved.ok(),await saved.text());
    await page.reload();await expect(page.getByLabel("Grid layout padding",{exact:true})).toHaveValue("12");
    for(const name of ["Add screen overlay","Add world panel"]){
      const [placed]=await Promise.all([page.waitForResponse(r=>r.url().endsWith("/api/ui")&&r.request().method()==="POST",{timeout:120000}),page.getByRole("button",{name,exact:true}).click()]);assert.ok(placed.ok(),await placed.text());
      await expect(page.getByRole("button",{name,exact:true})).toBeEnabled({timeout:120000});
    }
    const scene=parse(await readFile(path.join(root,"assets/scenes/main.scene.toml"),"utf8"));
    const surfaces=(scene.entities as {components?:{type:string;space:string}[]}[]).flatMap(e=>e.components??[]).filter(c=>c.type==="emergent.ui");assert.deepEqual(surfaces.map(c=>c.space),["screen","world"]);
    await page.screenshot({path:"/tmp/emergent-ui-layouts.png",fullPage:true});
    await page.goto(`${base}/?scene=assets/scenes/main.scene.toml`);
    const viewport=page.getByLabel("Live Rust inspection viewport",{exact:true});
    await expect.poll(async()=>Number(await viewport.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(2);session=await viewport.getAttribute("data-session");
    await page.screenshot({path:"/tmp/emergent-ui-runtime-scene.png",fullPage:true});
    assert.deepEqual(errors,[]);
    console.log("Nested UI layout smoke passed: palette, shared native/preview rectangles, padding edit, native validated save/reopen grid spans, auto dimensions, screen/world placement and native scene frames. Port 3001 closed on exit.");
  } catch (error) { if(browser)await browser.contexts()[0]?.pages()[0]?.screenshot({path:"/tmp/emergent-ui-layout-failure.png",fullPage:true});console.error(output);throw error; }
  finally {
    if(session)await fetch(`${base}/api/inspection`,{method:"DELETE",headers:{"Content-Type":"application/json"},body:JSON.stringify({projectId,id:session})}).catch(()=>{});
    await browser?.close();
    if (server && server.exitCode === null) { const stopped = once(server, "exit"); server.kill("SIGTERM"); await stopped; }
    await rm(directory, { recursive: true, force: true });
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
