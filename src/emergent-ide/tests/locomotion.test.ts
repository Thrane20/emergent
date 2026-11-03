import assert from "node:assert/strict";
import test from "node:test";
import { changeType, locomotionCatalogue, parseLocomotionTrace, parseProfile, serializeProfile, type LocomotionType } from "../src/lib/locomotion";

const wheeled: LocomotionType = { id: "wheeled", label: "Wheeled", category: "Ground", status: "available", description: "", manoeuvres: ["steer"], rig: "wheels", fields: [{ name: "max_speed", label: "Max speed", kind: "number", help: "" }, { name: "wheelbase", label: "Wheelbase", kind: "number", help: "" }] };

test("unpowered physical boundary stays distinct from available flight types", () => {
  const boundary: LocomotionType = { ...wheeled, id: "drone_physics", label: "Drone (physics boundary)", status: "boundary", manoeuvres: [], fields: [] };
  const catalogue = locomotionCatalogue({ categories: ["Ground"], types: [wheeled, boundary] });
  assert.equal(catalogue.types[1].status, "boundary");
  assert.deepEqual(catalogue.types.filter(type => type.status === "available").map(type => type.id), ["wheeled"]);
});

test("profiles round-trip and switching type drops foreign fields", () => {
  const profile = { id: "car", name: "Car", description: "Small", type: "drone", fields: { max_speed: 9, max_tilt: 20 } };
  const text = serializeProfile(profile);
  assert.match(text, /kind = "locomotion"/);
  assert.deepEqual(parseProfile(text), profile);
  assert.deepEqual(changeType(profile, wheeled), { ...profile, type: "wheeled", fields: { max_speed: 9 } });
  assert.throws(() => parseProfile("schema_version = 1\nid = 'x'\n"), /needs/);
});

test("preview traces parse frames and reject malformed output", () => {
  const frame = "1 2 3 0.1 0.2 0.3 4 0.5 1 0 0 0 1 0";
  const trace = parseLocomotionTrace(`locomotion-preview 1 type=wheeled frames=2 frame_seconds=0.5\ncourse 10 0 -10\n${frame}\n${frame.replace(/ 0$/, " 1")}\n`);
  assert.equal(trace.type, "wheeled");
  assert.deepEqual(trace.course, [{ x: 10, y: 0, z: -10 }]);
  assert.equal(trace.frames[0].steer, 0.5);
  assert.equal(trace.frames[1].landed, true);
  assert.throws(() => parseLocomotionTrace("locomotion-preview 1 type=x frames=1 frame_seconds=1\ncourse\n1 2 3\n"), /malformed/);
  // Physical aircraft add a 15th column: the flight phase.
  const flying = parseLocomotionTrace(`locomotion-preview 1 type=fixed_wing_physics frames=1 frame_seconds=0.1\ncourse\n${frame} 9\n`);
  assert.equal(flying.frames[0].phase, "Final");
  assert.throws(() => parseLocomotionTrace(`locomotion-preview 1 type=fixed_wing_physics frames=1 frame_seconds=0.1\ncourse\n${frame} 99\n`), /malformed/);
  assert.throws(() => parseLocomotionTrace(`locomotion-preview 1 type=x frames=3 frame_seconds=1\ncourse\n${frame}\n`), /incomplete/);
});
