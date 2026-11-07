# Scene map editor (MAP-1)

Status: **in progress** (2026-11-02).

## Goal

A top-down editor for the current scene to mark out areas of interest: waypoints (single,
or a group laid down as a linked path), links between waypoints, and square or circular
zones. Every item carries name/value metadata (`type = "homebase"`) and zones can carry a
traversal cost, so agents that don't follow a ground path finder (aircraft, drones,
boats) can still prefer or avoid areas. Games query all of it at runtime. New map tools
should be easy to add later.

## What exists

- Scene instances with components (`[[entities.components]]`), validated by the Rust
  loader against the component registry; built-ins are applied by their subsystem.
- `marker.point` entity assets (transform only); the IDE viewer draws non-mesh entities
  as diamonds.
- The brain `patrol` node already takes waypoints as instance IDs (`Entities` fields).
- Scene edits go through `project-ops.ts` (`placeAsset`, `setObjectComponent`,
  `setTransform`, `reparentInstance`, `deleteInstance`): Rust-validated, journalled,
  undoable, and announced to open pages.
- The inspection session renders the scene natively and accepts `view` commands with an
  orthographic projection. Pitch is capped at about 83°, so it can't look straight down yet.
- Missing: a field kind for name/value metadata, any navigation or cost concept, and a
  public map query API.

## Design

### Map items are scene entities

Waypoints and zones are ordinary scene instances with built-in components, not a separate
map file. This keeps one identity and hierarchy system (SlotMap `EntityId` at runtime,
durable instance IDs in files). It also means parents, the scene graph, MCP refs,
transform tools, undo and the existing `patrol` node all work on map items unchanged.

| Component | Fields |
| --- | --- |
| `emergent.waypoint` | `links`: instance IDs it connects to; `two_way` (default true); `radius`: arrival radius |
| `emergent.zone` | `shape`: `circle` or `rect`; `radius`, or `size` [x, z]; `height` (0 = unbounded); `cost` (below); `layers` |
| `emergent.metadata` | `values`: name/value pairs, values being text, numbers or booleans |

- **Position and orientation:** these come from the entity's transform. A rectangle turns
  with its yaw.
- **Metadata on anything:** `emergent.metadata` can go on any object, for example a
  building tagged `type = "homebase"`, not only map items.
- **New field kind:** this needs a `Pairs` field kind. It is bounded to 32 pairs, keys of
  64 characters and text values of 256 characters.
- **Assets:** placing an item uses two project assets the editor creates once if missing,
  `assets/entities/map-waypoint.entity.toml` and `map-zone.entity.toml`. Like Point, they
  have no geometry.
- **Groups:** a "waypoint group" is a parent entity with linked child waypoints, so groups
  reuse the hierarchy.

### Costs

- **`cost` is a multiplier on travel through a zone:**
  - 1 is normal and has no effect.
  - 0.5 is preferred.
  - 3 means it is avoided unless it saves a lot of distance.
  - `blocked` means it is never entered.
- **Overlap:** where zones overlap, their multipliers multiply, and blocked wins.
- **`layers`:** this is a list of names such as `air`, `ground` or `naval`. Empty means
  every layer. A query names its layer, so a no-fly zone can stop aircraft while ground
  units ignore it.

### Runtime (`src/map.rs`)

**Building it.** Scene instantiation builds a `SceneMap` resource:
- dense arrays of zones, waypoints and metadata, keyed by `EntityId`;
- the waypoint graph as adjacency lists;
- a uniform XZ grid over the zones, with a 16 m cell size to start with.

**Queries.** These follow the `World::signals()` pattern:
- `zones_at(point)`
- `zones_where(key, value)`
- `metadata(entity)`
- `items_where(key, value)`
- `nearest_waypoint(point)`
- `waypoint_path(from, to, layer)`: A* over the waypoint graph, with each edge's length
  scaled by the zone costs along it.
- `cost_at(point, layer)`
- `segment_cost(a, b, layer)`: sampled at about 1 m steps, using the grid.

**Moving zones.** Zones are indexed at load. A zone under a moving parent is marked
dynamic and read from its live transform when queried. Deleting an entity removes its
items, and stale handles return nothing.

**Performance.** Queries don't allocate in steady state; paths fill a caller's `Vec`.
Expected scale is hundreds to a few thousand items per scene. Benchmarks will cover point
queries, segment cost and A* on a 2,000-waypoint graph. One existing limit matters here:
scenes are capped at 1,024 instances, which large waypoint networks may reach.

**Brains (MAP-3):**
- a condition, "In zone where …";
- a steering node, "Avoid costly zones", for aircraft and drones;
- "Follow waypoint path", which uses `waypoint_path`.

The planned `pathfind` node can later use the same costs.

### IDE editor (`/map`)

**Backdrop (as first proposed; replaced by the schematic, see Decisions).** The page shows the scene's real render from the inspection session:
- The camera is orthographic and looks straight down. This lifts the 83° pitch cap for this
  view and fixes the up vector so north stays up.
- Dragging pans the camera across the XZ plane.
- Zoom changes the camera height, which sets the view size.
- While panning, the last frame moves with the pointer until the next frame arrives, so
  panning stays smooth.

**Overlay.** Map items are drawn in an SVG overlay from scene data, not baked into the
render:
- waypoints, with link arrows;
- circles and rectangles, filled by cost, with blocked areas hatched;
- labels from `name` or `type`.

A top-down orthographic view is an exact 2D mapping, so the overlay lines up precisely and
updates instantly.

**Tools (left).** This follows the brain editor's palette pattern: click a tool to arm it,
then click the map to place.

| Tool | Gesture |
| --- | --- |
| Select / move | Click to select; drag to move; Delete to remove |
| Waypoint | Click to drop one |
| Waypoint path | Click, click, click; Enter or double-click ends; points are linked in order under a new group parent |
| Link | Click waypoint A, then B (toggles the link) |
| Circle zone | Press at the centre, drag out the radius |
| Rectangle zone | Drag corner to corner |

**Inspector (right).** The selected item's name, shape, cost and layers, plus a metadata
table to add, edit and remove pairs. A "Place under" parent picker at the top applies to
new items.

**Writes.** Every gesture writes straight to the scene through `project-ops` (journalled
and undoable), so there is no Save button. Placements and component changes will also be
pushed live to the open inspection session, which only transforms do today.

**Extensibility.** Tools live in a small registry (`map-tools.ts`). Each tool declares:
- its label and icon;
- its gesture: point, polyline, circle or rectangle;
- the components it creates;
- how to draw it.

The inspector builds its fields from the Rust component registry, so a new map component
appears there without UI changes. A new tool is one registry entry plus a draw function.

### MCP

Map items are instances, so `place_asset`, `set_component`, `reparent`, `delete` and
`undo` already work on them. MAP-3 adds a read-only `query_map` tool (zones at a point,
items where a key has a value, the path between waypoints).

## Validation

- **Rust:** loader rules (shapes, bounds, links to missing or self IDs, the pair limits);
  query correctness (circle and rotated rectangle containment, height); overlapping costs
  and blocked areas; layers; A* that avoids costly zones; dynamic zones; deleted entities;
  benchmarks.
- **IDE:** unit tests for the gesture math (screen to world) and for metadata
  serialisation; ops tests (create, edit, link, group, undo); and a Playwright smoke on
  port 3001 (place each item type, edit metadata, pan and zoom).

## Slices

1. **MAP-1: engine.** The three components, the `Pairs` field kind, the `SceneMap` resource
   and its queries, waypoint A* with costs; tests and benchmarks.
2. **MAP-2: editor.** The `/map` page with the top-down backdrop, overlay, tool palette,
   inspector and parent picker; direct, undoable writes; live push.
3. **MAP-3: use.** The brain nodes, the MCP `query_map` tool, and the scene viewer drawing
   zones and links in the normal 3D view.

## Delivered: MAP-1 (2026-11-02)

- `src/map.rs`: `Waypoint`, `Zone` and `Metadata` components (immutable; insert/discard
  hooks bump a generation, so the index rebuilds only after map changes) and the World
  queries `zones_at`, `cost_at`, `segment_cost`, `items_where`, `nearest_waypoint` and
  `waypoint_path` (A*, edge costs cached per layer until a zone or waypoint moves).
- Registry: `emergent.waypoint`, `emergent.zone`, `emergent.metadata`, and a `Pairs` field
  kind (32 pairs; 64-character names; text up to 256 characters, numbers, booleans),
  supported by the IDE's component inspector.
- Scene limit 4,096 instances / 2 MiB, shared by the engine (`SCENE_MAX_INSTANCES`,
  `SCENE_MAX_BYTES`) and the IDE (`scene-limits.ts`); candidate validation and inspection
  replies were raised to match.
- Measured (`examples/map_bench.rs`, M3 Pro, release, 4,096 instances: 2,000 waypoints in
  a linked grid and 2,096 zones): validation 120–135 ms (the same without map components),
  instantiate and map build 2.2 ms; map refresh about 12 µs of a 68 µs tick; `cost_at` and
  `zones_at` ≈ 0.1 µs; a 50 m `segment_cost` 3 µs; a corner-to-corner route over the grid
  46 µs once edge costs are cached (5.6 ms the first time after a change).
- IDE at 4,096 instances: snapshot 15 ms, index 42 ms, scene placement 48 ms, but
  validating each edit takes 1.36 s because the IDE runs the game's debug build (about
  130 ms optimised). Follow-up: optimise the engine dependency in debug builds of projects.
- Fixed on the way: the inspector dropped empty-text defaults, so adding an Audio source
  silently set Play on signal to "group".

## Delivered: MAP-2 (2026-11-02)

- `/map` page (`scene-map-editor.tsx`): schematic backdrop of object outlines from the
  engine (`--map-layout`: world matrices and XZ convex hulls, cached per scene revision),
  drawn over `top_down`, a height-shaded image (≤1024 px a side) of every mesh triangle seen from above,
  a metric grid and scale bar; pan by dragging empty space (or right/middle drag), zoom
  with the wheel about the cursor (shown as camera height); tools Select/move, Waypoint,
  Waypoint path (linked, grouped), Link (toggles; two-way links count once), Circle zone,
  Rectangle zone; Delete removes (links to deleted waypoints are cleaned up); Undo.
- Inspector: name, position, and the zone/waypoint/metadata fields built from the engine
  registry (links pick other waypoints; metadata pairs editor); edits save after a pause.
- Every edit goes through `editScenePath` → journalled, engine-validated, undoable; new
  items go under the "Place under" parent. Tools are a registry (`map-tools.ts`): a gesture
  plus a `build` function.
- Validation: `tests/scene-map.test.ts` (maths, and place/link/move/edit/delete/undo
  against a template copy) and `scripts/map-smoke.ts` (Chromium: every tool, inspector,
  zoom, undo; refuses to run against any IDE not linked to the named scratch project).
- Known limits: each edit re-validates and re-runs the layout in the game's debug build
  (well under a second at template scale, slower near 4,096 instances); rectangles are
  drawn axis-aligned and turned afterwards with the rotation field; offsets under a
  rotated or scaled parent are converted through its world matrix.

## Prefab-local maps (2026-11-03)

The Map source picker also opens prefab assets. New map items are children of the
prefab root by default; the same tools, inspector, native validation, revision guard
and undo edit the prefab file. The root cannot be removed in Map. Each runtime
instantiation gets fresh map handles, and `World` transforms resolve local positions
to global positions as the prefab is placed or moved. See [prefab maps](prefab-map-editor.md).

## Decisions (2026-11-02)

- Cost model: multiplier with `blocked` and named layers.
- Backdrop: a schematic of object footprints drawn by the IDE (instant, no render wait),
  not the engine render. Footprints come from the engine: each instance's world-space XZ
  outline (convex hull of its mesh, or terrain bounds) from a host command.
- Scene instance limit raised from 1,024 to 4,096, measured as part of the change.
