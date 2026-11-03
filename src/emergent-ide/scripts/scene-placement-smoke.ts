import { smokeProjectRoot } from "./project-fixture";
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { readFile, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium, expect } from "@playwright/test";
import { parse } from "smol-toml";
import { assetContents, assetTemplate } from "../src/lib/asset-catalog";

async function main() {
  const root = smokeProjectRoot();
  const id = `placement-smoke-${randomUUID().slice(0,8)}`;
  const asset = `assets/entities/${id}.entity.toml`, scene = `assets/scenes/${id}.scene.toml`;
  const assetText = assetContents(assetTemplate("object.cube")!,id);
  const source = `# preserve scene comment\nschema_version=1\nid="${id}"\n[[entities]]\nid="first"\nasset="${asset}"\n[[entities]]\nid="second"\nasset="${asset}"\n[entities.transform]\ntranslation=[-2,0,0]\n`;
  const base = process.env.EMERGENT_IDE_BASE_URL || "http://localhost:3001";
  const browser = await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL || "chrome",headless:true});
  try {
    await writeFile(path.join(root,asset),assetText,{flag:"wx"});
    await writeFile(path.join(root,scene),source,{flag:"wx"});
    const page = await browser.newPage({viewport:{width:1440,height:1100}});
    await page.goto(base);
    await page.getByLabel("Scene viewer",{exact:true}).selectOption(scene);
    await page.getByRole("button",{name:"Load",exact:true}).click();
    const frame = page.getByLabel("Live Rust inspection viewport",{exact:true});
    await expect.poll(async()=>Number(await frame.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(1);
    const before = await frame.screenshot();
    await page.screenshot({path:"/tmp/emergent-transform-inspector-before.png"});
    const list = page.getByRole("region",{name:"Scene instances"});
    await list.getByRole("button",{name:"Select instance first",exact:true}).click();
    const inspector = page.locator(".transform-inspector");
    await expect(inspector.getByLabel("Position X",{exact:true})).toHaveValue("0");
    await inspector.getByLabel("Position X",{exact:true}).fill("2");
    await inspector.getByLabel("Rotation (degrees) Y",{exact:true}).fill("45");
    await inspector.getByLabel("Scale X",{exact:true}).fill("1.5");
    await expect(inspector.getByLabel("Scale Y",{exact:true})).toHaveValue("1.5");
    const oldFrame = Number(await frame.getAttribute("data-frame"));
    const saved = page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement") && r.request().method()==="PUT",{timeout:120000});
    
    const save = await saved;
    assert.equal(save.status(),200,await save.text());
    await expect.poll(async()=>Number(await frame.getAttribute("data-frame")),{timeout:120000}).toBeGreaterThan(oldFrame+2);
    assert.notDeepEqual(await frame.screenshot(),before);
    assert.equal(await readFile(path.join(root,asset),"utf8"),assetText);
    let entries = parse(await readFile(path.join(root,scene),"utf8")).entities as {id:string;transform?:{translation:number[];scale:number[]}}[];
    assert.deepEqual(entries[0].transform?.translation,[2,0,0]);
    assert.deepEqual(entries[1].transform?.translation,[-2,0,0]);
    await page.reload();
    await page.getByLabel("Scene viewer",{exact:true}).selectOption(scene);
    await list.getByRole("button",{name:"Select instance first",exact:true}).click();
    await expect(inspector.getByLabel("Position X",{exact:true})).toHaveValue("2");
    const reset = page.waitForResponse(r=>r.url().endsWith("/api/scenes/placement") && r.request().method()==="PUT",{timeout:120000});
    await inspector.getByRole("button",{name:"Reset to asset defaults",exact:true}).click();
    assert.equal((await reset).status(),200);
    await expect(inspector.getByLabel("Position X",{exact:true})).toHaveValue("0");
    await expect(inspector.getByLabel("Position X",{exact:true})).toBeEnabled({timeout:120000});
    entries = parse(await readFile(path.join(root,scene),"utf8")).entities as typeof entries;
    assert.equal(entries[0].transform,undefined);
    assert.deepEqual(entries[1].transform?.translation,[-2,0,0]);
    const external = await readFile(path.join(root,scene),"utf8") + "\n# external edit\n";
    await writeFile(path.join(root,scene),external);
    await inspector.getByLabel("Position X",{exact:true}).fill("3");
    
    await expect(inspector.getByRole("alert")).toContainText("changed externally");
    assert.equal(await readFile(path.join(root,scene),"utf8"),external);
    await inspector.getByRole("button",{name:"Reload inspector",exact:true}).click();
    await expect(inspector.getByLabel("Position X",{exact:true})).toHaveValue("0");
    // A syntactically valid numeric edit must still pass the actual Rust graph validator.
    const projectId = (await readFile(path.join(root,"emergent.toml"),"utf8")).match(/^id\s*=\s*"([^"]+)"/m)![1];
    const state = await (await page.request.get(`${base}/api/scenes/placement?projectId=${encodeURIComponent(projectId)}&path=${encodeURIComponent(scene)}`)).json();
    const rejected = await page.request.put(`${base}/api/scenes/placement`,{timeout:120000,data:{projectId,path:scene,id:"first",revision:state.revision,transform:{translation:[0,0,0],rotation_degrees:[0,0,0],scale:[1e30,1e30,1e30]}}});
    assert.equal(rejected.status(),400);
    assert.match((await rejected.json()).error,/overflowing|non-invertible/);
    assert.equal(await readFile(path.join(root,scene),"utf8"),external);
    await page.screenshot({path:"/tmp/emergent-transform-inspector.png"});
    console.log("Placement UI smoke passed: independent scene overrides, uniform scale, real Rust render, reload/reset, external conflict and Rust validation rejection.");
  } finally {
    await browser.close();
    for (const file of [scene,asset]) await unlink(path.join(root,file)).catch(error=>{if(error.code!=="ENOENT")throw error;});
  }
}
main().catch(error=>{console.error(error);process.exitCode=1;});
