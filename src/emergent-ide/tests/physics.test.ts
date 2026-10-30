import assert from "node:assert/strict";
import test from "node:test";
import { parse } from "smol-toml";
import { physicsSettings, replaceInstancePhysics } from "../src/lib/scene-placement";

test("physics edits preserve transforms and other instances; reset removes the override",()=>{
  const source='# scene note\nschema_version=1\nid="main"\n[[entities]]\nid="cube"\nasset="assets/cube.entity.toml"\n[entities.transform]\ntranslation=[0,5,0]\n[[entities]]\nid="ground"\nasset="assets/ground.entity.toml"\n';
  const settings=physicsSettings({body:"dynamic",collider:{half_extents:[0.5,1,2],center:[0,0.5,0]}});
  const first=replaceInstancePhysics(source,"cube",settings);
  const second=replaceInstancePhysics(first,"cube",{...settings,enabled:false});
  const parsed=parse(second).entities as Record<string,unknown>[];
  assert.deepEqual(JSON.parse(JSON.stringify(parsed[0].transform)),{translation:[0,5,0]});
  assert.deepEqual(JSON.parse(JSON.stringify(parsed[0].physics)),{...settings,enabled:false});
  assert.deepEqual(parsed[1],(parse(source).entities as unknown[])[1]);
  assert.deepEqual(parse(replaceInstancePhysics(second,"cube",null)),parse(source));
  assert.ok(first.startsWith('# scene note'));
});
test("physics inputs reject malformed dimensions, nonfinite values and unknown fields",()=>{
  for(const value of [{mass:0},{mass:NaN},{restitution:2},{body:"kinematic"},{collider:{shape:"mesh"}},{collider:{half_extents:[1,-1,1]}},{enabled:"yes"},{surprise:true}])assert.throws(()=>physicsSettings(value));
  assert.equal(physicsSettings(undefined).enabled,false);
  assert.equal(physicsSettings({}).enabled,true);
});
