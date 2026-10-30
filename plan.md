# Emergent plan

Last updated: **2026-10-30**. This is the live work queue. Keep it short: move a
finished item into [docs/implementation-status.md](docs/implementation-status.md) in
the same change that finishes it. Long-range goals and stage gates are in
[GOALS.MD](GOALS.MD); the documentation map is in [docs/README.md](docs/README.md).

## Now

- **Bounded meshed-density comparison.** GPU pass profiling and native offscreen
  camera routes are delivered. Compare a mesh extracted from the same field with
  direct ray marching, including overhang/cavern quality and collision suitability.
  Research focused Rust extraction options and write the design before choosing
  an implementation. Keep gameplay and full resource budgets open.
  [Detailed checklist](docs/plans/terrain.md#current-priority--terrain-performance-v1).
- **Project MCP server: MCP-2 (edits).** MCP-1 (read and view) is delivered. Next:
  guarded transform, rename, reparent, delete, place-asset and component edits, with
  dry-run, an undo journal, a live inspection push, IDE refresh over SSE, and
  `assign_ids` for files without durable IDs.
  [Design](docs/design/project-mcp-server.md).

## Known issues

- `first_project` test `authored_offender_bounces_about_four_metres_above_ground`
  fails: it measures 3.34 m against an expected 4 ± 0.2 m. It is tied to the
  authored main scene, so decide whether the scene or the test is wrong. This
  failure also stops `cargo test --workspace` before later targets run.
- Player settings (`/player`, `player.settings.toml`) are not consumed at runtime.
  Either wire resolution/fps/AA into the native run, or label the page as planned.
- No evidence record exists yet for scene lighting/skyboxes.
- `assets/textures/c6dfc7dc-…/explosion_atlas.png` has no descriptor, so it has no
  durable ID and the MCP server can't address it. MCP-2 `assign_ids` will fix this.
- Codex as an MCP client is configured but not yet verified in a live session: the
  account hit its usage limit on 2026-10-30. Claude Code is verified.
- About a week of work (physics, terrain, particles, materials, tweens, structures,
  IDE editors) is uncommitted; the last commit is 2026-10-23.

## Next

Ordered by the current [terrain milestone](docs/plans/terrain.md), then the
foundation gates in [GOALS.MD](GOALS.MD) that block the developer-framework proof:

1. Choose the density representation/collision path from the bounded comparison;
   validate cave/overhang contacts and replacement safety.
2. Heightfield shared sampling, chunk visibility and distance LOD, following
   measured terrain budgets and the stage gates in the terrain plan.
3. Bounded preparation and streaming after visibility/LOD; include 1024 authoring
   responsiveness (GLB packing, paint, save, upload and collider activation).
4. Interactive play / pause / single-step with backlog reset, in the engine and IDE
   (Stage 2, Stage 3A).
5. Authored composition: nested reusable entities, authored behaviour links, type
   registry and schema/graph exports with structured diagnostics (Stage 2A).
6. Broader Stage 0 budgets: target hardware, workload sizes and frame/memory budgets, so
   that performance claims have something to be measured against.

Project MCP server, after MCP-2: MCP-3 (create assets, validate/run, IDE selection
as "this"), then MCP-4 (cost-confirmed generators).

## Backlog

Grouped by area. Each item is open; see the linked plan or design for scope.

- **Terrain** — continuous-world stages 0–6, volumetric/Marching Cubes research,
  hydraulic erosion, slope/biome material rules: [terrain plan](docs/plans/terrain.md).
- **Rendering** — spot shadows, cascades, culling/batching, polylines and debug overlays,
  native GLB import and materials, HDR/IBL, post-processing.
- **Physics** — kinematic bodies, sphere/capsule/convex shapes, triggers and events,
  public queries, joints, character controllers.
- **Animation** — tween bindings to scene properties from the inspector.
- **Authoring/IDE** — undo/redo, preserve-world reparent, snapping and multi-select,
  text-to-reference-image for Object Creator, model placement in scenes.
- **Platform** — Windows/Linux native runs, multi-output build and packaging,
  minimum OS policy.
- **Audio, input mapping, UI runtime** — not started; see the capability catalogue.
