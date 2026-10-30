import assert from "node:assert/strict";
import test from "node:test";
import {mkdtemp,writeFile,rm} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {readProjectBinding} from "../src/lib/project";
import {inspectionProject} from "../src/lib/object-http";
test("inspection binds the live manifest without scanning files being authored",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"emergent-binding-")),previous=process.env.EMERGENT_PROJECT_PATH;
 t.after(async()=>{if(previous===undefined)delete process.env.EMERGENT_PROJECT_PATH;else process.env.EMERGENT_PROJECT_PATH=previous;await rm(root,{recursive:true,force:true});});
 process.env.EMERGENT_PROJECT_PATH=root;await writeFile(path.join(root,"emergent.toml"),'id="test-project"\nasset_roots=["not-created"]\n');
 assert.deepEqual(await readProjectBinding(),{root,id:"test-project"});assert.deepEqual(await inspectionProject("test-project"),{root,id:"test-project"});
 await assert.rejects(inspectionProject("other-project"),/does not match/);
 await writeFile(path.join(root,"emergent.toml"),'id="replacement-project"\n');await assert.rejects(inspectionProject("test-project"),/does not match/);
});
