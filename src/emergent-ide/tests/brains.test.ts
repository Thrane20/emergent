import assert from "node:assert/strict";
import test from "node:test";
import { addNode, brainCreationContext, brainProblems, childrenOf, isEmptyBrainDraft, moveNode, newlyCreatedBrainPath, nodeCatalogue, nodeSummary, parseBrain, parseTrace, removeNode, rootOf, serializeBrain, setNodeField, shiftNode, type BrainDocument, type BrainSummary, type NodeCatalogue } from "../src/lib/brains";

const catalogue: NodeCatalogue = { categories: ["Composition", "Seek & flee"], nodes: [
  { id: "blend", label: "Blend", category: "Composition", role: "composite", status: "available", description: "", fields: [] },
  { id: "within_distance", label: "Within distance", category: "Composition", role: "decorator", status: "available", description: "", fields: [{ name: "group", label: "Group", kind: "text", help: "" }, { name: "distance", label: "Distance", kind: "number", unit: "m", help: "" }] },
  { id: "seek", label: "Seek", category: "Seek & flee", role: "steering", status: "available", description: "", fields: [{ name: "group", label: "Group", kind: "text", help: "" }] },
  { id: "pathfind", label: "Pathfind", category: "Seek & flee", role: "steering", status: "planned", description: "", fields: [] },
] };

test("node catalogue rejects duplicate IDs, including available and planned entries", () => {
  assert.equal(nodeCatalogue(catalogue).nodes.length, catalogue.nodes.length);
  const signal = { ...catalogue.nodes[2], id: "signal", category: "Actions" };
  for (const status of ["available", "planned"] as const) {
    assert.throws(() => nodeCatalogue({ categories: ["Actions"], nodes: [signal, { ...signal, status }] }), /IDs must be unique/);
  }
});

test("tree edits keep one root, respect arity and never move a node into itself", () => {
  let brain: BrainDocument = { id: "b", name: "Hunter", description: "", nodes: [] };
  brain = addNode(brain, catalogue, "seek", undefined, "s");
  brain = addNode(brain, catalogue, "blend", undefined, "root");
  assert.equal(rootOf(brain)?.id, "root", "a composite added at the top adopts the old root");
  assert.deepEqual(childrenOf(brain, "root").map(n => n.id), ["s"]);
  assert.throws(() => addNode(brain, catalogue, "seek", "s", "x"), /cannot take another child/);
  assert.throws(() => addNode(brain, catalogue, "pathfind", "root", "p"), /not available/);
  brain = addNode(brain, catalogue, "within_distance", "root", "near");
  brain = addNode(brain, catalogue, "seek", "near", "close");
  assert.throws(() => addNode(brain, catalogue, "seek", "near", "second"), /cannot take another child/, "decorators hold one child");
  assert.throws(() => moveNode(brain, catalogue, "near", "close"), /inside itself|cannot take/);
  brain = shiftNode(brain, "near", -1);
  assert.deepEqual(childrenOf(brain, "root").map(n => n.id), ["near", "s"]);
  brain = removeNode(brain, "near");
  assert.deepEqual(brain.nodes.map(n => n.id), ["root", "s"], "removing a node removes its subtree");
});

test("problems flag missing targets and children; serialisation round-trips", () => {
  let brain: BrainDocument = { id: "b", name: "Hunter", description: "Chases", nodes: [] };
  brain = addNode(brain, catalogue, "blend", undefined, "root");
  brain = addNode(brain, catalogue, "seek", "root", "s");
  assert.equal(brainProblems(brain, catalogue).get("s"), "Choose a target object or group.");
  brain = setNodeField(brain, "s", "group", "sheep");
  brain = setNodeField(brain, "s", "weight", 2);
  assert.equal(brainProblems(brain, catalogue).size, 0);
  assert.equal(nodeSummary(brain.nodes[1], catalogue.nodes[2]), "group sheep");
  const text = serializeBrain(brain);
  assert.match(text, /kind = "brain"/);
  assert.deepEqual(parseBrain(text), brain);
  assert.equal(setNodeField(brain, "s", "group", "").nodes[1].fields.group, undefined, "empty text clears a field");
});

test("empty brain drafts provide creation context and identify one new valid brain", () => {
  const draft: BrainDocument = { id: "draft", name: "Wolf hunter", description: "Flank nearby wolves", nodes: [] };
  assert.equal(isEmptyBrainDraft(undefined, draft), true);
  assert.equal(isEmptyBrainDraft("assets/brains/saved.brain.toml", draft), false);
  assert.equal(isEmptyBrainDraft(undefined, { ...draft, nodes: [{ id: "seek", type: "seek", fields: {} }] }), false);
  const context = brainCreationContext(draft);
  assert.match(context, /new, unsaved, empty brain/);
  assert.match(context, /"name":"Wolf hunter"/);
  assert.match(context, /"description":"Flank nearby wolves"/);
  const summary = (path: string, error?: string): BrainSummary => ({ path, id: path, name: path, description: "", nodes: 1, ...(error ? { error } : {}) });
  const before = [summary("assets/brains/existing.brain.toml")];
  assert.equal(newlyCreatedBrainPath(before, [...before, summary("assets/brains/new.brain.toml")]), "assets/brains/new.brain.toml");
  assert.equal(newlyCreatedBrainPath(before, [...before, summary("assets/brains/bad.brain.toml", "invalid")]), undefined);
  assert.equal(newlyCreatedBrainPath(before, [...before, summary("assets/brains/a.brain.toml"), summary("assets/brains/b.brain.toml")]), undefined);
});

test("preview traces parse and reject malformed frames", () => {
  const trace = parseTrace("brain-preview 2 agents=1 targets=1 obstacles=1 frame_seconds=0.5\nobstacles 2 3 0.8\n1 0 2 3 0 4\n1.5 0 2 3 0 4.5\n");
  assert.deepEqual(trace, { agents: 1, targets: 1, frameSeconds: 0.5, frames: [[1, 0, 2, 3, 0, 4], [1.5, 0, 2, 3, 0, 4.5]], obstacles: [{ x: 2, z: 3, radius: 0.8 }] });
  assert.equal(parseTrace("brain-preview 2 agents=1 targets=0 obstacles=0 frame_seconds=1\nobstacles\n1 2 3\n").obstacles.length, 0);
  assert.throws(() => parseTrace("brain-preview 2 agents=2 targets=0 obstacles=0 frame_seconds=1\nobstacles\n1 2 3\n"), /malformed/);
  assert.throws(() => parseTrace("brain-preview 2 agents=1 targets=0 obstacles=2 frame_seconds=1\nobstacles 1 2 3\n1 2 3\n"), /obstacles are malformed/);
  assert.throws(() => parseTrace("hello"), /Unexpected/);
});
