import assert from "node:assert/strict";
import test from "node:test";
import { debugCommand } from "../src/lib/debug-session";

test("debug commands are validated before reaching the game", () => {
  assert.equal(debugCommand({ op: "mouse", dx: 3.5, dy: -2 }, 7), 'request={seq=7, op="mouse", dx=3.5, dy=-2}\n');
  assert.throws(() => debugCommand({ op: "mouse", dx: Number.NaN, dy: 0 }, 1));
  assert.throws(() => debugCommand({ op: "mouse", dx: 1e6, dy: 0 }, 1));
  assert.equal(debugCommand({ op: "keys", held: ["w", "space"] }, 2), 'request={seq=2, op="keys", held=["w","space"]}\n');
  assert.throws(() => debugCommand({ op: "keys", held: ["f1"] }, 3));
  assert.throws(() => debugCommand({ op: "select", id: "bad id\n" }, 4));
});
