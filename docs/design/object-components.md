# Object components ("Add Component")

Status: **slices 1–4 implemented** (2026-10-30). Remaining: asset-level component defaults,
live push to the inspection world, and moving the spin/bounce/avoidance wiring that is
tied to `main` and the `bda577fe…` scene (left while those scenes have uncommitted edits). Covers GOALS.MD Stage 2A
"authored behaviour links, full type registration and registry/schema exports" and
the Stage 3 interactive-input gap.

## Goal

A scene object carries a list of typed **components**, as in Unity: Transform,
Physics, Material, Camera and Light today, and anything else tomorrow (a first-person
controller, spin, follow, a project-defined "jumping bean"). In the IDE you click
**Add Component** on an object, pick a type, and set its fields in a form built from
the type's schema. When the game runs, every enabled component on an active object
does its job each fixed tick. Statements become "write me a new component type and
attach it", instead of hand-wiring code to one instance ID.

## What exists and what is wrong with it

- `src/behavior.rs`: a closed enum of three engine behaviours with SlotMap keys,
  typed ports, links, committed snapshots and stable commit order. It is sound but
  only reachable from Rust; scenes cannot author it.
- Project behaviours (`spin.rs`, `bounce.rs`, `avoidance.rs`) are wired in
  `game.rs` by hard-coded instance IDs. A new behaviour means editing `game.rs`.
- No keyboard input reaches the game. The window handles only close, resize and redraw.
- Physics exposes `add_force` only; no velocity read/write. `bounce.rs` fakes its
  motion because of this.
- Lights: values live only in the asset. Instances cannot override them and the
  inspector has no Light section.

## Model

**Component type.** A Rust type registered under a stable string ID
(`emergent.light`, `emergent.character_controller`, `project.jumping_bean`). It
declares a **schema** — named fields with type (number, vector3, colour, bool,
enum, entity ref), default, range and help text — and how to attach to an entity.
Type IDs are durable; Rust type names and SlotMap keys are never serialised.

**Registry.** `ComponentRegistry` lists types. The engine registers built-ins; the
project registers its own at startup through public API only. The game binary
exports the registry as JSON (`--export-components`), which the IDE and MCP read.
This is the "registry/schema export" Stage 2A asks for.

**Authored form.** Components live on the instance, with asset defaults:

```toml
# asset (defaults for every instance)
[[components]]
type = "emergent.character_controller"
speed = 4.0

# scene instance (adds or overrides; matched by type)
[[entities]]
id = "cube"
asset = "assets/entities/cube.entity.toml"
[[entities.components]]
type = "emergent.character_controller"
enabled = true
speed = 6.0
```

Existing `[physics]`, `[material]`, `[camera]` and `[light]` tables stay valid and are
shown as built-in components; they are not migrated in the first slice. At most one
component of each type per object in the first slice (no duplicates to address).

**Runtime ownership (SlotMap).** Each component type owns its runtime state in its
own storage keyed by `EntityId` (dense `Vec` plus index, like other component
storage). Entity deletion removes its components through the existing
`remove_owner`-style cleanup. Components that publish values for links reuse the
existing `BehaviorGraph` keys and ports rather than a second identity system.

**Execution.** Fixed tick only. Order: read input snapshot → each component type
runs over its enabled instances in authored order (type order fixed by the
registry, stable) → stage transform/physics writes → commit → physics step. One
transform writer per object (already enforced for behaviours); a component that
moves a dynamic body writes velocity or force, never the transform.

**Active / enabled.** Each component has `enabled`. Each object gains `active`
(default true); an inactive object's components do not run and it does not render.

## Input

`Input` is a per-tick snapshot owned by the engine: keys held, pressed this tick and
released this tick, plus mouse delta. The native window fills it from winit; the
headless runner fills it from a scripted sequence so tests and replays are
deterministic. Gameplay reads named **actions** (`move_forward`, `jump`) mapped to keys
in `assets/data/input/*.input.toml` (the planned catalogue entry), with WASD/space/
mouse defaults so a new project works with no file.

## First built-in types

| Type | Fields | Needs |
| --- | --- | --- |
| `emergent.light` | kind, colour, intensity, range, cone angles, shadows | Instance overrides; direction = transform rotation |
| `emergent.character_controller` | speed, jump speed, relative-to (camera / world), drive force (N: a = F/m toward speed, released keys coast under contact friction at μ·g; 0 = instant), look with mouse | Input, physics velocity API |
| `emergent.spin` | axis, degrees per second | Replaces `spin.rs` wiring |
| `emergent.follow` | target (entity ref), max speed | Existing `follow_position` behaviour |

The controller on a dynamic body sets horizontal velocity from input and keeps
gravity, so "the cube drops, then WASD moves it" works. On a body without physics it
moves the transform directly.

## IDE and MCP

- Inspector: a **Components** list under the object with Add Component (searchable
  registry), enable toggle, remove, and a form generated from the schema. Light gets
  its section this way.
- Saves go through `commitSceneEdit` (revision guard, native validation, journal).
- MCP: `list_component_types`, `add_component`, `set_component`, `remove_component`,
  using the same ops. The existing `set_material` / `set_physics` / `set_camera` stay.
- Statements: implementing a statement may add a **project component type** (Rust
  module + registration) and attach it to the object, compiled through the existing
  staged-check pipeline. Built-in types attach instantly with no compile.

## Hot paths and scale

Per tick: one pass per component type over a dense list; no allocation in steady
state; input snapshot is a fixed-size bitset. Scenes are bounded at 1,024 instances,
so the first slice needs no parallelism. Measure tick cost with 1,024 controlled
objects before claiming a budget.

## Phasing

1. **Light component + instance overrides + generic schema form.** Proves the
   registry, schema export, authored `[[components]]`, IDE form and MCP ops on
   something visible at tick zero.
2. **Input snapshot + character controller + physics velocity API.** WASD in
   native Run; scripted input in headless tests.
3. **Spin / follow as components**; move `game.rs` wiring to authored components;
   decide the bounce test as part of this.
4. **Project component types from statements.**

## Validation

- Load rejects unknown types, unknown fields, out-of-range values, duplicate types per
  object and entity refs outside the scene, with paths in the message.
- Round-trip: IDE/MCP edit → file → load → same values.
- Deterministic headless test: a scripted W held for 1 s moves the controlled cube
  `speed` metres (within tolerance) on flat ground, while it still falls under gravity.
- Deleting an object removes its component state; stale entity refs fail explicitly.
- Native smoke: WASD moves the cube in a Run window.

## Decisions (2026-10-30)

- Every active object with an enabled controller receives WASD; toggle `enabled` to
  choose. A single "player" object is not modelled.
- Third person: the scene camera stays separate and movement is relative to where it
  looks. Camera follow is a later component.
- Build slice 1 (light component, registry, schema form, MCP tools) first.

## Slice 1 as built

- Engine: `src/component.rs` (`ComponentRegistry::builtin`, `ComponentType::validate`,
  `export_toml`). The loader checks each `[[entities.components]]` entry against the
  registry (unknown type, unknown field, wrong shape, out of range, duplicate type, light
  component on a non-light) and applies `emergent.light` over the asset's `[light]`.
  `enabled = false` removes the light source from that instance.
- Game binary: `--export-components` prints the registry; the IDE caches it per project.
- IDE: `lib/components.ts` (shape checks, patching), `replaceInstanceComponent` /
  `saveSceneComponent` through the guarded scene edit, `ComponentsInspector` (fields by
  schema, hidden when the light's type does not accept them, per-field reset, enabled,
  "Use asset defaults"). Saving restarts inspection; live push is not implemented.
- MCP: `list_component_types` and `set_component` (journalled and undoable).
- Not yet: asset-level `[[components]]` defaults, Add Component for `any` types (none
  exist until slice 2), project-registered types, live push without restart.

Validation: Rust schema and loader tests (`component.rs`,
`tests/authored_scene.rs::light_components_*`); IDE unit tests (`tests/components.test.ts`);
a headless-Chrome check on an isolated IDE that edits the key light's colour and intensity
in the inspector, sees the frame change, then switches it off, rejects a bad field, resets
and undoes through MCP.

## Slices 2–4 as built

- **Input** (`src/input.rs`): `Key`, `KeySet` (bitset) and `Input` (held/pressed/released,
  `axis`). `Engine::set_keys` takes held keys from the platform; each fixed tick derives
  edges once, so catch-up ticks cannot repeat a press. `Input` is a world resource and an
  `EngineContext` field. The first project's window maps physical keys and clears them on
  focus loss.
- **Motion** (`src/motion.rs`): `CharacterController`, `Follow` and `Spin` components run
  once per tick after behaviours and before physics, from cached queries into reused
  buffers. Controllers on dynamic bodies call `World::drive_body` (horizontal velocity,
  rotation locked, jump only when vertical speed is near zero — a shortcut until contact
  queries exist); otherwise they move the transform. Camera-relative movement uses the
  `ActiveCamera` resource set by scene loading. Follow reads the target's previous
  committed position (one-tick lag) and holds still if the target is gone.
- **Loader rules**: one controller-or-follow per object; spin/follow rejected on dynamic
  bodies; follow targets must be in the scene and not the object itself; required fields
  without defaults (follow `target`) must be authored; disabled components are not attached.
- **Project types** (`ComponentRegistry::register`, `SceneDefinition::load_with`,
  `LoadedScene::components`): the first project registers `project.jumping_bean` in
  `src/components.rs`, attaches its runtime in `ProjectComponents`, and the physics scene
  authors it on `falling-cube` instead of `game.rs` matching that ID. The binary exports the
  combined registry; the IDE re-reads it when the game's Rust source changes.
- **IDE/MCP**: Add Component lists every `any` type (built-in and project), vector and
  target-object fields, schema defaults, Remove component. Statement and chat
  implementation prompts now direct the AI to reuse a component or add a project
  component type, never to hard-code instance IDs.

Validation: `tests/components.rs` (loader rules; scripted W+D slides 3 m diagonally,
follow and spin rates; a falling dynamic cube lands, W moves it ~4 m upright, held Space
jumps once), `input.rs`/`component.rs` unit tests, IDE unit tests, a headless-Chrome check
of Add Component, edit, engine-rejected conflict, Remove, target selection and MCP
`set_component`, and a 120-tick native Metal run of the physics scene with the jumping bean
and a controller on the falling cube. WASD in the native window was not driven by an
automated test.
