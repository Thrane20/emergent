# Emergent implementation status

Last reconciled: **2026-10-30** against source, tests and evidence records.
This is the ledger of **delivered** work: one row per slice, with links to its design
and evidence. Goals and stage gates live in [GOALS.MD](../GOALS.MD); work in progress
and next steps live in [plan.md](../plan.md). The narrative status document this
replaced is archived in [history/epic-log-2026-10.md](history/epic-log-2026-10.md).

All runtime evidence is **macOS / Apple Silicon / Metal** unless stated. No row
claims Windows/Linux execution or an agreed production frame budget.

## Current validation snapshot — 2026-10-30

| Check | Result |
| --- | --- |
| `cargo test --workspace --all-features` | Engine: 142 pass (103 unit, 25 authored-scene, 9 physics, 3 units, 2 ECS mapping). First project: 15 pass, **1 fails** — `authored_offender_bounces_about_four_metres_above_ground` (measured 3.34 m, expects 4 ± 0.2 m). Cargo stopped at that failure, so later targets did not run. |
| `pnpm test` (`src/emergent-ide`) | 140 pass, 0 fail |
| Project MCP server (MCP-1/2) | `pnpm test` 140 pass; `test:mcp-edit` and the live browser smoke pass; type-check, lint, Rust fmt/Clippy clean; first project 16 pass + the known bounce failure; live Claude Code and Codex edit sessions verified; [evidence](platform/project-mcp-server-macos.md) |
| Terrain performance increment | Engine release tests, Clippy, rustdoc, lint/types/build, browser and native smokes pass; [scope and measurements](benchmarks/terrain-performance/README.md). No cross-platform or continuous-world performance acceptance. |

## Engine foundation

| Slice | Source | Decision / evidence | Open boundary |
| --- | --- | --- | --- |
| Fixed-step loop, bounded catch-up, exact headless ticks | `src/engine.rs` | [iterations](history/foundation-iterations.md) | Interactive pause/resume/step; module builder |
| SlotMap world, hierarchy, dirty transforms | `src/world.rs` | ADR 0001, [transform baseline](benchmarks/iteration-2-transform-propagation.md) | Preserve-world reparenting |
| Standalone `bevy_ecs` components/schedules, SlotMap identity | `src/world.rs`, `src/engine.rs` | ADR 0002–0003, [ECS comparison](research/iteration-3-ecs-comparison.md), [runtime](benchmarks/iteration-4-runtime-integration.md) | Workload acceptance |
| Typed behaviour graph, committed snapshots | `src/behavior.rs` | ADR 0004, [baseline](benchmarks/iteration-5-behavior-graph.md) | Authored links, type registry |
| wgpu/winit graphics boundary | `src/graphics.rs` | ADR 0005, [smoke](platform/iteration-6-macos-smoke.md) | Windows/Linux runs |
| Points, lines, convex polygons, indexed meshes; renderer-owned `GeometryId` | `src/geometry.rs` | ADR 0006, [smoke](platform/iteration-8-macos-geometry-smoke.md) | Concave/outlined polygons, debug overlays |
| Eight procedural primitives; outward winding fix (PN-1) | `src/primitives.rs` | ADR 0007, [smoke](platform/iteration-9-macos-primitives-smoke.md) | — |
| Ambient/directional/point/spot lights, eight-light Lambert | `src/light.rs` | ADR 0008–0009, [smoke](platform/iteration-11-macos-smoke.md) | Area/environment lighting |
| Directional shadow map + object cast/receive (SH-1) | `src/graphics.rs` | [design](design/object-shadows.md), [evidence](platform/object-shadows-macos.md) | Cascades, quality budget |
| Point-light shadows, two six-face slots (SH-2) | `src/graphics.rs` | [design](design/point-light-shadows.md), [evidence](platform/point-light-shadows-macos.md) | Spot shadows, more slots |
| Cameras: perspective/orthographic, clip planes | `src/camera.rs` | [design](design/camera-inspector-frustum.md), [evidence](platform/camera-authoring-macos.md) | Public render-extraction concept |
| Materials: flat/smooth/unlit, Blinn-Phong, emission, noise channels, bump (M-1, SURF-1) | `src/material.rs` | [design](design/material-editor.md), [evidence](platform/material-editor-macos.md), [surface quality](design/surface-quality-comparison.md) | PBR, shader graphs |
| Diffuse PNG/JPEG, triplanar projection | `src/texture.rs` | [design](design/diffuse-image-materials.md), [evidence](platform/diffuse-images-macos.md) | Authored UVs, shared texture residency |
| Shared material library, four-slot terrain blend | `src/material.rs` | [design](design/material-library-terrain-paint.md) | Slope/biome rules |
| Seeded Perlin/OpenSimplex/Worley noise (NF-1) | `src/noise.rs` | [design](design/noise-filter-assets.md), [evidence](platform/noise-filter-assets-macos.md) | Filter graphs, value noise |
| Rapier physics: static/dynamic, box + heightfield colliders (PH-1) | `src/physics.rs` | [design](design/physics-foundation.md), [evidence](platform/physics-foundation-macos.md) | Kinematic, other shapes, queries/events, joints |
| Heightfield terrain generation v1/v2, presets incl. plain/canyon/underwater | `src/terrain.rs` | [terrain plan](plans/terrain.md) | Chunks/LOD/streaming |
| Terrain performance v1: first increment | `src/graphics.rs`, `src/raymarched_terrain_graphics.rs`, `examples/terrain_performance.rs`, `examples/terrain_live_preview.rs` | [design](design/terrain-performance-v1.md), [benchmarks and validation](benchmarks/terrain-performance/README.md) | Gameplay traversal/memory budgets, depth termination, chunks/LOD |
| Optional GPU pass profiling and native camera-route benchmark | `src/gpu_profile.rs`, `examples/terrain_gpu_profile.rs` | [design](design/terrain-performance-v1.md#gpu-profiling-increment), [GPU evidence](benchmarks/terrain-performance/gpu/README.md) | Serial diagnostic only; particle timing, presentation and total memory budgets |
| Ray-marched terrain prototype and v2 biomes | `src/raymarched_terrain.rs`, `src/raymarched_terrain_graphics.rs` | [design](design/raymarched-terrain.md), [biomes](design/raymarched-biomes.md), [evidence](platform/raymarched-terrain-macos.md), [biome evidence](platform/raymarched-biomes-macos.md), [GPU profiling](benchmarks/terrain-performance/gpu/README.md) | Collision, lighting parity, gameplay budgets |
| Scene environment: ambient, background, equirectangular skybox | `src/environment.rs`, `src/skybox_graphics.rs` | [design](design/scene-lighting.md); **no evidence record** | HDR/IBL, seam repair |
| CPU particle pools, instanced billboards, flipbooks, ping-pong | `src/particles.rs`, `src/particle_graphics.rs` | [design](design/particle-generator.md), [evidence](platform/particle-generator-macos.md), [ping-pong](platform/sprite-ping-pong-macos.md), [benchmarks](benchmarks/particles-2026-10-29/) | GPU path, collision hooks |
| Scalar tweens and easing (TWEEN-1) | `src/tween.rs` | [design](design/tween-editor.md), [evidence](platform/tween-editor-macos.md) | Scene property bindings |
| Polygon structure assets | `src/structure.rs` | [design](design/object-builder.md) | — |
| Units contract (1 unit = 1 m), unit planes | — | [audit](design/world-units-and-scale-audit.md), [unit planes](design/unit-plane-defaults.md) | — |
| Offscreen capture, persistent inspection worker, picking | `src/inspection.rs`, `src/graphics.rs` | [design](design/persistent-inspection-session.md), [evidence](platform/persistent-inspection-macos.md), [picking](platform/viewport-picking-gizmos-macos.md) | Video transport |

## Game project and authored content

| Slice | Design / evidence | Open boundary |
| --- | --- | --- |
| Independent `projects/first_project`, manifest with durable IDs and output declarations | [iterations](history/foundation-iterations.md), [visual run](design/first-project-visual-run.md) | Multi-output build/packaging |
| Schema-1 scene loading: primitives, lights, empty nodes, terrain, structures, particles, environment (AS-1) | [design](design/authored-scene-loading.md), [evidence](platform/authored-scene-loading-macos.md) | Nested compositions, behaviour links, type registry/exports |
| Instance transform overrides, local names, hierarchy (AS-2, SG-1, IDE-NAME) | [transforms](design/scene-transform-inspector.md), [graph](design/scene-graph-authoring.md), [names](design/object-renaming.md) | Undo/redo, preserve-world reparent |
| Saved terrain, sculpted fields, terrain colliders (TERRAIN-3) | [design](design/saved-scene-terrain.md), [evidence](platform/saved-terrain-macos.md) | — |
| IDE Run uses the selected scene (`--run-scene`) | [archive](history/epic-log-2026-10.md) | Native debugger attach, play/pause/step |
| Player settings file (`player.settings.toml`: resolution, target/min fps, AA) | IDE `/player`; **no design or evidence record** | **Not read by the engine or first project** — authoring only |

## IDE (`src/emergent-ide`)

| Area | Delivered | Design / evidence |
| --- | --- | --- |
| Project studio | File tree, Project Map, scene-first hierarchy with drag/drop parenting, TOML editing with revision guards | [scene viewer](design/ide-scene-viewer.md), [graph](design/scene-graph-authoring.md) |
| Inspection viewport | Native paused worker that follows the Scene tab, 30 Hz, orbit/pan/zoom, picking, XYZ gizmos, Local/Global, F / Shift-F | [picking & gizmos](design/viewport-picking-gizmos.md), [focus](design/editor-focus-align.md) |
| Live inspector | Transform, physics, material, camera and noise sections with automatic save after 300 ms (IDE-LIVE-1) | [archive](history/epic-log-2026-10.md) |
| AI workflows | Scoped chat, Statements, reuse-first Implement, chat that can apply changes | [statements](design/ide-statements.md), [implementation](design/statement-implementation.md), [chat](design/actionable-project-chat.md) |
| Run control | Cargo Run/Stop, diagnostics, bounded logs | [design](design/ide-local-run-control.md) |
| Project MCP server (MCP-1 read, MCP-2 edit) | `/api/mcp` + `scripts/mcp-stdio.mjs`; durable `asset:` / `scene:` / `instance:` refs. Read: `find`, `list`, `describe`, `view`, `project_overview`, `history`. Edit: `set_transform`, `rename`, `reparent`, `delete`, `place_asset`, `set_material` / `set_physics` / `set_camera`, `set_environment`, `assign_ids`, `undo`; all revision-guarded, Rust-validated, dry-runnable and journalled. Live push to inspection and IDE refresh over SSE. Verified with Claude Code and Codex. | [design](design/project-mcp-server.md), [evidence](platform/project-mcp-server-macos.md); MCP-3 next |
| AI provider selection | Claude (Anthropic Messages, structured output, server-side fallback) or Codex (OpenAI Responses); per-provider model, URL and key | [evidence](platform/project-mcp-server-macos.md) |
| Add asset | Categorized templates, optional scene inclusion | [design](design/ide-asset-creation.md) |
| Object Creator `/objects` | fal TRELLIS image-to-3D, recoverable jobs, GLB validation, bundle save | [design](design/object-creator.md); browser preview only, no native GLB |
| Object Builder `/object-builder` | Primitive palette, face/vertex editing, extrude/inset, save/place/reopen | [design](design/object-builder.md) |
| Terrain Builder `/terrain` | Height Field / Ray March switch, biome picker, sculpt, paint, library, 1024 grids, live ray-march preview | [terrain plan](plans/terrain.md) |
| Materials `/materials` | Shared material library authoring | [design](design/material-library-terrain-paint.md) |
| Textures `/textures` and Images `/images` | FLUX texture generation; unified image/sprite/video/skybox library, background removal, atlas baking, sprite families | [textures](design/texture-creator.md), [images](design/image-generator-library.md), [media](platform/unified-media-library-macos.md), [sprite variants](platform/sprite-animation-variants-macos.md) |
| Particles `/particles` | Particle Factory: emitters, sprite library, saved effect carousel | [design](design/particle-generator.md), [library](platform/particle-factory-library-macos.md) |
| Tweens `/tweens` | Curve editor, save/reopen | [design](design/tween-editor.md) |
| Lighting `/lighting` | Scene ambient/background/skybox editor with native preview | [design](design/scene-lighting.md) |
| Player `/player` | Player display/render targets (see authored-content caveat) | — |
| fal integration | Searchable model selection with schema checks; shared balance badge | [models](platform/fal-model-selection-macos.md), [balance](platform/fal-balance-header-macos.md), [sprite quality](platform/sprite-api-quality-investigation-macos.md) |

## Known gaps between docs and code

- **Player settings** are saved but have no runtime consumer.

- **Scene lighting/skybox** and the **unified terrain builder** have design briefs but
  no `docs/platform/` evidence record.
- The **bounce-height test** fails against the current authored main scene.
- The capability catalogue (`capabilities/engine-capabilities.toml`) and the HTML guide
  (`docs/`) were not re-audited in the 2026-10-30 consolidation.
