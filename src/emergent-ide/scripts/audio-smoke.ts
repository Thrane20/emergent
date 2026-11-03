// Audio editor smoke against a running IDE (default http://localhost:3001) whose project has at
// least one clip: tile waveforms, waveform, preview, trim + save through the engine, add to scene, Generate tab.
// No paid calls. It edits the linked project, so AUDIO_SMOKE_PROJECT must name that scratch project. Screenshots go to AUDIO_SMOKE_OUT (default: the OS temp folder).
import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import {readFile} from "node:fs/promises";
import {chromium,type Page} from "@playwright/test";

/** Refuses to run against an IDE linked to any project other than the scratch one named. */
async function confirmProject(page: Page, project: string) {
  const manifest = await readFile(path.join(project, "emergent.toml"), "utf8"), expected = /^name\s*=\s*"([^"]*)"/m.exec(manifest)?.[1];
  const shown = (await page.locator(".settings-breadcrumb").first().textContent())?.split(" / ")[0]?.trim();
  if (!expected || shown !== expected) throw new Error(`The IDE is linked to "${shown}", not the scratch project "${expected}". Stopping before any edit.`);
}

async function main(){
 const base=process.env.AUDIO_SMOKE_URL??"http://localhost:3001",out=process.env.AUDIO_SMOKE_OUT??os.tmpdir();
 const browser=await chromium.launch({args:["--autoplay-policy=no-user-gesture-required"]});
 try{
  const page=await browser.newPage({viewport:{width:1440,height:900}}),errors:string[]=[];
  page.on("console",message=>{if(message.type()==="error")errors.push(message.text());});
  page.on("pageerror",error=>errors.push(error.message));
  await page.goto(`${base}/audio`);
  const project=process.env.AUDIO_SMOKE_PROJECT;if(!project)throw new Error("Set AUDIO_SMOKE_PROJECT to the scratch project the IDE is linked to.");
  await confirmProject(page,project);
  const firstClip=page.getByRole("list",{name:"Saved audio clips"}).getByRole("button",{name:/^Open /}).first();
  await firstClip.waitFor();await firstClip.click();
  // Every tile swaps its category icon for the engine's waveform outline.
  await page.waitForFunction(()=>{const tiles=document.querySelectorAll('[aria-label="Saved audio clips"] .audio-thumbnail');return tiles.length>0&&[...tiles].every(tile=>tile.querySelector(".audio-thumbnail-wave"));},undefined,{timeout:120000});
  await page.getByLabel("Waveform with trim and loop handles").waitFor({timeout:30000});
  await page.getByText(/mono|stereo/).first().waitFor();
  await page.screenshot({path:path.join(out,"audio-editor.png")});
  await page.getByRole("button",{name:"▶ Play"}).click();
  await page.locator(".audio-playhead").waitFor({state:"attached",timeout:5000});
  await page.getByRole("button",{name:"■ Stop"}).click();
  // Trim the end by a tenth of a second with the keyboard handle, then save through the engine.
  const end=page.getByRole("slider",{name:"Trim end"});await end.focus();for(let i=0;i<10;i++)await end.press("ArrowLeft");
  await page.getByText("UNSAVED").waitFor();
  await page.getByRole("button",{name:"Save clip"}).click();
  await page.getByText(/Saved\. The engine plays/).or(page.locator(".brain-error")).first().waitFor({timeout:120000});
  const alert=page.locator(".brain-error");if(await alert.count())throw new Error(`Save refused: ${await alert.first().textContent()}`);
  await page.screenshot({path:path.join(out,"audio-saved.png")});
  await page.getByRole("button",{name:"Add sound point"}).click();
  await page.getByRole("link",{name:"Open scene ↗"}).waitFor({timeout:60000});
  await page.getByRole("tab",{name:"Generate"}).click();
  await page.getByRole("tab",{name:"Import"}).click();
  await page.getByRole("button",{name:"New blank clip"}).click();
  await page.getByRole("tab",{name:"Speech"}).click();
  await page.getByLabel("Line to speak").waitFor();
  await page.getByLabel("Speech generator model").waitFor();
  await page.screenshot({path:path.join(out,"audio-generate.png")});
  assert.deepEqual(errors.filter(text=>!/Download the React DevTools/.test(text)),[]);
  console.log(`PASS audio editor: waveform, preview, keyboard trim + engine save, add to scene, generate tab. Screenshots in ${out}.`);
 }finally{await browser.close();}
}
main().catch(error=>{console.error(error);process.exitCode=1;});
