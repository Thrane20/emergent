import assert from "node:assert/strict";
import {randomUUID} from "node:crypto";
import {appendFile,readFile,writeFile,unlink} from "node:fs/promises";
import path from "node:path";
import {chromium,expect} from "@playwright/test";

async function main(){
 const base=process.env.EMERGENT_IDE_BASE_URL||"http://localhost:3000",root=process.cwd();
 const token=randomUUID(),marker=`:root { --emergent-dev-stability: "${token}"; }`,css=path.join(root,"src/app/noise-filter.css"),probe=path.join(root,`.next/dev/${randomUUID()}.json`),log=path.join(root,".next/dev/logs/next-development.log");
 const browser=await chromium.launch({channel:process.env.PLAYWRIGHT_CHANNEL||"chrome",headless:true});let edited=false;
 try{
  const page=await browser.newPage({viewport:{width:1600,height:1100}}),errors:string[]=[];let navigations=0;
  page.on("framenavigated",frame=>{if(frame===page.mainFrame())navigations++;});page.on("pageerror",error=>errors.push(error.message));
  await page.goto(base,{waitUntil:"domcontentloaded"});await expect(page.getByRole("heading",{name:"Scene graph",exact:true})).toBeVisible({timeout:30000});
  await page.waitForTimeout(3000);const settled=navigations;
  await page.waitForTimeout(5000);assert.equal(navigations,settled,"idle development must not keep refreshing");
  await writeFile(probe,JSON.stringify({generated:"bg-red-500"}));await page.waitForTimeout(3000);
  assert.equal(navigations,settled,"generated cache writes must not reload the page");
  const compiled=async()=>((await readFile(log,"utf8")).match(/Compiled in/g)??[]).length;
  await appendFile(css,`\n${marker}\n`);edited=true;
  await expect.poll(()=>page.evaluate(()=>getComputedStyle(document.documentElement).getPropertyValue("--emergent-dev-stability")),{timeout:15000}).toContain(token);await page.waitForTimeout(3000);
  const after=await compiled();await page.waitForTimeout(3000);assert.equal(await compiled(),after,"one CSS edit must settle instead of recompiling forever");
  assert.equal(navigations,settled,"CSS hot updates must preserve the current document");assert.deepEqual(errors,[]);
  console.log("Dev stability smoke passed: page loads, idle/cache writes stay stable, and one CSS edit hot-updates and settles without reloads.");
 }finally{
  if(edited){const current=await readFile(css,"utf8");await writeFile(css,current.replace(`\n${marker}\n`,""));}
  await unlink(probe).catch(error=>{if(error.code!=="ENOENT")throw error;});await browser.close();
 }
}
main().catch(error=>{console.error(error);process.exitCode=1;});
