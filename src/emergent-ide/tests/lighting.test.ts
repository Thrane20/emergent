import assert from "node:assert/strict";
import test from "node:test";
import {mkdtemp,mkdir,writeFile,rm} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {parse,stringify} from "smol-toml";
import {DEFAULT_LIGHTING,lightingSettings} from "../src/lib/lighting-settings";
import {readSceneLighting,replaceSceneLighting,saveSceneLighting} from "../src/lib/scene-lighting";
import type {ProjectSnapshot,ProjectFile} from "../src/lib/project";
import {imageLibrarySettings,projectImages,DEFAULT_SPRITE_SHEET} from "../src/lib/image-library";
import {textureRequest,texturePrompt} from "../src/lib/texture-job-types";
const source='# keep scene\nschema_version=1\nid="main"\n[[entities]]\nid="ground" # keep name\nasset="assets/ground.entity.toml"\n[entities.transform]\nscale=[2,3,4]\n';
test("lighting defaults and input bounds match the native scene contract",()=>{
 assert.deepEqual(lightingSettings(undefined),DEFAULT_LIGHTING);
 for(const value of [null,[],{ambient_color:[1,2,3]},{ambient_color:[0,0]},{ambient_intensity:NaN},{ambient_intensity:-1},{ambient_intensity:11},{skybox_rotation:361},{skybox_intensity:Infinity},{skybox:"../secret.png"},{skybox:"https://example.com/sky.png"},{skybox:"assets/sky.hdr"},{other:1}])assert.throws(()=>lightingSettings(value));
 assert.equal(lightingSettings({ambient_intensity:0,skybox_intensity:0,skybox_rotation:360,skybox:"assets/sky.png"}).skybox_rotation,360);
});
test("lighting table edits preserve unrelated scene data and comments",()=>{
 const settings={...DEFAULT_LIGHTING,ambient_color:[.4,.5,.6],skybox:"assets/sky.png",skybox_rotation:90};
 const added=replaceSceneLighting(source,settings),parsed=parse(added);assert.deepEqual(parsed.environment,parse(stringify({environment:settings})).environment);assert.deepEqual(parsed.entities,parse(source).entities);assert.ok(added.startsWith(source.trimEnd()));assert.match(added,/# keep name/);
 const replaced=replaceSceneLighting(added,{...settings,skybox:null});assert.equal((parse(replaced).environment as Record<string,unknown>).skybox,undefined);assert.deepEqual(parse(replaced).entities,parsed.entities);
 const beforeEntities='schema_version=1\nid="main"\n[environment]\nambient_intensity=2\n'+source.slice(source.indexOf('[[entities]]'));assert.deepEqual(parse(replaceSceneLighting(beforeEntities,settings)).entities,parsed.entities);
 assert.throws(()=>replaceSceneLighting('schema_version=1\nid="main"\nentities=[]\nenvironment={ambient_intensity=1}\n',settings),/inline/);
});
test("lighting revisions include scene and panorama changes and reject stale saves",async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),"scene-lighting-"));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(path.join(root,"assets"));
 const scenePath="assets/main.scene.toml";await writeFile(path.join(root,scenePath),replaceSceneLighting(source,{...DEFAULT_LIGHTING,skybox:"assets/sky.png"}));await writeFile(path.join(root,"assets/sky.png"),"panorama bytes");
 const project:ProjectSnapshot={root,id:"test",name:"Test",entryScene:scenePath,assetRoots:["assets"],files:[{path:scenePath,name:"main.scene.toml"} as ProjectFile],entities:[],connections:[],outputs:[]};
 const first=await readSceneLighting(project,scenePath);await writeFile(path.join(root,"assets/sky.png"),"new panorama bytes");assert.notEqual((await readSceneLighting(project,scenePath)).revision,first.revision);
 await assert.rejects(saveSceneLighting(project,{path:scenePath,revision:first.revision,settings:DEFAULT_LIGHTING}),/changed externally/);await assert.rejects(readSceneLighting(project,"../main.scene.toml"),/Choose a scene/);
});
test("skyboxes share the image library with a distinct type and panoramic generation prompt",()=>{
 const settings=imageLibrarySettings({kind:"skybox",sprite:DEFAULT_SPRITE_SHEET});assert.equal(settings.kind,"skybox");
 const image={path:"assets/sky.png",name:"sky.png",modifiedAt:"2026-10-30"},sidecar={path:"assets/sky.png.library.json",name:"sky.png.library.json",modifiedAt:"2026-10-30",content:JSON.stringify({schema_version:1,...settings})};
 const images=projectImages({assetRoots:["assets"],files:[image,sidecar]} as ProjectSnapshot);assert.equal(images[0].kind,"skybox");assert.equal(images[0].sprite,undefined);
 const request=textureRequest({name:"Blue sky",prompt:"Evening clouds",seed:42,count:1,tileFriendly:false,kind:"skybox"});assert.match(texturePrompt(request),/equirectangular/);assert.match(texturePrompt(request),/2:1/);
});
