# Brains: stacked behaviour trees and the Brain editor

Status: **phases 1–3 implemented** (2026-10-31); MCP brain tools and the neighbour grid are open.
Builds on [object components](object-components.md).

## Goal

A **brain** is a named, saved tree of behaviour nodes. You build it in the Brain editor
from a library of standard nodes (seek, flee, flocking, patrol, evaluation, …), refine it
with AI, save it, and attach it to scene objects. Brains can contain other saved brains,
so small behaviours roll up into larger ones ("sheep" = flock + graze + flee wolves;
"wolf pack" = pursue nearest sheep + keep formation). Emergence comes from many objects
running simple stacked brains against each other's committed positions.

## Decisions (2026-10-30)

- **Steering tree.** Leaves output a desired velocity; composites blend, prioritise or
  select; decorators gate or score a single child. No success/failure task nodes yet.
- **Full 3D.** Nodes steer in 3D. A dynamic physics body ignores the vertical part (it is
  driven like the character controller and keeps falling), and the brain component has a
  `planar` switch for non-physics ground agents.
- **Preview in the first build.**
- Brain assets are referenced by asset path, like entity assets in scenes. Node IDs are
  durable within a brain.

## Model

**Brain asset** — `assets/brains/<id>.brain.toml`: id, name, description and a flat
`[[nodes]]` list. Each node has a durable `id`, a `type` from the node catalogue, a
`parent` (absent on the single root), typed parameters, `weight` and optional `label`.
List order is sibling order. A flat list keeps TOML diffs and AI edits small. Saved
brains are referenced by asset ID, never by path or runtime key.

**Node kinds**

- *Steering leaves* output a desired velocity on the ground plane (seek, flee, wander, separation, …).
- *Composites* combine children: **Blend** (weighted sum, clamped to max speed),
  **Priority** (first child with a non-zero output, Reynolds-style), **Selector** (first
  child whose condition passes), **Sequence** (children in turn, advancing when one
  completes, e.g. patrol legs), **Utility** (child with the highest score wins).
- *Conditions / evaluators* gate or score a child (target within distance, in view cone,
  neighbours ≥ n, chance, cooldown). A failed condition makes its branch inactive.
- *Brain reference* embeds another saved brain as a subtree. Cycles are rejected at load.

**Targets** are an object in the scene (entity ref), the nearest member of a **group**,
or "self/home". Groups come from a new `emergent.group` component (`name`, e.g. "sheep",
"wolves"), which also defines flock neighbourhoods.

**Applying** — `emergent.brain` component with field `brain` (asset ID) plus
`max_speed`. The brain's output velocity drives the object exactly like the character
controller (dynamic body: `drive_body`; otherwise transform). One mover per object: a
brain cannot be combined with a controller or follow.

## Runtime (Rust, `src/brain.rs`)

- Brains compile at scene load into an index-linked node array per asset (shared by all
  objects using it), validated (types, params, weights, references, depth ≤ 32, ≤ 256
  nodes after inlining referenced brains). Evaluation recurses at most 32 levels. Per-object state (current sequence step, wander angle, cooldown timers)
  lives in a small per-agent array owned by the brain system, keyed by `EntityId`.
- Each fixed tick, after input and before physics: read the committed positions and
  velocities snapshot (built once per tick into a dense array), evaluate each agent's tree
  iteratively (no recursion), write desired velocity, then apply all moves. Reading the
  snapshot makes results independent of agent order and gives a one-tick delay, matching
  the engine's behaviour rules.
- Wander and chance use a per-agent seeded RNG (seed from the instance ID), so runs replay.
- Neighbour queries (flocking, nearest-of-group) start as a linear scan over the group's
  members: O(n²) for 1,024 agents is ~1M distance checks per tick. Measure it; add a
  uniform grid when the scan exceeds its budget.

## Node library (headings → nodes). ✓ = first slice, ○ = later

1. **Composition** — Blend ✓, Priority ✓, Selector ✓, Sequence ✓, Utility ✓, Random
   choice ○, Repeat ○, Cooldown ✓, Invert ○, Wait ✓.
2. **Evaluation & senses** — Within distance ✓, Outside distance ✓, In view cone ✓,
   Neighbours at least ✓, Chance ✓, Timer elapsed ○, Line of sight ○ (needs ray queries),
   Stat threshold ○ (needs stats), Utility scorers: distance ✓, crowding ✓.
3. **Seek & flee** — Seek ✓, Flee ✓, Arrive (slow on approach) ✓, Pursue (predict) ✓,
   Evade ✓, Hide behind obstacle ○.
4. **Wander & idle** — Wander ✓, Hold position ✓, Return home ✓, Orbit / circle ✓, Drift ○.
5. **Paths & patrol** — Patrol waypoints (loop / ping-pong) ✓, Follow path once ✓, Follow
   target (keep distance) ✓, Pathfind (A*/navmesh) ○.
6. **Flocking & formation** — Separation ✓, Alignment ✓, Cohesion ✓, Flock (all three,
   tuned) ✓, Follow leader ✓, Formation slot (line, wedge, ring) ✓, Queue / single file ○.
7. **Avoidance & bounds** — Avoid objects (keep clear of a group) ✓, Stay within area
   (radius around home or a point) ✓, Obstacle avoidance ○ (needs colliders/rays), Wall
   follow ○.
8. **Group tactics (saved-brain templates)** — Predator, Prey, Herd, Guard area, Swarm,
   Escort, Ambush. These ship as starter `.brain.toml` assets built from the nodes above,
   so they are editable, not new primitives.
9. **Actions** — Face movement ✓, Look at target ✓, Speed limit ✓, Jump ○, Signal/event ○.
10. **Project brains** — every saved brain in the project appears here and can be dropped
   in as a sub-behaviour.

## Brain editor (IDE `/brains`)

Same frame as the other editors (rail, header, three panels, AI chat):

- **Left — Library.** Searchable, collapsible headings above; planned nodes visible but
  disabled; "Project brains" at the bottom. Click (or drag) adds the node under the
  selected node.
- **Centre — Tree.** Top-down tree with the root node at the top. Nodes show type,
  label, key parameters and weight. Select, reorder among siblings, move under another
  node, collapse, delete. Brain-reference nodes expand read-only.
- **Right — Inspector.** Name/description of the brain; parameters of the selected node
  rendered from the node schema (the same field kinds as components), plus **Used by**
  (scene objects with this brain) and **Apply to…** (adds `emergent.brain` to a chosen
  instance).
- **Bottom — Build with AI.** Scoped to the open brain: "make them flee wolves when
  closer than 6 m but otherwise flock" edits the brain TOML through the guarded AI
  change path, validated by Rust before saving.
- **Preview** (centre toggle): a top-down sandbox where N agents (and optional targets)
  run the brain in the Rust runtime (`--simulate-brain`, fixed ticks, returns
  trajectories), so emergence is visible without building a scene.

Saving writes the asset atomically after `--validate-brain` passes; the node catalogue
is exported from Rust (`--export-brain-nodes`) like the component registry, so the
library, inspector forms, validation and AI prompts share one source.

## MCP

`list_brain_nodes`, `describe` for brain assets, `create_brain`, `edit_brain`
(add/move/update/remove node by node ID, revision-guarded), and `set_component` with
`emergent.brain` to apply.

## Phasing

1. Rust runtime + catalogue export + validation; `emergent.group` and `emergent.brain`
   components; first-slice nodes; deterministic headless tests (seek converges, flee
   diverges, flock separation bound, patrol order, priority/selector semantics,
   reference cycles rejected).
2. Brain editor page: library, tree, inspector, save, apply to object.
3. Preview sandbox and AI chat scoping; starter group-tactic brains.
4. MCP tools; neighbour grid if measurements need it.

## Validation

Unit tests per node, composite semantics tests, replay determinism (same seed and
input ⇒ identical trajectories), 1,024-agent flock tick-time measurement in release
build, IDE tests for tree edits and save round-trip, browser check of build → save →
apply → run.

## As built (2026-10-31)

- **Engine** — `src/brain.rs`: catalogue (34 available nodes, 15 planned) exported by
  `--export-brain-nodes`; `BrainDefinition` loads, validates and inlines references
  (cycles, depth > 32 and > 256 nodes rejected); `CompiledBrain` resolves targets, groups
  and waypoints per scene and is shared by its agents; `BrainAgent` holds per-agent node
  state, velocity, home and a seeded RNG; the step runs first in the motion phase.
  Components `emergent.group` and `emergent.brain` (max speed, acceleration, ground only,
  face movement). Scenes reject brains naming objects they do not contain, and more than
  one mover per object. `simulate` drives the normal engine tick for the preview.
- **Game tools** — `--validate-brain <path> [--candidate f]`, `--simulate-brain <path>
  [--candidate f] key=value…`. AI changes to `.brain.toml` are validated in the staged
  candidate build before publication.
- **IDE** — `/brains` (rail: Brain Editor): library by heading (planned nodes shown
  disabled, saved brains embeddable or openable), top-down tree (drag from the library,
  drag to re-parent, reorder, delete, problems flagged), schema inspector, Rust preview
  (agents, groups, targets, speed, seed; group names inferred from the brain), save with
  revision guard and Rust validation, apply to a scene object, and the AI chat scoped to
  the brain file. Starter brains: Prey, Predator, Guard area, Swarm, Escort.
- **Measurements** (release, Apple M3 Pro, `cargo test --release --test brains --
  --ignored`): a flock-only brain costs 0.027 ms/tick for 64 agents and 0.53 ms/tick for
  256 (quadratic neighbour scan, three scans per flock node). Extrapolated ~8 ms at 1,024
  — the grid is needed before large flocks; the preview caps at 256 agents.
- **Not yet** — MCP brain tools, a spatial grid, line of sight/obstacles/pathfinding
  (need physics queries), actions such as jump and signal, and facing for dynamic bodies.

Validation: `tests/brains.rs` (every available node loads and runs; 15 invalid-tree
cases and a reference cycle; seek reaches, arrive stops at 0.3–0.55 m, flee escapes,
priority hovers at the panic edge, patrol visits waypoints in order in a scene and stops
on `once`, flock keeps agents > 0.3 m apart, identical traces for the same seed),
`tests/brains.test.ts` (tree edits, arity, round-trip, problems, trace parsing), a
headless-Chrome run of edit → save → preview → new brain → save → apply, and a 120-tick
native Metal run of `main` with the Prey brain on the pyramid.

