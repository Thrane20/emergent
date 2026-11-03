import assert from "node:assert/strict";
import test from "node:test";
import { parse } from "smol-toml";
import { authoredComponents, componentTypes, objectComponentTypes, pairsValue, pairValue, patchComponents } from "../src/lib/components";
import { replaceInstanceComponent } from "../src/lib/scene-placement";

const registry = componentTypes(parse(`[[types]]\nid="emergent.light"\nlabel="Light"\ndescription="d"\napplies_to="light"\n[[types.fields]]\nname="intensity"\nlabel="Intensity"\nkind="number"\nmin=0.0\nhelp="h"\n[[types.fields]]\nname="range"\nlabel="Range"\nkind="number"\nvariants=["point","spot"]\nhelp="h"\n`));

test("registry export parses and only offers types that match the object or are already authored", () => {
  assert.equal(registry[0].fields[1].variants?.join(), "point,spot");
  assert.deepEqual(objectComponentTypes(registry, "light", []).map(type => type.id), ["emergent.light"]);
  assert.deepEqual(objectComponentTypes(registry, "primitive", []), []);
  assert.throws(() => componentTypes({ types: [{ id: "Light", label: "x", applies_to: "light", fields: [] }] }));
});

test("component patches set, reset, disable and drop empty overrides", () => {
  let list = patchComponents([], "emergent.light", { intensity: 2, color: [1, 0, 0] });
  assert.deepEqual(list, [{ type: "emergent.light", enabled: true, fields: { intensity: 2, color: [1, 0, 0] } }]);
  list = patchComponents(list, "emergent.light", { intensity: null });
  assert.deepEqual(list[0].fields, { color: [1, 0, 0] });
  list = patchComponents(list, "emergent.light", { color: null, enabled: false });
  assert.deepEqual(list, [{ type: "emergent.light", enabled: false, fields: {} }], "a disabled component stays even with no fields");
  assert.deepEqual(patchComponents(list, "emergent.light", { enabled: true }), [], "an enabled empty override is removed");
  assert.deepEqual(patchComponents(list, "emergent.light", null), []);
  assert.throws(() => patchComponents([], "emergent.light", { intensity: Number.NaN }));
  assert.throws(() => patchComponents([], "emergent.light", { type: 1 }));
  assert.deepEqual(authoredComponents([{ type: "emergent.follow", target: "leader" }])[0].fields, { target: "leader" }, "entity fields are instance IDs");
  // Objects are only name/value pairs (metadata); anything nested is rejected.
  assert.throws(() => authoredComponents([{ type: "emergent.light", intensity: { bright: { very: true } } }]));
  const added = patchComponents([], "emergent.character_controller", {}, true);
  assert.deepEqual(added, [{ type: "emergent.character_controller", enabled: true, fields: {} }], "added types stay while empty");
  assert.deepEqual(patchComponents(patchComponents(added, "emergent.character_controller", { speed: 6 }, true), "emergent.character_controller", { speed: null }, true), added);
  assert.deepEqual(patchComponents(added, "emergent.character_controller", null, true), []);
  assert.deepEqual(patchComponents([], "emergent.spin", { axis: [0, 0, 1] }, true)[0].fields, { axis: [0, 0, 1] });
});

test("component edits rewrite only the instance's [[entities.components]] tables and keep trailing tables", () => {
  const source = `schema_version = 1\nid = "main"\n\n[[entities]]\nid = "lamp"\nasset = "assets/lamp.entity.toml"\n\n[[entities]]\nid = "cube"\nasset = "assets/cube.entity.toml"\n\n[environment]\nambient_intensity = 0.2\n`;
  const added = replaceInstanceComponent(source, "lamp", "emergent.light", { color: [1, 0, 0], intensity: 3 });
  const plain = (value: unknown) => JSON.parse(JSON.stringify(value));
  const document = plain(parse(added)) as { entities: Record<string, unknown>[]; environment: unknown };
  assert.deepEqual(document.entities[0].components, [{ type: "emergent.light", color: [1, 0, 0], intensity: 3 }]);
  assert.equal(document.entities[1].components, undefined);
  assert.deepEqual(document.environment, { ambient_intensity: 0.2 });
  const disabled = replaceInstanceComponent(added, "lamp", "emergent.light", { enabled: false });
  assert.deepEqual((plain(parse(disabled)) as typeof document).entities[0].components, [{ type: "emergent.light", enabled: false, color: [1, 0, 0], intensity: 3 }]);
  const removed = replaceInstanceComponent(disabled, "lamp", "emergent.light", null);
  assert.deepEqual(parse(removed), parse(source));
  assert.doesNotMatch(removed, /components/);
});

test("pairs fields round-trip and empty text defaults stay defaults", () => {
  const types = componentTypes(parse(`[[types]]\nid="emergent.metadata"\nlabel="Metadata"\ndescription="d"\napplies_to="any"\n[[types.fields]]\nname="values"\nlabel="Values"\nkind="pairs"\nhelp="h"\n[types.fields.default]\n[[types]]\nid="emergent.zone"\nlabel="Zone"\ndescription="d"\napplies_to="any"\n[[types.fields]]\nname="layers"\nlabel="Layers"\nkind="text"\ndefault=""\nhelp="h"`));
  assert.deepEqual(types[0].fields[0].default, {}, "an empty table is a default, so adding Metadata needs no starting value");
  assert.equal(Object.getPrototypeOf(types[0].fields[0].default), Object.prototype, "pages receive plain objects");
  assert.equal(types[1].fields[0].default, "", "an empty-text default is kept, not treated as required");
  assert.deepEqual([pairValue("2"), pairValue("true"), pairValue("homebase"), pairValue(" "), pairValue("1e3")], [2, true, "homebase", " ", 1000]);
  const source = `schema_version = 1\nid = "main"\n[[entities]]\nid = "a"\nasset = "assets/point.entity.toml"\n`;
  const written = replaceInstanceComponent(source, "a", "emergent.metadata", { values: { type: "homebase", team: 2, friendly: true } }, true);
  const component = authoredComponents((parse(written).entities as { components: unknown }[])[0].components)[0];
  assert.deepEqual(component.fields.values, { type: "homebase", team: 2, friendly: true });
  assert.equal(Object.getPrototypeOf(component.fields.values), Object.prototype);
  assert.ok(!pairsValue(Object.fromEntries(Array.from({ length: 33 }, (_, i) => [`k${i}`, i]))), "over 32 pairs");
  assert.ok(!pairsValue({ list: [1, 2] }));
});
