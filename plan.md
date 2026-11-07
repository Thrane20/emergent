# Emergent plan

Last updated: **2026-11-07**. This is the live work queue. Keep it short: move a
finished item into [docs/implementation-status.md](docs/implementation-status.md) in
the same change that finishes it. Long-range goals and stage gates are in
[GOALS.MD](GOALS.MD); the documentation map is in [docs/README.md](docs/README.md).

## Now

Object Builder’s carousel, attribute cards (collapsed AI chat at the bottom, with a taller preview), draft AI conversation, local/global part controls, primitive mesh dimensions, size baking, orthographic X/Y/Z viewer framing, line editing, midpoint multi-selection drag and compact context actions, same-parent grouping, delete/duplicate shortcuts, per-part materials (drag-and-drop swatches, rendered natively as material sections) and separate save/placement workflow are delivered; [evidence and remaining validation](docs/platform/object-builder-ui.md).

- **Skeletal animation (SKEL): follow-ups.** SKEL-1…6 are delivered: skinned GLB/VRM
  import and GPU skinning, clips and `emergent.animator`, humanoid maps, BVH mocap and
  retargeting, crossfade and idle/walk/run speed blending with in-place clips, two-bone
  foot IK on static ground, head look-at, turn to movement, the `biped` locomotion type,
  Shift-to-run, tick/key scene previews, and a template mannequin with a playable
  `third-person` scene (WASD, Shift, Space); SKEL-6 adds IDE import of rigged models and
  BVH takes (engine-checked), a rig report, a skin-weight view and Meshy auto-rig on fal
  ([spike](docs/research/auto-rig-providers.md)). A live run took a generated character from
  FLUX → Trellis → Meshy rig to walking and running in the template scene (~$0.25). Open: foot locking (feet slide when clip and actual speed differ), hand
  IK/FABRIK, IK on moving bodies, non-upright characters, IDE tick previews, real
  ray-cast cost in the benchmark, hands-on play of the native window, and real mocap
  takes for the mannequin. [Plan](docs/plans/skeletal-animation.md).

- **Object classification follow-ups.** Stable 32-slot layers/tags, inspector authoring and runtime mask matching are delivered. The object inspector now uses compact cards and a fixed searchable Add component action above Statements, styled through the primary theme colour (purple in Charcoal, with cyan secondary accents). Next: explicit collision-group and camera/brain filtering integration. [Design](docs/design/object-labels.md).

- **Shader editor follow-ups.** SHADER-1 supplies bounded graph assets, native materials, AI draft generation and three starter effects. SHADER-2 replaces the node list with a wired node canvas (palette, drag-to-connect, saved card positions) and adds the shared shader carousel, blank creation, example loading, fixed preview/scrolling controls, graph zoom/fit, and MCP discovery and editing (`list_shader_nodes`, `describe`, `create_shader`, `edit_shader`, shader refs in `set_material`). Next: texture/normal and vector parameters, impact/gameplay inputs, depth/refraction, matching animated shadows, and measured pipeline sharing. [Design](docs/design/shader-editor.md).

- **Physics authoring follow-up.** Collider/body attribute edits now update the retained inspection world and outline without restarting its worker. Empty model-part roots now fit their collider to descendant geometry, and kinematic locomotion defaults to a Static collider in the inspector. Validation currently re-reads the saved scene; narrow parsing if measured edit latency warrants it.

- **First-person follow-ups.** FPS-1 supplies captured mouse look, camera-relative WASD, supporting-contact jumps, a reusable rig and collision walk fixture. Next: hands-on sensitivity/capture calibration, capsule/stair and moving-platform policies. [Design](docs/design/first-person-controller.md).

- **Terrain Performance v2.** Distance-aware ray marching and mesh/cluster frustum
  culling are delivered ([measurements](docs/benchmarks/terrain-performance/v2/README.md)).
  Long-step ray tracing and crack-free heightfield distance LOD are delivered too:
  ray routes −74 to −83%, heightfield 1024 −62 to −64% main GPU p50.
  Next: LOD popping check and shadow timing, then the meshed-density comparison.
  [Checklist](docs/plans/terrain.md#current-priority--terrain-performance-v2).
- **Continuous terrain world.** Stage 1 is complete: `TerrainWorld` descriptor,
  seamless chunk preparation (shared lattice, halo normals), world-space height-band
  materials, and a rendered 2 × 2 fixture identical to one merged mesh.
  Next: stage 2 chunk storage, incremental rendering and visibility. [Design](docs/design/terrain-world.md),
  [stages](docs/plans/terrain.md#1--seamless-sampling-and-world-assets).
- **Bounded meshed-density comparison.** GPU pass profiling and native offscreen
  camera routes are delivered. Compare a mesh extracted from the same field with
  direct ray marching, including overhang/cavern quality and collision suitability.
  Research focused Rust extraction options and write the design before choosing
  an implementation. Keep gameplay and full resource budgets open.
  [Detailed checklist](docs/plans/terrain.md#current-priority--terrain-performance-v1).
- **Project MCP server: MCP-3 (build and run).** MCP-1 (read) and MCP-2 (edits)
  are delivered, including the embedded chat adapter for configured AI providers
  ([evidence](docs/platform/embedded-mcp-chat.md)). Next: `create_asset` from templates, `validate` / `run` /
  `run_status`, and the IDE selection as a resource, so that "this" or "the selected
  cube" resolves to a ref. [Design](docs/design/project-mcp-server.md).

- **Locomotion: physics flight.** FLIGHT-0 baseline/contract and FLIGHT-1 reusable
  PID and FLIGHT-2 physical body boundary are delivered: **3/8** gates. Next is
  FLIGHT-3: the powered 3D pilot (cascaded control, actuator limits, persistent
  flight goals and hold). The boundary profile is loadable but unpowered. [Stage gates](docs/plans/physics-locomotion.md).
  Wheel rig delivered (wheels steer and roll from the car's activity); brake lights wait for runtime material overrides.
  Physical fixed wing FW-1/2/3 delivered (take-off, climb, holds, manual/aerobatics, circuit and PID-power landing on a map zone; editor Circuit preview with phases, Take off/Land brain nodes, MCP preview; ~1 µs per aircraft per tick). Airports (AIR-1/2) delivered: `aeroway` tags on scene-map items (runway, taxiway, holding position, stand, tower), a control tower sequencing arrivals (circuit capacity, holding stack, base-turn spacing, landing clearance, go-around re-sequencing) and departures (taxi, hold short, line up, centreline take-off), right/left circuits, Depart/Arrive brain nodes and Airport map tools. AIR-3a delivered: scene `[wind]`, air-relative flight with crabbed crosswind approaches, towers landing into the wind, and a scene-map airport overlay (landing direction, circuit legs, join point, holding fix, wind) with wind controls and MCP `set_wind`. Next: AIR-3b propeller/control-surface visuals, gusts, live traffic on the map. [Design](docs/design/fixed-wing-physics.md), [airports](docs/design/airport-operations.md).
  Biped (legged, with SKEL-5 foot IK) is delivered. Other types remain queued: reversing,
  guided missile, bomb drop, helicopter and quadruped. [Current design](docs/design/locomotion.md).
- **Brains: follow-ups.** Editor, runtime, preview, starter brains, MCP authoring
  tools, AI creation directly from an empty draft, duplicate catalogue ID validation
  and opt-in altitude arrival completion are delivered. Next: a neighbour grid before
  flocks above ~300 agents (0.53 ms/tick at 256, quadratic), and the remaining planned nodes
  (pathfinding, obstacle avoidance, terrain blocking sight). [Design](docs/design/brain-editor.md).
  **Brain modes:** delivered (engine, Taxi/Hold course nodes, IDE mode panels, MCP). Next:
  a run-time control channel so the IDE/MCP can show and switch modes in a running game; scene
  validation of Switch mode names. [Design brief](docs/design/brain-modes.md).
- **Models with movable parts (MODEL-1): follow-ups.** Engine import and the Model Parts
  editor are delivered. Next: one live fal split (Tripo Segment and Hi3D) to confirm the
  real output shape, exact triangle clipping for plane cuts, and sharing one texture
  across a part set instead of embedding a copy in each part. [Plan](docs/plans/model-parts.md).
- **Projects: follow-ups.** PROJ-1 (new/open/duplicate/save), PROJ-01 (engine
  host and standard template) and PROJ-02 (independent games, explicit project
  selection and engine-owned validation fixtures) are delivered. Next: a versioned
  host contract. Game-specific demo wiring belongs in its own project.
  [Plan](docs/plans/projects.md).
- **Object components: follow-ups.** Slices 1–4 are delivered (light, controller,
  spin, follow, project types, input). Next: move the spin/bounce/avoidance wiring for
  `main` and the `bda577fe…` scene onto authored components once those scenes' current
  edits are settled; asset-level component defaults; live component push to inspection.
  [Design](docs/design/object-components.md).

- **Audio (AUDIO-1): follow-ups.** AUDIO-1a–1e are delivered (engine, clips, streaming,
  scene components, play on signal, reverb, ducking, limiter, `/audio` editor, fal
  generation and the MCP `generate_audio` tool). Open: occlusion and reverb zones, music
  transitions/crossfades, and a voice picker that lists ElevenLabs account voices.
  [Plan](docs/plans/audio.md).

- **Scene map (MAP): follow-ups.** MAP-1 (engine: waypoint, zone and metadata components,
  zone costs and layers, map queries, A* routes, 4,096-instance scenes), MAP-2 (the
  `/map` top-down editor), and prefab-local map editing are delivered. Next MAP-3: brain nodes (in zone, avoid costly
  zones, follow waypoint path), an MCP `query_map` tool, zones and links in the 3D viewer,
  and faster validation near the instance limit (the IDE runs the game's debug build).
  [Design](docs/design/scene-map-editor.md), [prefab maps](docs/design/prefab-map-editor.md).

## Known issues

- **Model Parts box cuts:** rectangle drawing, eight-corner resizing and Save To Object Library (stable prefab, library tile and whole-object placement) are delivered. Cuts remove surfaces and leave open edges; caps and large-model interaction timing remain unverified/out of scope. [Cut evidence](docs/platform/model-box-cut.md), [library evidence](docs/platform/model-parts-library.md).

- **IDE test flake (2026-11-03):** `ui-assets.test.ts` ("UI saves and AI edits validate natively…") fails only in the parallel `pnpm test` run: its staged statement check reports `first-person.scene.toml` missing from the stage folder. It passes alone, alongside the other Cargo-backed tests, and with `--test-concurrency=1`; reverting the plugin snapshot change did not affect it. Cause not found.
- **Vercel AI Gateway:** provider selection, live model discovery and manual header balance refresh are delivered. The configured key authenticated for read-only credits; Claude Sonnet 4.6 was reported `plan_restricted` despite a positive balance. Authenticated generation and paid-tier entitlement remain unresolved. OpenAI/Anthropic remaining credit is not exposed by the configured inference keys; the header links to billing. [Gateway evidence](docs/platform/vercel-ai-gateway.md), [balance evidence](docs/platform/ai-balances.md).

- Player settings (`/player`, `player.settings.toml`) are not consumed at runtime.
  Either wire resolution/fps/AA into the native run, or label the page as planned.
- No evidence record exists yet for scene lighting/skyboxes.
- The Line, Convex polygon and Indexed mesh templates write a `[geometry]` table the
  scene loader doesn't read, so they're marked planned. Point is now a placeable empty
  entity (`marker.point`). Look-at and path-finding to scene targets aren't built yet.
- `assets/textures/c6dfc7dc-…/explosion_atlas.png` still has no durable ID. Run the
  MCP `assign_ids` tool (dry-run first) to give it one; this hasn't been applied to
  the real project yet.

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
5. Authored composition follow-ups: recursive prefab composition, linked instance
   updates/overrides, authored behaviour-port links and schema/graph exports with
   structured diagnostics (Stage 2A). Named snapshot prefabs, drag/keyboard authoring and runtime label lookup
   are delivered, with sidebar cards, collapsed read-only project files, persistent header drop target, prefab-origin icons, searchable Add Asset prefab placement and consistent empty optional wheel-list validation; [design](docs/design/prefabs.md).
6. Broader Stage 0 budgets: target hardware, workload sizes and frame/memory budgets, so
   that performance claims have something to be measured against.

Project MCP server, after MCP-3: MCP-4 (cost-confirmed generators), then play-session
control once engine pause/step exists.

UI Designer UI-1 is implemented, including generation from a new unsaved starter UI,
automatic opening of the generated asset, chat visibility/drafting and MCP UI
discovery, widget catalogue, guarded create/edit, layout inspection and live refresh; see
the [delivery ledger](docs/implementation-status.md).
UI rendering now supports screen overlays and world-space panels, grid spans and
content auto sizing. Next: pointer/ray picking, action dispatch and value binding,
then focus/navigation, text input and richer fonts. [Design](docs/design/ui-designer.md).

Scene object duplication is delivered: D or the row Duplicate button copies a subtree,
selects it and advances the name suffix. [Validation](docs/platform/object-duplication-macos.md).

- **Plugins (PLUG-1…5)** — static-crate plugins that register components, brain nodes and locomotion types, then packaging/install. Design: [docs/design/plugins.md](docs/design/plugins.md). PLUG-1…3 delivered (code seam, brain/locomotion hooks, in-project plugin folders); next PLUG-4 package and install.

## Backlog

Grouped by area. Each item is open; see the linked plan or design for scope.

- **Terrain** — continuous-world stages 0–6, volumetric/Marching Cubes research,
  hydraulic erosion, slope/biome material rules: [terrain plan](docs/plans/terrain.md).
- **Rendering** — spot shadows, cascades, culling/batching, polylines and debug overlays,
  native GLB import and materials, HDR/IBL, post-processing.
- **Physics** — kinematic bodies, sphere/capsule/convex shapes, triggers and events,
  public queries, joints, character controllers.
- **Animation** — tween bindings to scene properties from the inspector; skeletal
  animation after SKEL-6 (see Now): weight painting/joint placement, MCP import and auto-rig tools, morph targets, animation state graph editor, ragdoll,
  clip compression, skeleton LOD, compute skinning: [skeletal animation plan](docs/plans/skeletal-animation.md).
- **Authoring/IDE** — undo/redo, preserve-world reparent, snapping and multi-select,
  text-to-reference-image for Object Creator, model placement in scenes.
- **Platform** — Windows/Linux native runs, multi-output build and packaging,
  minimum OS policy.
- **Audio** — AUDIO-1 in progress (see Now). **Input mapping** — not started; see the capability catalogue.
- **UI** — pointer input/action binding and later navigation/text entry remain open;
  asset authoring (UI-1) is implemented. [Design](docs/design/ui-designer.md).
