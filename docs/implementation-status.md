# Emergent implementation status

Last reconciled: **2026-10-30** against source, tests and evidence records.
UI-1 and PREFAB-1 implementation and validation added **2026-11-01**; the snapshot below remains dated.
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

## Plugins

| Slice | Source | Decision / evidence | Open boundary |
| --- | --- | --- | --- |
| PLUG-1 | `plugin.rs` (`Plugin`, `PluginRuntime`, `PluginRuntimes`, `registry`), namespaced `ComponentRegistry::register_in`, `ProjectGame::plugins()` and the host's `Plugged` wrapper, standard template comment | [Design](design/plugins.md); `tests/plugins.rs` (namespace rules, missing plugin rejects the scene, fixture plugin moves an object 2 m in 60 ticks); workspace tests and Clippy pass; template `--headless`/`--export-components` and `first_project` build unchanged (2026-11-03). No benchmark yet. | Plugins are listed by hand in `plugins()`; folders, manifests, brain/locomotion hooks and packaging are PLUG-2…5. A failing plugin runtime stops the game but the process exits 0. |
| PLUG-2 | `plugin.rs` (`BrainNode`, `LocomotionModel`, `Params`, `Hook`, registration checks), `brain.rs` (`Op::Plugin`, `*_with(registry)` loaders/catalogue), `locomotion.rs` (`Model::Plugin`, `*_with`), scene loading and host brain/locomotion tools use the project registry | [Design](design/plugins.md); `tests/plugins.rs`: registration rules, plugin node + type loaded from a scene move an object (capped at the plugin's 2 m/s); ignored benchmark `measure_plugin_node_tick_cost` shows plugin node = built-in Seek within noise (numbers in design). | Steering leaves and kinematic movers only; packed fields (≤16 numbers/bools). |
| PLUG-3 | `project.rs` (`PluginManifest`, `ProjectPlugin`, `asset_roots()`), host `check_plugins`, `templates/plugin/`, standard template `src/generated/plugins.rs`, IDE `project-plugins.ts`, `/plugins`, `/api/plugins`, duplicate/statement-check path fixes | [Design](design/plugins.md); `tests/plugins.rs` (manifest loading, asset roots, 8 rejection cases), host unit test (compiled vs listed), IDE `project-plugins.test.ts` (create, rollback, bad entries); end-to-end 2026-11-03: template-derived project + IDE-created `acme.beans` builds, exports `acme.beans.drift`, validates a scene using a plugin asset, runs `--headless`, stale list exits 1 with the plugin named; staged AI check rejects a broken plugin edit and accepts a valid one; `/plugins` page and API checked on a port-3001 IDE. | No remove/edit UI or MCP tools; IDE `ui-assets` test fails only under the parallel `pnpm test` run (passes alone and serially, unrelated to plugins — see plan). |

## First-person controller

| Slice | Source | Decision / evidence | Open boundary |
| --- | --- | --- | --- |
| FPS-1 | `motion.rs`, `input.rs`, `physics.rs`, shared native host, standard template player prefab and collision scene | [Design](design/first-person-controller.md), [validation](platform/first-person-controller.md) | Upright box; native mouse calibration, capsules, stairs and moving-platform attachment remain open. |

## Scene object duplication

The scene object list supports D and a Duplicate row button. Copies get fresh authored IDs, preserve local settings and children, retain the same parent, remap internal entity/brain links and keep external targets. The selected copy is named `Name (N)` using the next unused suffix. Saves use the existing revision guard and native validation; typing, modifiers and key-repeat do not trigger the shortcut. [Evidence](platform/object-duplication-macos.md).

## Live physics attributes

Collider/body edits in the scene inspector keep the worker, entity handles and camera intact. The saved scene is validated, then only the selected physics component and collider outline are replaced. Save/live failures are reported separately. [Evidence](platform/live-physics-attributes.md). Native Run sessions still consume saved settings when started.

Transform inspector parent selection was removed; reparenting uses the scene graph drag-and-drop controls. TypeScript and focused lint checks validate the UI change; the browser graph smoke was updated but not rerun.

Force-field template correction: new-template selection resumes animation, and the pulse modulates opacity and glow visibly. Browser pixel regression, graph/native shader tests, types and lint pass. [Evidence](platform/shader-editor.md#force-field-pulse-correction).

## Surface shader editor

| Slice | Source | Decision / evidence | Open boundary |
| --- | --- | --- | --- |
| SHADER-1 | `shader.rs`, material/renderer integration, shared host, IDE `/shaders`, shader/material project assets and template showcase | [Design](design/shader-editor.md), [validation](platform/shader-editor.md) | Bounded scalar graph. Texture/normal inputs, depth/refraction, custom shadows and gameplay bindings remain open (the freeform canvas landed in SHADER-2). AI provider calls were mocked for smoke validation. |
| SHADER-2 | `shader.rs` (editor-only `position`), IDE `shader-node-graph.tsx`, `shader-graph.ts` layout/ordering, `mcp-shader.ts` | [Design](design/shader-editor.md#delivered-shader-2) | Node canvas: palette, drag wires between sockets, unplug, loop refusal, keyboard inspector, auto-layout for graphs without positions, top-left zoom and Fit all controls. Shared shader carousel with + New, examples picker, and a fixed preview above scrolling controls. [UI validation](platform/shader-editor.md#library-and-viewport-layout). MCP: `list_shader_nodes`, shader `describe` (nodes, named inputs, outputs, users), `create_shader`/`edit_shader` by node ID with engine validation against every scene, dry run, journal/undo; `set_material` takes a shader ref. No rendered MCP preview of a shader; graph limits unchanged (32 nodes, scalars only). |

## Object layers and tags

Project-configured fixed slots (32 layers and 32 tags), object layer dropdown and tag toggle/multi-select dialogs, configuration dialog, asset defaults/scene overrides, runtime `ObjectClassification` masks and retained inspection edits are delivered. [Design](design/object-labels.md), [evidence](platform/object-labels.md). Collision groups, camera layer masks and automatic brain filtering remain open.

Theme colour roles now expose primary/secondary, hover, soft-surface and foreground tokens. Charcoal uses purple primary and cyan secondary; Add Component is a primary action, shared save/create actions use primary, and focus/selection uses secondary. Original green theme remains available. [Validation](platform/theme-colours.md).

## Object attribute inspector

Main-screen object attributes now use consistent compact cards, with Layers & tags above Transform, independent Physics/Material cards and existing component cards. A separate fixed action row above Statements opens a searchable native Add component dialog with selectable entries, cancellation and preserved required-field validation. [Validation](platform/object-attributes.md).

The Add Asset modal lists and searches current project prefabs, with part counts and scene-root instantiation through the existing revision-guarded placement callback. [Evidence](platform/prefab-picker.md).

Prefab capture regression fixed: explicit empty locomotion wheel lists now match their declared defaults. Required entity lists remain nonempty. Native schema and prefab remapping tests pass; the affected project object passes staged native candidate validation with no project files written. [Evidence](platform/prefab-wheel-lists.md).

## Scene sidebar cards

Scene graph, Prefabs and Project files use independent cards. Project files is collapsed initially and has no delete controls. The prefab header keeps capture/drop visible while collapsed. Prefab assets and newly placed roots have distinct package icons; optional validated `prefab_label` preserves root provenance across reload without a live asset link. [Evidence](platform/sidebar-cards.md).

## Engine foundation

| Slice | Source | Decision / evidence | Open boundary |
| --- | --- | --- | --- |
| Fixed-step loop, bounded catch-up, exact headless ticks | `src/engine.rs` | [iterations](history/foundation-iterations.md) | Interactive pause/resume/step; module builder |
| SlotMap world, hierarchy, dirty transforms | `src/world.rs` | ADR 0001, [transform baseline](benchmarks/iteration-2-transform-propagation.md) | Preserve-world reparenting |
| Standalone `bevy_ecs` components/schedules, SlotMap identity | `src/world.rs`, `src/engine.rs` | ADR 0002–0003, [ECS comparison](research/iteration-3-ecs-comparison.md), [runtime](benchmarks/iteration-4-runtime-integration.md) | Workload acceptance |
| Typed behaviour graph, committed snapshots | `src/behavior.rs` | ADR 0004, [baseline](benchmarks/iteration-5-behavior-graph.md) | Authored links, type registry |
| Audio core (AUDIO-1a): clips, sounds, listener, distance gain, pan, Doppler, voice budget, lock-free mixer | `src/audio/mod.rs`, `examples/audio_bench.rs` | [plan](plans/audio.md); 13 unit tests; bench (M3 Pro, release, 64 sources, 32 voices): update p50 1.9 µs / p99 7.5 µs, 512-frame block p50 51 µs / p99 109 µs of 10.7 ms | Device output, decoding, assets, components, IDE editor (AUDIO-1b–e) |
| Audio device, clips and scene components (AUDIO-1b): cpal output, symphonia decoding, `.audio.toml`, `emergent.audio_source` / `emergent.audio_listener`, background loader, host wiring | `src/audio/`, `src/project_host/visual.rs`, `tests/audio.rs`, `projects/first_project/assets/scenes/audio-demo.scene.toml` | [plan](plans/audio.md); WAV/MP3/OGG/FLAC decode checked; engine plays a scene source end to end headless; demo scene Doppler-range test; native smoke opened 48 kHz stereo output on macOS (not listened to) | IDE Audio editor, fal generation, streaming, gizmos (AUDIO-1c–e) |
| Audio editor and generation (AUDIO-1c/1d): `/audio` library, import (WAV/MP3/OGG/FLAC, sniffed), waveform with trim/loop handles, gain/normalise, fades, Web Audio preview, engine-validated saves (`--inspect-audio`), add sound point to a scene, viewer range rings; fal sound effect / music / speech jobs with candidates and saved provenance | `src/emergent-ide/src/lib/audio-*.ts`, `src/emergent-ide/src/components/audio-*.tsx`, `src/project_host/audio_tools.rs`, `src/project_host/inspection_tools.rs` | [plan](plans/audio.md); `tests/audio-assets.test.ts`, `tests/audio-jobs.test.ts`, `scripts/audio-smoke.ts` (Chromium, port 3001, scratch project); one live generation per mode on 2026-11-02 (ElevenLabs SFX v2 2.0 s, Music 10.06 s, TTS v3 1.65 s), each decoded by the engine | Streaming, play-on-signal, MCP generation, nested-input model adapters (AUDIO-1e) |
| Audio follow-ups (AUDIO-1e): streamed clips (decoder thread, chunk recycling, music streams by default), brain play-on-signal, send reverb on the listener, music ducking under speech, output limiter, MCP `generate_audio` (needs `confirm_paid`), adapters for prefixed schema paths and nested voices (MiniMax, ElevenLabs v4) | `src/audio/mixer.rs`, `src/audio/stream.rs`, `src/audio/asset.rs`, `src/emergent-ide/src/lib/audio-jobs.ts`, `src/emergent-ide/src/lib/fal-models.ts` | [plan](plans/audio.md); streamed output matches the decoded clip sample for sample and loops without a step (10/10 runs); bench (M3 Pro, release, 64 sources/32 voices, 512 frames): render p50 73 µs, 80 µs with reverb; live MiniMax 2.8 HD and ElevenLabs v4 speech kept and decoded (2026-11-02) | Occlusion, reverb zones, transitions |
| Scene map engine (MAP-1): waypoint, zone and metadata components, `Pairs` field kind, zone grid index and queries, A* waypoint routes over zone costs, 4,096-instance / 2 MiB scenes | `src/map.rs`, `src/scene.rs`, `src/component.rs`, `tests/map.rs`, `examples/map_bench.rs`, `src/emergent-ide/src/lib/scene-limits.ts` | [design](design/scene-map-editor.md); 4 map tests; bench at 4,096 instances: refresh ~12 µs/tick, point queries ~0.1 µs, cached route 46 µs | `/map` editor (MAP-2); brain nodes and MCP query (MAP-3); slow debug-build validation at the limit |
| Scene map editor (MAP-2): `/map` top-down editor with engine-drawn outlines, pan/zoom, waypoint, path, link, circle and rectangle zone tools, inspector with metadata pairs, direct undoable edits | `src/emergent-ide/src/components/scene-map-editor.tsx`, `src/emergent-ide/src/lib/scene-map*.ts`, `src/emergent-ide/src/lib/map-tools.ts`, `src/project_host/map_tools.rs` | [design](design/scene-map-editor.md); `tests/scene-map.test.ts`; `scripts/map-smoke.ts` passed in Chromium | Brain nodes, MCP query, 3D viewer overlay (MAP-3) |
| wgpu/winit graphics boundary | `src/graphics.rs` | ADR 0005, [smoke](platform/iteration-6-macos-smoke.md) | Windows/Linux runs |
| Points, lines, convex polygons, indexed meshes; renderer-owned `GeometryId` | `src/geometry.rs` | ADR 0006, [smoke](platform/iteration-8-macos-geometry-smoke.md) | Concave/outlined polygons, debug overlays |
| Eight procedural primitives; outward winding fix (PN-1) | `src/primitives.rs` | ADR 0007, [smoke](platform/iteration-9-macos-primitives-smoke.md) | — |
| Ambient/directional/point/spot lights, eight-light Lambert | `src/light.rs` | ADR 0008–0009, [smoke](platform/iteration-11-macos-smoke.md) | Area/environment lighting |
| Directional shadow map + object cast/receive (SH-1) | `src/graphics.rs` | [design](design/object-shadows.md), [evidence](platform/object-shadows-macos.md) | Cascades, quality budget |
| Point-light shadows, two six-face slots (SH-2) | `src/graphics.rs` | [design](design/point-light-shadows.md), [evidence](platform/point-light-shadows-macos.md) | Spot shadows, more slots |
| Cameras: perspective/orthographic, clip planes | `src/camera.rs` | [design](design/camera-inspector-frustum.md), [evidence](platform/camera-authoring-macos.md) | Public render-extraction concept |
| Materials: flat/smooth/unlit, Blinn-Phong, emission, noise channels, bump (M-1, SURF-1) | `src/material.rs` | [design](design/material-editor.md), [evidence](platform/material-editor-macos.md), [surface quality](design/surface-quality-comparison.md) | PBR, shader graphs |
| Diffuse PNG/JPEG, triplanar/UV projection, per-material X/Y tiling | `src/texture.rs`, `src/material.rs` | [design](design/diffuse-image-materials.md), [evidence](platform/diffuse-images-macos.md) | Shared texture residency |
| Imported models (MODEL-1) | `.glb` import: default-scene meshes with node transforms, normals, UVs and the embedded base-colour image; `kind = "model"` entity assets render with a UV-mapped material unless an authored material overrides it; 100 MiB / 1M vertices / 500k triangles | `src/model.rs`, [plan](plans/model-parts.md) | Live fal split unverified |
| Shared material library, four-slot terrain blend | `src/material.rs` | [design](design/material-library-terrain-paint.md) | Slope/biome rules |
| Seeded Perlin/OpenSimplex/Worley noise (NF-1) | `src/noise.rs` | [design](design/noise-filter-assets.md), [evidence](platform/noise-filter-assets-macos.md) | Filter graphs, value noise |
| Rapier physics: static/dynamic, box + heightfield colliders, child-geometry box fitting for empty group roots (PH-1) | `src/physics.rs`, `src/scene.rs` | [design](design/physics-foundation.md), [evidence](platform/physics-foundation-macos.md) | Kinematic, other shapes, queries/events, joints |
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
| Schema-1 scene loading: primitives, lights, empty nodes, terrain, structures, particles, environment (AS-1) | [design](design/authored-scene-loading.md), [evidence](platform/authored-scene-loading-macos.md) | Recursive compositions, behaviour-port links, schema/graph exports |
| Named prefabs (PREFAB-1): drag subtree capture, scrollable table, drag/keyboard and searchable Add Asset scene placement, internal component/brain remapping and Rust spawning by path or unique label | [design](design/prefabs.md), [macOS evidence](platform/prefabs-macos.md); 170 IDE tests (serial), optimized engine tests and disposable standard-template drag/keyboard browser/native smoke | Recursive assets, linked updates/overrides, spawn performance budgets; audio remains planned |
| Instance transform overrides, local names, hierarchy (AS-2, SG-1, IDE-NAME) | [transforms](design/scene-transform-inspector.md), [graph](design/scene-graph-authoring.md), [names](design/object-renaming.md) | Undo/redo, preserve-world reparent |
| Saved terrain, sculpted fields, terrain colliders (TERRAIN-3) | [design](design/saved-scene-terrain.md), [evidence](platform/saved-terrain-macos.md) | — |
| IDE Run uses the selected scene (`--run-scene`) | [archive](history/epic-log-2026-10.md) | Native debugger attach, play/pause/step |
| Player settings file (`player.settings.toml`: resolution, target/min fps, AA) | IDE `/player`; **no design or evidence record** | **Not read by the engine or first project** — authoring only |

## IDE (`src/emergent-ide`)

| Area | Delivered | Design / evidence |
| --- | --- | --- |
| Project studio | File tree, Project Map, scene-first hierarchy with drag/drop parenting, TOML editing with revision guards | [scene viewer](design/ide-scene-viewer.md), [graph](design/scene-graph-authoring.md) |
| Inspection viewport | Native paused worker that follows the Scene tab, 30 Hz, orbit/pan/zoom, picking, XYZ gizmos, Local/Global, F / Shift-F, right-click "add an asset here" (Rust `probe` raycast to the nearest mesh or ray-marched surface, then the guarded `place_asset` op, so it is validated, journalled and undoable) | [picking & gizmos](design/viewport-picking-gizmos.md), [focus](design/editor-focus-align.md) |
| Live inspector | Transform, physics, material, camera and noise sections with automatic save after 300 ms (IDE-LIVE-1) | [archive](history/epic-log-2026-10.md) |
| AI workflows | Scoped chat, Statements, reuse-first Implement, chat that can apply changes | [statements](design/ide-statements.md), [implementation](design/statement-implementation.md), [chat](design/actionable-project-chat.md) |
| Run control | Cargo Run/Stop, diagnostics, bounded logs | [design](design/ide-local-run-control.md) |
| UI assets and UI Designer (UI-1) — implemented | Schema-1 `*.ui.toml`, eight widgets, bounded Rust validation/layout, exported catalogue and shared TS/Rust layout fixture; `/ui` library, canvas, drag/resize, hierarchy, inspector, action labels, screen/perspective previews, revision-guarded native saves and AI edits; scrollable workspace with multiline AI drafts preserved through the first save; generation from an unsaved starter panel and automatic opening of the new UI; MCP discovery, widget catalogue, create/edit by node ID, paged layout inspection, dry-run/undo and external-change refresh with dirty-draft protection | [design](design/ui-designer.md), [evidence](platform/ui-designer-macos.md); in-game rendering, input and action dispatch remain UI-2/3 |
| Locomotion | Profiles (`*.locomotion.toml`) for Slide, Wheeled, Drone, Fixed wing, Ballistic and Rocket; kinematic runtime after brains publishing activity (steer, throttle, brake, bank, rotor, thrust, fuel); brain intent (target, brake, hover, boost, altitude) and Locomotion brain nodes; `emergent.locomotion` component; `/locomotion` editor with rig view and Rust preview. Wheel rig: `steering_wheels`/`rolling_wheels`/`wheel_radius` on the component turn and roll wheel children each tick (`WheelRig`), loader-validated, settable from the inspector and MCP `set_component` | [design](design/locomotion.md), [wheel rig](design/locomotion.md#wheel-rig-loco-wheels--brief) |
| Flight-control foundation (FLIGHT-0/1/2) | Fixed-step scalar PID with filtered derivative, actuator-aware anti-windup and zero-allocation stepping; unpowered physical profile/body ownership, transient force/torque, committed state and reconstruction reset handling. Full drone pilot remains planned | [contract](design/flight-control.md), [baseline](benchmarks/flight-baseline.md), [checks](platform/flight-control-macos.md) |
| Brains and Brain Editor | Stacked brains per object with Movement, Look and Actions channels (Look at, Look along movement, Scan, Signal, On signal; `World::signals`); steering-tree brains (`*.brain.toml`, 45 nodes in 9 headings incl. Battle, line of sight and hide against box colliders), `emergent.brain` / `emergent.group` components, deterministic runtime in the fixed tick, Rust preview sandbox, opt-in altitude arrival completion for sequences, `/brains` editor with library, tree, inspector, apply-to-object and AI chat that can create directly from an empty named draft; starter Prey/Predator/Guard/Swarm/Escort/Soldier/Skirmisher/Scout | [design](design/brain-editor.md); MCP tools and neighbour grid next |
| Object components | Registry with schemas (built-in + project types), exported by the game; authored `[[entities.components]]` validated by the loader; `emergent.light`, `emergent.character_controller` (WASD, camera-relative, dynamic-body drive + jump, optional drive force with friction coasting), `emergent.spin`, `emergent.follow`, camera behaviours `emergent.look_at` and `emergent.follow_camera`; project type `project.jumping_bean`; per-tick keyboard input; inspector Add Component / schema forms; MCP `list_component_types` / `set_component` | [design](design/object-components.md) |
| Physical fixed wing (FW-1, FW-2, FW-3) | `src/fixed_wing.rs`, `Model::FixedWingPhysics` and aircraft preview in `src/locomotion.rs`, Take off/Land nodes in `src/brain.rs`, friction-0 rule in `src/physics.rs`, editor Circuit course and phase readout, MCP `preview_locomotion` phases/touchdown; `tests/fixed_wing_physics.rs` (11), `tests/mcp-vehicles.test.ts`, `examples/fixed_wing_bench.rs` (~1 µs/aircraft/tick) | [design](design/fixed-wing-physics.md) |
| Wind and airport map overlay (AIR-3a) | `src/wind.rs` (scene `[wind]`, `Wind` resource), air-relative pilot with crab in `src/fixed_wing.rs`, into-wind runway choice and `airport_views` in `src/tower.rs`, `--map-layout` airports/wind, scene-map overlay and wind controls, MCP `set_wind`; tests in `tests/fixed_wing_physics.rs`, `tests/airport.rs`, IDE `tests/scene-map.test.ts` | [design](design/airport-operations.md#delivered-air-3a-wind-and-the-map-overlay) |
| Fixed-wing slow-flyer stability fix | Heading→bank loop scheduled with airspeed (turn-rate command) and climb demand capped by spare thrust; a 5 m/s lift-off profile no longer wing-rocks (138 bank reversals → 0 in the circuit) or bleeds to the stall. | `src/fixed_wing.rs`; `tests/fixed_wing_physics.rs` (`a_slow_flyer_holds_its_wings_level_and_its_speed_in_the_circuit`) | [design](design/fixed-wing-physics.md) |
| Airports and control tower (AIR-1, AIR-2) | `src/tower.rs` (airports from `aeroway` tags, tower dispatch), pilot circuits/taxi/centreline take-off/gates in `src/fixed_wing.rs`, Depart/Arrive nodes in `src/brain.rs`, IDE Airport map tools; `tests/airport.rs` (5: departure, five sequenced arrivals with no go-arounds, shared runway, right circuit, brain sortie) | [design](design/airport-operations.md) |
| Project MCP server (MCP-1 read, MCP-2 edit) | `/api/mcp` + `scripts/mcp-stdio.mjs`; durable `asset:` / `scene:` / `instance:` refs. Read: `find`, `list`, `describe`, `view`, `project_overview`, `history`. Behaviour: `list_brain_nodes`, `list_locomotion_types`, `create_brain`, `edit_brain`, `write_locomotion`, `preview_brain`, `preview_locomotion`, components by ref. Edit: `set_transform`, `rename`, `reparent`, `delete`, `place_asset`, `set_material` / `set_physics` / `set_camera`, `set_environment`, `assign_ids`, `undo`; all revision-guarded, Rust-validated, dry-runnable and journalled. Live push to inspection and IDE refresh over SSE. Verified with Claude Code and Codex. | [design](design/project-mcp-server.md), [evidence](platform/project-mcp-server-macos.md); MCP-3 next |
| AI provider selection | Claude (Anthropic Messages, structured output, server-side fallback), Codex (OpenAI Responses), Vercel AI Gateway (Responses), or local Ollama (Responses, keyless activate/deactivate, selectable localhost port with live status and installed-model list); isolated per-provider model, URL and key. Live Gateway catalogue filters tool/structured language models, with manual ID entry and runtime model changes on save. | [existing providers](platform/project-mcp-server-macos.md), [Gateway research and validation](platform/vercel-ai-gateway.md); authenticated Gateway generation unverified; Ollama 0.34.2 full MCP chat turn verified with qwen3-coder (text-emitted Qwen tool calls are recovered IDE-side) |
| AI account balances | Configured providers beside fal across shared headers. Manual refresh reads Vercel USD credit; OpenAI/Anthropic link to billing because configured inference keys do not provide a supported remaining-credit API. No AI balance polling; settings changes invalidate the snapshot. | [research and validation](platform/ai-balances.md) |
| Embedded MCP chat | In-memory MCP client shares server tools/instructions with external clients; Claude and OpenAI tool/result loops, image results, project-scoped context/resources, sequential calls, revision checks, change receipts after partial failure, bounded execution and terminal checked file-change handoff. Existing structured editor/Statement requests retain their scoped workflow. | [design](design/embedded-mcp-chat.md), [local test evidence](platform/embedded-mcp-chat.md); paid provider and browser smoke unverified |
| Add asset | Categorized templates, optional scene inclusion | [design](design/ide-asset-creation.md) |
| Object Creator `/objects` | fal TRELLIS image-to-3D from a project library image (or a disk upload), recoverable jobs, GLB validation, bundle save | [design](design/object-creator.md); browser preview only, no native GLB |
| Model Parts `/models` | Split a generated model with fal (Tripo Segment, Hi3D Split; recoverable single-submission jobs) or by plane cuts; draw and resize an oriented 3D box with eight corner handles, then remove enclosed surfaces from intersected parts with clipped triangles and preserved UVs/normals; merge, rename, parent, pivot; Save To Object Library publishes a stable reusable prefab and natively validated part set (`parts/parts.toml`, per-part GLB and `kind = "model"` entities, group entity); place as a scene hierarchy | [plan](plans/model-parts.md), [box cut evidence](platform/model-box-cut.md), [library save/placement](platform/model-parts-library.md); open cut edges, no caps; live fal split unverified |
| Object Builder `/object-builder` | Primitive palette, face/line/point editing, extrude/inset, Shift-click multi-selection, midpoint viewport gizmo and joint drag, Backspace delete, D duplicate, compact viewport/graph context actions and same-parent empty grouping, geometry-preview carousel, right-hand graph/transform/material cards and collapsed bottom AI chat, local/global part controls, orthographic X/Y/Z axis framing, AI mesh dimensions and undoable size baking, AI conversation from blank drafts, Save and separate tile-context scene placement | [design](design/object-builder.md), [browser/native and simulated-provider evidence](platform/object-builder-ui.md); paid AI generation unverified |
| Terrain Builder `/terrain` | Height Field / Ray March switch, biome picker, sculpt, paint, library, 1024 grids, live ray-march preview | [terrain plan](plans/terrain.md) |
| Projects (PROJ-1, PROJ-01, PROJ-02) | Top-bar project switcher: new from `templates/standard` (own git repo and Cargo workspace, rewritten engine path), open by path or recent list, duplicate, save as a project-scoped git commit; `.data/workspace.json` active project, `EMERGENT_PROJECT_PATH` pins. IDE tool contract hosted by the engine (`emergent::project_host`, `ProjectGame`); ordinary games call `project_host::main`; engine workspace contains only engine/template, no implicit sample fallback, and tests/smokes use engine-owned fixtures | `src/project_host/`, [plan](plans/projects.md), [isolation evidence](platform/project-isolation-macos.md) |
| Materials `/materials` | Shared material library authoring; swatch carousel and rotating sphere/cube/torus browser preview (Texture Creator removed 2026-10-30; textures come from Images) | [design](design/material-library-terrain-paint.md) |
| Images `/images` | Unified image/sprite/video/skybox library, background removal, atlas baking, sprite families | [textures](design/texture-creator.md), [images](design/image-generator-library.md), [media](platform/unified-media-library-macos.md), [sprite variants](platform/sprite-animation-variants-macos.md) |
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

### Brain catalogue duplicate validation

The IDE now rejects duplicate node IDs before they reach the brain library,
inspector or AI context. The previous check treated `Set.add()` as a boolean,
so it never rejected duplicates. Focused IDE tests cover both available and
planned duplicates of `signal`. On 2026-11-02 the running IDE API returned 64
unique IDs with one available `signal`; the reported browser warning was not
reproduced in a browser during this fix.

### Altitude arrival completion

Altitude steps can opt into `finish_on_arrival` with an `arrival_radius` tolerance
(default 0.1 m), so a Sequence can advance from takeoff to landing even when its
child is Hover. Existing altitude hold remains the default. BehaviourTest's Cube
brain enables this for both steps and lands its centre at 0.5 m with a matching
drone ground-height floor. An optimized headless run of the actual scene peaked
at 5.0344 m and settled at 0.5 m with zero speed and the sequence complete.
[Design](design/locomotion.md#completing-an-altitude-step),
[macOS evidence](platform/drone-flight-macos.md).

### Flight-control foundation (FLIGHT-0/1/2)

Delivered 2026-11-02: **3/8** [flight stage gates](plans/physics-locomotion.md).
FLIGHT-0 supplies the reproducible baseline and frozen ownership, goal/contact,
timing and synthetic actuator contracts. FLIGHT-1 supplies `control::Pid`, explicit
time, derivative filtering, bounded integral/output, caller feedforward, actuator
anti-windup, reset and telemetry, without a new dependency. Tests cover scalar
reference cases, no D kick, timestep scaling, saturation/recovery, rollback on
invalid arithmetic, zero allocation, and vertical force-only lift/hold of a real
Rapier body. The `control.pid` capability is preview.

FLIGHT-2 is delivered: boundary-only `drone_physics` loads with a dynamic body
and physics feature, accepts external transient forces/torques, and never writes
a pose. `World::physics_body_state` publishes committed mass/gravity/motion and
reconstruction revisions; pending rebuilds invalidate old feedback. Agent motion
history resets on revision changes; brain self/neighbour feedback uses actual
motion. Scene/runtime checks reject missing/static bodies, unsupported scale and
competing writers. The editor labels/disables the boundary and powered preview
rejects it. The full pilot remains FLIGHT-3; BehaviourTest is unchanged. [Contract](design/flight-control.md),
[baseline measurements](benchmarks/flight-baseline.md),
[validation](platform/flight-control-macos.md), [FLIGHT-2 checks](platform/physics-locomotion-boundary-macos.md).

### Brain AI empty-draft entry

Empty unsaved brains now accept AI questions and creation requests. The draft's
name/description guide existing brain tools, and a newly created brain is loaded
from the refreshed library. Unsaved graphs with nodes keep their send/save guard
while still allowing prompt text to be drafted.
See [design](design/brain-editor.md#ai-from-an-empty-draft). The empty-draft,
creation-context and single-created-brain selection rules have focused IDE tests.

### Nested UI layouts

Horizontal, Vertical and Grid containers and panel layout modes support nesting,
width/height lengths, padding, gaps and main/cross alignment. Legacy panel stacks
are preserved. Source: `src/ui.rs`, IDE `ui.ts` / `ui-designer.tsx`. Design:
[UI layouts](design/ui-designer.md#nested-layout-containers). Evidence and remaining
validation: [layout checks](platform/ui-layouts-macos.md).

### UI rendering and advanced layouts — 2026-11-02

Grid column/row spans, `auto` content dimensions and screen/world UI surfaces are
implemented. Native widget images and bitmap text are prepared once; overlays
composite last and world panels use ordinary entity transforms/depth. The shared
host renders `emergent.ui` in inspection and Run; the designer places either mode
in a scene. The two obsolete first-project authored-scene spin/bounce tests were
removed at the user's request; generic spin, bounce and physics coverage remains.
[Runtime design](design/ui-designer.md#runtime-rendering-spans-and-auto-sizing),
[evidence](platform/ui-runtime-macos.md). Interaction, actions and mutable widget
state remain future work; this slice renders authored state.
