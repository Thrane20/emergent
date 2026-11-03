# Terrain plan

Last reconciled: 2026-10-30 against source (`src/terrain.rs`, `src/raymarched_terrain.rs`,
`src/physics.rs`, `src/emergent-ide/src/components/terrain-*.tsx`).
Status: standalone heightfield and experimental ray-marched authoring are implemented;
ray-marched collision and every continuous-world stage remain open.
Progress: **10 standalone authoring slices implemented; 0 of 7 continuous-world stage exits complete.**

This is the active detailed plan for terrain. The project-wide queue is
[plan.md](../../plan.md); delivered work is summarised in
[implementation-status.md](../implementation-status.md). Checkboxes mark the stated
slice, not production performance acceptance or every extension of a feature.

## Completed standalone terrain authoring

- [x] **Versioned generation:** Hilly, Mountainous, Rivers and Island presets;
  Perlin/noise and diamond-square bases; seed, extent, relief, octaves, roughness,
  bounded domain warp, mountain groups/ridges and conservative thermal erosion.
  Algorithm v2 is the default; explicit v1 recipes retain compatibility.
- [x] **Preview and comparison:** orbit/zoom/reset, smooth normals, grayscale
  terrain, adjustable preview sunlight and matched three-way algorithm comparison
  with preparation timings and output sizes. Comparison remains capped at 256.
- [x] **Sculpting:** top-down soft circular grayscale falloff, raise/lower/smooth/
  set-height flatten, radius and proportional strength, pointer/keyboard painting,
  one-stroke undo and 3D rebuild at stroke completion. Heightmap/controls sit left
  of the 3D preview. Direct 3D painting and imported brush masks are not implemented.
- [x] **Saved authored fields and collision:** validated frozen height samples
  feed both the rendered mesh and optional static Rapier heightfield. Recipe-only
  legacy terrain remains loadable. Sizes/heights are metres; instance scale is a multiplier.
- [x] **Reopen from the main editor:** selected terrain opens in Terrain Builder;
  in-place updates preserve asset/instance identity, placement, materials and physics.
  Shared-asset changes affect every referencing instance.
- [x] **Material library painting:** up to four complete library materials,
  manual soft painting, normalized RGBA blend weights, exact sidecar save/reopen,
  and explicit height-band processing for river bed/ground/mountain/summit with
  soft transitions and undo. Native inspection renders all channels; the builder
  preview shows diffuse colours/images. Height bands do not simulate rivers or biomes.
- [x] **Terrain carousel/library:** project-scoped shaded swatches, New, Load,
  Save, Save as new, Refresh, unsaved-work guard, destination selection and repeated
  Add to scene with fresh instance IDs. Library-only saves leave scenes unchanged.
- [x] **512/1024 grids:** 32/64/128/256/512/1024 samples per side, full 32-bit mesh
  indices, bounded sidecars/requests, packed preview GLBs and bounded generation.
  Increasing resolution regenerates the recipe; it does not resample sculpted edits.
- [x] **Unified builder and biomes:** one Height Field / Ray March switch with a
  shared biome picker (cave/cavern, canyon, underwater seabed, plain, hilly,
  mountainous, island, rivers). Cave/cavern is ray-march only; heightfield gains
  Plain/Canyon/Underwater presets. Ray March has a live native orbit preview through
  a persistent worker. Legacy recipes keep their output. See
  [design](../design/unified-terrain-builder.md),
  [acceptance](../platform/unified-terrain-builder-macos.md) and the latest
  [performance/workflow validation](../benchmarks/terrain-performance/README.md).
- [x] **Publication safeguards:** project-path validation, staged native scene
  checks, terrain/scene/material revision conflicts and coordinated asset publication.
  Full dependency hashing is separate from raw TOML document preview limits.

Evidence: [generation](../../src/terrain.rs), [builder](../../src/emergent-ide/src/components/terrain-builder.tsx),
[brushes](../../src/emergent-ide/src/components/terrain-sculptor.tsx),
[paint](../../src/emergent-ide/src/lib/terrain-paint.ts),
[asset transactions](../../src/emergent-ide/src/lib/terrain-assets.ts),
[library](../../src/emergent-ide/src/lib/terrain-library.ts),
[sculpt/save/reopen validation](../platform/terrain-sculpting-macos.md),
[library acceptance](../platform/terrain-library-macos.md) and
[high-resolution design/measurements](../design/terrain-high-resolution.md).

## Current priority — Terrain Performance v1

Agreed 2026-10-30: measure and reduce the current terrain cost before expanding
world residency. First increment is implemented and validated; the full profiling
and large-world acceptance gates remain open. See [design](../design/terrain-performance-v1.md)
and [measurements / validation](../benchmarks/terrain-performance/README.md).

- [x] Repeatable warmed release capture benchmark with fixed overview, grazing,
  cavern, small projected region and offscreen views; raw timings, p50/p95/p99,
  adapter, viewport and colour-reference comparisons retained.
- [x] Conservative clip-plane rejection and projected scissor rectangles for both
  standalone and scene ray regions; near-plane/inside fallback, transformed and
  orthographic bounds checked. Renderer SlotMap ownership is unchanged.
- [x] Runtime region/submission/scissor-area diagnostics and a culling bypass for
  comparisons. Scissor area is not a GPU fragment invocation count.
- [x] Ray-step heatmap, including magenta iteration exhaustion; trace caps retained.
- [x] Auto / Full / Half preview quality. Auto renders 480×270 during camera changes,
  then restores 960×540 after 250ms without a view command. PNG envelope remains
  960×540; reduced frames are upscaled. Quality does not alter saved recipes.
- [x] Avoid mountain noise overwritten by simpler biomes and zero-strength rock
  noise. Record measured gains and regressions separately from intended benefits.
- [x] Optional main/shadow GPU pass timestamps, serial native offscreen camera
  routes for 1km/4km/eight ray regions, cavern and 128/512/1024 heightfields; exact
  uploaded mesh-buffer counts and separate CPU stages. Native presentation smoke
  remains functional. [GPU evidence](../benchmarks/terrain-performance/gpu/README.md).
- [ ] Native Run frame distributions with simulation/presentation, delivery latency,
  complete memory/allocation budgets and player traversal routes. The new GPU
  benchmark is serial offscreen rendering; the earlier capture benchmark also
  includes pixel readback. Neither is a gameplay frame budget.
- [ ] Evaluate opaque-depth termination after profiling; it needs a pass/depth
  sampling design, not simply reversing mesh and ray draw order.

Next: run a bounded meshed-density comparison against the same authoritative field
before selecting the cave/overhang collision path. This priority is informed by
the GPU measurements: field evaluation dominates the ray routes, while the dense
heightfield has lower GPU cost on this fixture. The representations/materials are
different, so these measurements do not establish equivalent visual quality.
Retain shared-lattice heightfield chunk visibility/LOD (stages 1–3 below) and
complete gameplay/resource budget selection before large-world acceptance.
Add streaming only after visibility/detail and bounded preparation are proven.
No continuous-world stage exit is closed by this increment.

## Remaining authoring work

- [x] **Large-terrain inspector reproduction:** saved 1024 terrain selection,
  enabled transform controls and live native orbit passed on 2026-10-30 without
  the reported preview-limit error. The browser regression script is retained.
- [ ] **Extended large-terrain acceptance:** repeat cold/warm selection and
  collider/four-material editing stress cases. The reproduction check does not
  establish responsiveness or every inspector path under load.
- [ ] **Responsiveness and resource budgets:** measure stroke-release GLB packing,
  paint regeneration, TOML/JSON parsing, save validation, GPU upload and collider
  activation at 1024. Move or split blocking preparation only where measurements
  show a problem; add cancellation/progress where needed. The builder still prepares
  a complete mesh and synchronously packs GLBs on stroke completion.
- [ ] **Material parity and rules:** assess full native-material preview parity,
  slope-based painting, world-space projection and shared texture residency. Current
  height-only processing and four-material paint are complete, not slope/biome rules.
- [ ] **Authoring extensions:** explicitly scope non-destructive resolution
  resampling, imported grayscale brush masks, direct 3D painting and multi-step
  undo/redo before implementation. These are follow-on features, not prerequisites
  for the current standalone workflow.
- [ ] **Generator research:** compare additional heightfield and volumetric
  methods in the final section before adding a generator selector or new asset type.

## Bounded ray-marching milestone — delivered prototype, collision pending

Agreed implementation order (2026-10-30):

1. Close the saved-1024-terrain inspector regression using the native viewer.
2. Prototype one seeded bounded ray-marched region with ground, mountain, cave
   and arch. Use analytical distance fields before dense voxel storage.
3. Prove mesh/implicit depth occlusion, inside/outside cameras and projection;
   document lighting/shadow limits and an explicit collision approach.
4. Measure optimized rendering/capture costs at several viewport sizes. Compare
   heightfield costs only under equivalent conditions; do not claim a frame budget.
5. Integrate authoring/persistence and collision after the prototype boundary is
   validated, then revise continuous-world stages 0–6 for both terrain variants.

- [x] Write [ownership/data-flow/validation design](../design/raymarched-terrain.md).
- [x] Revalidate the large-terrain inspector: saved 1024 terrain selection,
  enabled transform controls and native orbit passed on 2026-10-30.
- [x] Implement validated headless field and native GPU ray-marching pass.
- [x] Validate prototype depth, camera and bounded-work behaviour; record
  [native evidence and capture measurements](../platform/raymarched-terrain-macos.md).
- [x] Add experimental Terrain Builder option and authored scene contract, native
  preview, shared library save/load, scene placement and transform integration.
- [ ] Prototype collision for overhangs/caves using the same authoritative field.
- [ ] Review results before shared residency/streaming implementation.

Terrain Builder exposes ray marching through the top-level **Ray March** method switch.
Schema-1 `raymarched-terrain` assets store the seeded recipe. Ordinary World entities
and renderer-owned typed SlotMap handles support up to eight transformed regions.
Version 2 now defaults to a 1024m biome landscape, with 64–4096m region widths,
world-space biome scale, rolling fields, ridged mountains, 3D rock displacement,
winding carved caves and slope/elevation grass/rock/snow colouring. Version-1
ground/mountain/cave/arch fixtures remain loadable without automatic migration.
Physics/material attachments are rejected;
collision, scene-light/shadow parity, gameplay traversal and equivalent-surface
comparison remain open. Optional main/shadow GPU profiling and bounded camera
orbits are now recorded in the [GPU evidence](../benchmarks/terrain-performance/gpu/README.md).

### Biome landscape increment

- [x] Add versioned biome density generation and world-space feature scale;
  increasing extent reveals more landscape instead of scaling the same fixtures.
- [x] Expose biome scale, mountain relief, overhang strength and cave radius in
  Terrain Builder. New terrain defaults to a 1km region; native 4km capture checked.
- [x] Integrate a shared Height Field / Ray March switch and biome modal with
  cave/cavern, canyon, underwater/seabed, field/plain, hilly, mountainous, island
  and rivers. Heightfields explicitly exclude enclosed cave/cavern surfaces.
- [x] Replace ray-march snapshot camera controls with persistent live native orbit,
  pan, zoom and projection controls, reusing inspection protocol/session ownership.
- [x] Persist explicit biome choices and add distinct CPU/WGSL generation branches.
  Missing biome fields retain earlier mixed v2 output and v1 fixtures stay compatible.
- [x] Preserve old assets as version 1; selecting Biome landscape is an explicit
  recipe change. Library copies remain available before changing shared assets.
- [x] Validate seed/recipe bounds, CPU distance gradient bounds, cave carving,
  non-heightfield overhangs and sampled CPU/GPU coverage on the kilometre fixture.
- [ ] Add editable biome/material assignment, richer geology/erosion and lighting
  parity after measuring the current density evaluation cost.
- [ ] Profile close cave views, grazing rays, extreme parameter combinations and
  representative traversal before claiming a realtime budget. Iteration exhaustion
  remains a bounded miss, not guaranteed convergence for every valid recipe.

See [biome design](../design/raymarched-biomes.md) and
[biome validation](../platform/raymarched-biomes-macos.md).
The unified workflow has a separate [design](../design/unified-terrain-builder.md)
and [acceptance record](../platform/unified-terrain-builder-macos.md).

References researched: Sebastian Lague's [Ray-Marching Unity source](https://github.com/SebLague/Ray-Marching)
and [Marching-Cubes Unity source](https://github.com/SebLague/Marching-Cubes).
The first informs field composition/camera rays; the latter is a starting point
for investigating collision surface extraction. No Unity source has been copied.

Next collision design: compare an extracted static triangle collider from the same
field against direct implicit contacts/queries. Test roof/floor/wall contacts,
multiple surfaces at the same X/Z, tunnel entrances, steep slopes, transformed
regions, surface error, collider preparation time and replacement safety. Retain
World/physics handles and reject heightfield collision for this variant. Select
the extraction method after research and measurement, before adding a collision toggle.

Ray marching traces a field in the shader; Marching Cubes extracts a triangle mesh.
They may share field evaluation but require different rendering/detail strategies.
Keep the seven continuous-world gates below open. Bounds, budgets and lifetimes
are shared concerns; mesh stitching is not a ray-marching LOD solution.

## Outcome and first workload

Build a continuous, finite landscape that the player can cross without scene
changes, visible cracks, missing collision, or loading work blocking the frame.
Use independently managed terrain chunks, distance LOD, visibility culling, and
bounded asynchronous preparation. Expand world size only after this workload passes.

| Item | Initial proposal |
| --- | --- |
| World | 2,048 × 2,048 m; 16 × 16 chunks |
| Chunk width | 128 m |
| Finest grid | 128 × 128 cells; 129 × 129 height samples; 1 m spacing |
| Render LODs | 128, 64, 32, 16 cells per side; same physical extent |
| Finest chunk geometry | 16,641 vertices and 32,768 triangles, before edge stitching |
| Initial continuity fixture | 2 × 2 chunks, then 4 × 4 |
| Streaming fixture | Entire 2 km world traversed with bounded resident resources |
| Observers | One render observer and one physics interest region initially |

These values are test inputs, not measured optimal sizes or performance claims.
Choose render/collision radii and memory caps from measurements in stage 0;
do not assume that all 256 chunks should remain loaded. Continuous means seamless
travel inside explicit world bounds; an infinite world is outside this increment.

## Current evidence and gaps

The following is verified by source inspection, not a new runtime test:

- [Terrain implementation](../../src/terrain.rs): versioned independent patches,
  resolutions 32/64/128/256/512/1024, a dense heightfield, and one complete mesh per patch.
  Defaults are 100 m width and resolution 128. Existing generation uses patch-local
  coordinates; repeating recipes or changing seeds does not establish continuity.
- [Scene loading](../../src/scene.rs) and [saved terrain design](../design/saved-scene-terrain.md):
  scene-load generation or frozen sculpted samples, existing entity/geometry
  handles, and optional static terrain collision. Authored scenes currently cap
  instances at 1,024; runtime residency must have a separate explicit bound.
- [Physics](../../src/physics.rs): Rapier static heightfields share terrain samples and
  grid orientation with rendering. Streaming activation and retirement need a new
  lifecycle contract.
- [Renderer](../../src/graphics.rs): SlotMap-owned geometry with insertion/removal and
  per-instance drawing. General visibility/culling is still listed as planned in
  the [capability catalogue](../capabilities/engine-capabilities.toml).
- [Project rendering](../../src/project_host/visual.rs) uploads loaded scene
  geometry. Streaming needs incremental additions/removals through public engine
  contracts, rather than rebuilding or re-uploading the whole scene.
- Terrain Builder already supports saved edits and material painting. Their
  existing transactions and source validation should be reused; current preview
  and sidecar formats do not establish a runtime streaming format.

## Ownership, coordinates, and compatibility

The engine owns reusable terrain sampling, chunk lifecycle, LOD preparation, and
collision integration. The game chooses its world asset, observer, movement rules,
and loading presentation. The IDE authors and inspects the same engine data.

- A terrain subsystem owns chunk records in a typed SlotMap. A coordinate lookup
  maps `(terrain world, chunk x, chunk z)` to a live chunk key; coordinates are a
  lookup and authored address, not a competing runtime ownership graph.
- Existing `World` owns any chunk entities through `EntityId`; the renderer owns
  `GeometryId`; physics retains its existing ownership. Chunk records hold these
  handles and validate them at activation, replacement, and retirement boundaries.
- Durable terrain asset IDs plus integer chunk coordinates identify saved content.
  Never serialize SlotMap keys. Work requests carry world lifetime, chunk handle,
  and content revision so late results cannot revive deleted or replaced terrain.
- Derive samples from an integer global lattice: `chunk_coord * 128 + local_sample`.
  Adjacent chunks evaluate identical border positions, including negative coordinates.
  Use wider coordinate arithmetic for generation and chunk-local f32 geometry.
- Add a separately versioned world descriptor/grid contract. Its 129-sample grid
  must not silently alter the existing 32/64/128/256/512/1024 patch format, saved samples,
  material resolution validation, or legacy recipe output.
- Begin with an unrotated, unit-scale terrain root; explicitly reject unsupported
  transforms. Define bounds and coordinate precision for the first 2 km world.
  Floating-origin support is a later measured requirement.
- Reuse installed noise, SlotMap, ECS, renderer, and Rapier facilities. Prefer
  standard-library bounded worker queues unless a concrete gap justifies a dependency.

Data flow: validated world asset → requested chunk coordinates → bounded CPU
preparation → revision-checked completion → budgeted GPU upload/collider creation
→ coherent activation → visibility/LOD selection → safe retirement and eviction.
Workers return owned prepared data; they never mutate live world, GPU, or physics stores.

## Implementation stages

### 0 — Baseline and budgets

- [x] Single-patch preparation measurements exist for 256/512/1024 on Apple M3 Pro
  in [the high-resolution record](../design/terrain-high-resolution.md), with
  workload and memory limitations. This is partial stage evidence only: GPU/physics
  activation, gameplay frame percentiles and streaming residency budgets remain open.

- [x] [Bounded GPU camera routes](../benchmarks/terrain-performance/gpu/README.md)
  record main/shadow pass timings, separate serial CPU stages, triangle and uploaded
  mesh-buffer counts. This extends the baseline; presentation, physics, allocator/
  driver memory, chunk crossings and agreed acceptance budgets remain open.

- [ ] Record release-build hardware, GPU/backend, viewport, lighting/shadows,
  movement speeds, and agreed frame-time target. Measure existing single-patch
  generation, mesh preparation, upload, collider preparation, and frame times.
- [ ] Use fixed seeds and routes: stationary, walking, fast boundary crossing,
  repeated backtracking, and teleport. Include flat and mountainous workloads.
- [ ] Set numeric acceptance budgets for CPU/GPU time, p95/p99 frame time, resident
  CPU/GPU bytes, pending jobs, prepared bytes, upload bytes/time, and collider work.
  Include headroom for non-terrain gameplay; average FPS alone is insufficient.

Exit: checked-in baseline and explicit budgets before performance implementation.

### 1 — Seamless sampling and world assets

- [ ] Add the versioned bounded world descriptor and headless chunk preparation API.
  Validate extents, sample counts, numeric overflow, finite values, and project paths.
- [ ] Start with world-coordinate noise and shaping. Do not independently apply
  patch-local diamond-square, islands, mountain placement, or closed-boundary erosion.
  Enable each only after it has a defined world-space continuity contract.
- [ ] Generate a halo for central-difference normals so shared heights also have
  matching normals. Material projection and procedural weights use world coordinates.
- [ ] Test equal borders/corners, negative coordinates, different generation orders,
  reproducible revisions, finite bounds, and legacy fixture compatibility.

Exit: a static 2 × 2 fixture has matching geometry, shading, and material boundaries.
Cross-platform bit-identical floating point is not implied by same-platform determinism.

### 2 — Chunk ownership, incremental rendering, and visibility

- [ ] Add typed chunk storage and explicit requested/preparing/ready/active/retiring
  states with bounded transitions and coherent world/resource changes.
- [ ] Expose the minimal public integration for incremental render resources;
  update the reference project’s upload/extraction path and paused inspection.
- [ ] Add conservative chunk bounds and camera-frustum culling. Define shadow-caster
  selection separately so off-camera terrain can still cast visible shadows.
- [ ] Reuse dense traversal/work buffers. Recompute bounds and cached visibility
  inputs only on relevant terrain, transform, or camera changes.
- [ ] Test stale keys, foreign-world handles, root deletion, repeated reload,
  resource removal, conservative bounds, and visibility at frustum edges.

Exit: 4 × 4 fixed-detail chunks render independently and release their resources.

### 3 — Distance LOD with closed boundaries

- [ ] Derive all LODs from the authoritative finest field; do not regenerate a
  different landscape per LOD. Use shared full-detail normals/material coordinates.
- [ ] Start with distance bands plus hysteresis; constrain adjacent chunks to at
  most one LOD step and implement matching edge indices for all neighbour masks.
- [ ] Define missing-neighbour handling and world-edge appearance. Commit neighbour
  topology changes coherently; keep the previous valid mesh until replacements are ready.
- [ ] Prepare reusable topology off the frame path. Measure visible popping before
  deciding whether geometric morphing is needed.
- [ ] Test every edge/corner orientation, neighbour mask, LOD pair, winding, boundary
  oscillation, and chunk arrival/removal. Keep collision independent of render LOD.

Exit: moving through the fixture changes detail without cracks or LOD thrashing.

### 4 — Bounded streaming and failure recovery

- [ ] Request a bounded render neighbourhood with prefetch and unload hysteresis.
  Prioritize nearby missing terrain and use capped concurrency, queues, caches,
  prepared bytes, and per-frame upload/activation work.
- [ ] Run sampling, loading, and CPU mesh preparation off frame-critical threads.
  Budget required main-thread GPU and physics work explicitly; if one chunk exceeds
  the budget, reduce its work unit or change preparation before claiming success.
- [ ] Cancel obsolete work and discard stale completions. Evict unused CPU/GPU
  resources without removing terrain pinned for physics or unsaved authoring.
- [ ] Define startup/teleport readiness: keep the player at a safe ready location
  or suspend movement/simulation until destination collision is active. Report
  load errors and allow retry without exposing holes or corrupting authored files.
- [ ] Test teleport, rapid direction reversal, worker failure, deleted worlds,
  queue saturation, repeated traversal, shutdown, and bounded memory after churn.

Exit: the 2 km world can be crossed repeatedly within stage-0 budgets.

### 5 — Collision residency and terrain queries

- [ ] Use the authoritative finest samples and existing triangle orientation for
  static heightfields. Activate/remove colliders only at safe physics boundaries.
- [ ] Define a physics interest region around supported moving bodies, distinct
  from the camera. For the first workload use one player/body cluster and cap its
  extent; reject unsupported remote activity rather than silently dropping collision.
- [ ] Size prefetch from maximum supported speed and preparation latency. Retain
  supporting terrain until bodies have left and replacement ground is ready.
- [ ] Add a bounded public height/normal query with explicit available, pending,
  and outside-world results. Specify interpolation using the collision triangles.
- [ ] Test walking/falling across edges and corners, slopes, high-speed travel,
  camera movement away from the player, unload while supported, and teleports.

Exit: streaming and visual LOD never remove the player’s required collision.

### 6 — Editor integration, persistence, and acceptance

- [ ] Add Create/Open terrain world to existing Terrain Builder and scene workflows.
  Persist a world descriptor reference rather than hundreds of authored chunk entities.
- [ ] Add chunk bounds, coordinates, LOD colours, residency/collision state, memory,
  timings, and pending-work counts using existing inspection controls. Allow camera
  inspection and streaming visualization while simulation time is stopped.
- [ ] Keep standalone sculpted terrain usable. For this first world release, make
  world chunks generation-only and disable unsupported world sculpt/paint saves
  explicitly. Cross-border authoring requires shared sample ownership, neighbour/LOD/
  collider invalidation, and transactional multi-chunk saving in a later increment.
- [ ] Add a dedicated temporary fixture/example through public engine APIs; leave
  the user’s existing scenes and assets intact. Update public exports, scene checks,
  capability status, API docs, and project integration together as they become callable.
- [ ] Run the acceptance workload, record evidence and remaining platform limits,
  and update this tally, [plan.md](../../plan.md) and [GOALS.MD](../../GOALS.MD).

Exit: author, save, reopen, inspect, and run a continuous world with recorded
correctness and performance evidence. All seven stage exits must pass for completion.

## Implementation reach and validation

Expected engine reach: `src/terrain.rs`, `src/scene.rs`, `src/graphics.rs`,
`src/physics.rs`, `src/lib.rs`, and existing engine/world scheduling boundaries.
Reference-project reach: its scene loading, render extraction, paused inspection,
and a dedicated world fixture. IDE reach: terrain settings/tool/assets libraries,
terrain API, Terrain Builder, Project Studio, inspection, and their existing tests.
Update manifests/config only when a stage requires them; preserve unrelated work.

For Rust implementation, run formatting, focused regression tests, workspace tests,
all-feature release tests, Clippy with warnings denied, and rustdoc. Add representative
release benchmarks for generation, lifecycle churn, visibility/LOD, and streaming.
For IDE implementation, run affected tests, lint/type/build checks and targeted save,
reopen, and inspection smoke tests. Use port **3001** for test services and shut them
down afterward; the user’s port 3000 service stays available.

Record measured results separately from proposed targets: p50/p95/p99 frame times,
CPU/GPU costs where tooling permits, queue depth, resident/peak bytes, load latency,
uploaded bytes, and collider activation cost. Include cold/warm traversal, shadows,
boundary churn, teleport, and idle-after-load allocation behaviour. Report any
unavailable measurement or untested platform explicitly.

## Deferred scope and risks

Defer infinite worlds/floating origin, caves/overhangs, hydraulic erosion,
network replication, vegetation streaming, multi-observer scheduling, GPU generation,
and cross-chunk sculpting/painting. Assess them after the finite-world acceptance run.

Main risks are patch-local generator seams, LOD topology/shading discontinuities,
per-chunk GPU material duplication, shadow draw cost, collider activation spikes,
unbounded prepared work, and late asynchronous results. The staged gates address
these before expanding scale; chunking alone establishes no performance guarantee.

## Generator research and volumetric terrain — not implemented

Production authored terrain remains a 2D heightfield: one height per X/Z sample.
The separate native ray-marching prototype represents caves and arches through
analytical distance fields. Marching Cubes would extract a mesh from a 3D density
field; that extraction is not implemented by the ray-marching shader.

| Technique | Current status | Remaining work |
| --- | --- | --- |
| Height-biased density | Bounded solid ground prototype implemented | General density composition/noise and authored rules remain open. |
| Domain warping | Implemented for bounded 2D patch generation | Research 3D warping and global-coordinate continuity separately. |
| Ridges / billow | Mountain groups and connecting ridges implemented | Generic ridged/billow fractal modes and 3D density composition remain open. |
| Heightmap/biome masking | Presets and height-band material paint implemented | Spatial biome masks and blended density/generation rules remain open. |
| SDF primitives | Native bounded ground/sphere/tunnel/torus prototype | Editable primitives, saved landmarks and collision extraction remain open. |
| Erosion | Conservative thermal heightfield erosion implemented | Hydraulic water/sediment simulation and volumetric erosion remain open. |
| Marching Cubes extraction | Not implemented | Evaluate topology ambiguity, normals, seams, collision and LOD transitions. |

- [ ] Research focused Rust libraries and published algorithms before selecting
  extraction, noise or erosion implementations; record sources, licences, maintenance,
  data ownership and reproducible output. No library or method is selected here.
- [ ] Compare a small fixed workload for heightfields versus density terrain:
  plain, mountain ridge, overhang, cavern and designed arch. Record preparation time,
  surface quality, peak memory and collider cost in optimized builds.
- [ ] Write a separate design brief for density storage, typed SlotMap ownership,
  durable serialization, coordinate lattice, material assignment, editing, collider
  preparation and asynchronous lifecycle. Reuse engine stores/handles.
- [ ] Prototype one bounded density chunk headlessly, with deterministic fixtures
  and tests for empty/full cells, ambiguous cases, winding and finite normals.
- [ ] Prove adjacent chunk continuity and mixed-detail seams before integration.
  Heightfield edge-stitching cannot be assumed to solve volumetric LOD transitions.
- [ ] Add generator selection only for methods with implemented/validated contracts.
  Keep legacy heightfield assets loadable and reject unsupported combinations.

Exit: a separately reviewed volumetric design and measured prototype justify an
implementation plan. Supporting every technique is a research backlog, not a
promise that each belongs in the first continuous-world release.

## Validation log

### 2026-10-30 consolidation check

Full Rust workspace (`cargo test --workspace --all-features`): engine 139 tests pass
(100 unit, 25 authored-scene, 9 physics, 3 units, 2 ECS mapping). IDE `pnpm test`: 128
pass. The first-project bounce-height test still fails (see [plan.md](../../plan.md));
it is unrelated to terrain.

### Audit validation — 2026-10-30

Executed from `src/emergent-ide`:

```sh
pnpm exec tsx --conditions=react-server --test tests/terrain.test.ts tests/terrain-assets.test.ts tests/terrain-brush.test.ts tests/terrain-library.test.ts tests/terrain-paint.test.ts
```

Result: **17 passed, 0 failed**. This verifies settings/protocol/mesh invariants,
brush math, saved-field/material roundtrips, stale-write rejection and library
placement preparation. These tests do not execute the full browser/native workflow.

Existing evidence reports browser library/save/reopen and high-resolution native
checks; see the linked records above. No browser, Rust suite or new performance
benchmark was rerun for the initial documentation audit. The subsequent native
browser check closed the 1024 inspector reproduction (see evidence above).
All continuous-world exit gates remain open.
