import assert from "node:assert/strict";
import test from "node:test";
import type { ModeValue } from "../src/lib/components";
import { addMode, fieldsFromModes, modesFromFields, modesProblem, moveMode, removeMode, renameMode } from "../src/lib/brain-modes";

const a = "assets/brains/a.brain.toml", b = "assets/brains/b.brain.toml";

test("legacy brain + stack read as always-on modes and stay legacy while plain", () => {
  const modes = modesFromFields({ brain: a, stack: [b], max_speed: 3 });
  assert.deepEqual(modes, [{ name: "main", brain: a, active: true }, { name: "stack1", brain: b, active: true }]);
  assert.deepEqual(fieldsFromModes(modes), { brain: a, stack: [b], modes: null });
  assert.deepEqual(fieldsFromModes(modes.slice(0, 1)), { brain: a, stack: null, modes: null });
});

test("any transition, inactive mode or rename switches to modes, which round-trip", () => {
  const legacy = modesFromFields({ brain: a, stack: [b] });
  const linked = legacy.map((mode, i) => (i === 0 ? { ...mode, on_next: "stack1" } : { ...mode, active: false }));
  const patch: Record<string, unknown> = fieldsFromModes(linked);
  assert.deepEqual(patch, { brain: null, stack: null, modes: [{ name: "main", brain: a, active: true, on_next: "stack1" }, { name: "stack1", brain: b }] });
  assert.deepEqual(modesFromFields({ modes: patch.modes as ModeValue[] }), linked.map(mode => ({ on_next: undefined, on_previous: undefined, keep_running: false, ...mode })));
  assert.ok("modes" in fieldsFromModes(renameMode(legacy, 0, "park")) && fieldsFromModes(renameMode(legacy, 0, "park")).modes !== null);
});

test("renames follow links, removals clear them, moves reorder and new names are unique", () => {
  let modes = addMode([], a);
  assert.deepEqual(modes, [{ name: "mode1", brain: a, active: true }], "the first mode starts active");
  modes = addMode(modes, b);
  assert.deepEqual(modes[1], { name: "mode2", brain: b, active: false });
  modes = modes.map((mode, i) => (i === 0 ? { ...mode, on_next: "mode2" } : { ...mode, on_previous: "mode1" }));
  modes = renameMode(modes, 1, "taxi");
  assert.equal(modes[0].on_next, "taxi");
  modes = renameMode(modes, 0, "park");
  assert.equal(modes[1].on_previous, "park");
  assert.deepEqual(moveMode(modes, 1, -1).map(mode => mode.name), ["taxi", "park"]);
  assert.equal(moveMode(modes, 0, -1), modes, "cannot move past the top");
  const removed = removeMode(modes, 1);
  assert.deepEqual(removed.map(mode => [mode.name, mode.on_next]), [["park", undefined]]);
  assert.equal(addMode([{ name: "mode2", brain: a, active: true }], a)[1].name, "mode3");
});

test("problems are reported before the engine rejects them", () => {
  assert.equal(modesProblem([]), "Add at least one mode.");
  assert.match(modesProblem([{ name: "Park", brain: a, active: true }]), /must be 1–32/);
  assert.match(modesProblem([{ name: "a", brain: a, active: true }, { name: "a", brain: b, active: false }]), /Two modes/);
  assert.match(modesProblem([{ name: "a", brain: "", active: true }]), /Choose a brain/);
  assert.equal(modesProblem([{ name: "park", brain: a, active: true }]), "");
});
