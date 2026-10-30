# Foundation iterations 2–11 (2026-10-22 → 2026-10-23)

Archived record of the original numbered iterations, consolidated from the former
root `ITERATION_2.md` … `ITERATION_11.md` files on 2026-10-30. Content is unchanged;
paths in backticks are relative to the repository root. Current status lives in
[implementation-status.md](../implementation-status.md).

---

## Iteration 2 — Transform Graph Foundation

Status: complete and validated on 2026-10-22. Benchmark evidence is recorded in `docs/benchmarks/iteration-2-transform-propagation.md`.

### Outcome

Turn the current entity tree into a useful spatial graph. An entity keeps an authored local transform, while the world computes and caches its global transform from its ancestors. The result remains headless and becomes the stable input needed by a later renderer, behaviour system, and project-mode inspector.

This is the next increment because every visible scene depends on transform semantics. It also exercises the SlotMap hierarchy under realistic traversal without committing Emergent to `hecs`, `bevy_ecs`, or a rendering architecture yet.

### Decisions to record first

Add a short architecture decision record covering:

- right- or left-handed coordinates;
- the up and forward axes;
- world units;
- rotation representation and composition order;
- local transform composition order;
- matrix layout at public API boundaries;
- how engine coordinates will map to `wgpu` clip-space conventions later.

Choose a focused SIMD-friendly Rust math crate after checking its native support and benchmark characteristics. Prefer using its vector, quaternion, and affine/matrix types internally over creating a broad custom mathematics library. Pin the selected crate version in `Cargo.lock`.

### Scope

#### Transform data

- Expand `Transform` from translation-only data to local translation, rotation, and scale.
- Keep local transform data separate from the derived global transform cache.
- Give newly spawned entities identity transforms.
- Reject or explicitly handle transform values that cannot be represented safely. Define the policy for zero scale and non-finite values before exposing setters.

#### Hierarchy propagation

- Add an explicit `World::propagate_transforms()` phase.
- Traverse roots before descendants without recursive calls, so deeply nested user scenes cannot exhaust the call stack.
- Compose every child's global transform from its parent's current global transform and its own local transform.
- Mark the affected subtree dirty when a local transform changes or an entity is reparented. Skip clean subtrees when doing so is demonstrably simpler or faster for the current layout.
- Preserve the current default reparenting rule: the entity keeps its local transform, so its global transform may change under its new parent.
- Keep hierarchy mutations coherent before propagation begins. A failed mutation leaves hierarchy links and cached transforms unchanged.

#### Read and mutation APIs

- Replace unrestricted mutable access to transform fields with world methods or a mutation guard that reliably marks the entity and descendants dirty.
- Provide explicit local-transform and global-transform reads.
- Keep `EntityId` world-scoped and generational. Derived transform storage must use the same authoritative identity mapping and must not introduce another ownership graph.
- Avoid allocating during steady-state propagation after world capacity has settled.

#### Reference example

Extend the small executable to build a three-level hierarchy:

```text
Scene
└── Carrier
    └── Scout
```

Move or rotate the carrier during fixed ticks and report the scout's derived world position. The example should make transform inheritance visible in console output without adding a window or renderer.

### Validation

- Test translation, rotation, scale, and combined parent/child composition using numerical tolerances appropriate for floating-point values.
- Test a multi-level hierarchy, multiple roots, reparenting, detaching to a root, subtree deletion, and propagation after stale-handle errors.
- Test that a local mutation invalidates all descendants and does not invalidate unrelated branches.
- Test a hierarchy deep enough to prove propagation and deletion do not rely on call-stack recursion.
- Verify clean propagation performs no entity changes and produces identical results.
- Run `cargo fmt --check`, `cargo test`, and Clippy with warnings denied.
- Add a small release-mode propagation benchmark for representative deep and wide hierarchies. Record the workload, hardware, toolchain, and results as a baseline; do not describe it as a performance target.

### Acceptance gate

Iteration 2 is complete when:

- coordinate and transform conventions are documented;
- global transforms remain correct after spawn, mutation, reparent, and deletion;
- deep hierarchy operations are iterative and tested;
- the reference game demonstrates inherited movement for 120 fixed ticks;
- the propagation benchmark and validation commands are recorded; and
- no ECS, renderer, window, asset, or project-mode dependency has been introduced.

### Deferred from this iteration

- `hecs` versus `bevy_ecs` selection;
- behaviour instances and behaviour links;
- `wgpu`, `winit`, cameras, and rendering;
- scene serialization and durable authored IDs;
- game project folders and build outputs;
- visual project mode and editing tools;
- preserve-world reparenting, shear decomposition, and animation.

These remain in `GOALS.MD`. The likely following iteration is a narrow behaviour-graph prototype or the ECS comparison spike, using this transform workload as one of its representative cases.

---

## Iteration 3 — ECS comparison spike

Status: complete and validated on 2026-10-22. The recommendation and evidence are recorded in `docs/research/iteration-3-ecs-comparison.md` and ADR 0002.

Version boundary: `0.3.0`, tagged `v0.3.0-ecs-direction`.

### Question

Should Emergent use `hecs`, standalone `bevy_ecs`, or focused SlotMap-owned component storage for component queries and system execution?

SlotMap remains authoritative for Emergent entity identity, hierarchy, scenes, and behaviour identities throughout this spike. ECS-native entity IDs are internal storage handles mapped from an Emergent-owned typed key.

### Candidates

- `hecs` 0.11.2 with its default `std` feature: a small archetype ECS with direct query and random-access APIs, but no built-in schedule.
- `bevy_ecs` 0.20.0 with default features disabled and only `std` enabled: an archetype ECS with queries, change detection, resources, commands, and a system scheduler.
- `slotmap` 1.1.1 plus `SecondaryMap`: the smallest baseline with direct control and no archetype query or scheduler implementation.

The spike dependencies remain under `[dev-dependencies]`; completing the comparison does not place either ECS in the engine's production dependency graph.

### Workloads

Use the same position/velocity component and deterministic update for every candidate:

1. Native dense update over 100,000 entities.
2. Random access through 100,000 Emergent-owned SlotMap handles and, for ECS candidates, a `SecondaryMap` to ECS-native IDs.
3. Repeated spawn/despawn churn including canonical identity and mapping maintenance.
4. Bevy scheduled-system execution, measured separately from direct query iteration.

Run optimized builds and record checksums so each implementation is shown to perform equivalent work. Record process memory separately where practical. These are local microbenchmarks, not universal ECS rankings.

### Qualitative review

Compare:

- query and mutation ergonomics;
- explicit scheduling and deterministic ordering;
- change detection and deferred commands;
- ability to keep one authoritative hierarchy;
- stale-handle and mapping cleanup semantics;
- dependency count, feature selection, minimum Rust version, and build cost;
- suitability for AI-generated game systems and machine-readable registrations;
- escape hatches for specialized packed or renderer-owned data.

### Acceptance gate

The spike is complete when:

- all three equivalent workloads compile and run in release mode;
- dependency and benchmark evidence is recorded with its limitations;
- a small Bevy schedule proves standalone system execution;
- mapping creation and deletion rules are documented;
- the recommendation identifies a production direction and explicitly rejects or defers the alternatives; and
- neither `hecs` nor `bevy_ecs` has become a production dependency merely because it was evaluated.

### Out of scope

- migrating the current `World` implementation;
- implementing behaviour links;
- parallel schedule benchmarking;
- serialization and project schemas;
- rendering or `wgpu` extraction;
- a permanent abstraction supporting multiple interchangeable ECS backends.

Any production adoption follows in a separate iteration with a focused design and migration plan.

---

## Iteration 4 — Production ECS integration

Status: complete and validated on 2026-10-22. Runtime measurements are recorded in `docs/benchmarks/iteration-4-runtime-integration.md` and the lifecycle contract is recorded in ADR 0003.

Version boundary: `0.4.0`, tagged `v0.4.0-ecs-integration`.

### Outcome

Integrate standalone `bevy_ecs` as Emergent's production component and schedule layer without weakening the existing SlotMap identity and hierarchy contracts. Every canonical entity receives exactly one internal Bevy entity. Game systems operate on Bevy components in explicit fixed-tick phases and request structural changes through an Emergent-owned lifecycle queue.

### Ownership and identity

- `World` continues to own the SlotMap entity store, hierarchy, local transforms, global-transform cache, and canonical `EntityId` values.
- `World` also owns a private Bevy component world and a `SecondaryMap<EntityKey, bevy_ecs::Entity>`.
- Each internal Bevy entity carries a read-only-by-construction `EntityIdentity` component containing its canonical `EntityId`.
- Bevy entity IDs never appear in Emergent's public identity or hierarchy APIs.
- Spawn commits the SlotMap entity, Bevy entity, identity component, and mapping as one world operation. Subtree removal validates every mapping first, then removes Bevy entities and mappings before releasing SlotMap keys.
- Public component access validates the canonical mapping and reports missing or corrupt mappings explicitly.

### Data flow and schedules

The fixed tick has three Bevy schedule phases:

1. `PreFixed`: prepare component state for the tick.
2. The existing game `fixed_update` callback, followed by `Fixed`: execute normal simulation systems.
3. Commit queued Emergent entity lifecycle commands, propagate the SlotMap transform graph, then run `PostFixed` for derived component work and observation.

The existing `after_fixed_update` callback runs after `PostFixed`. A `SimulationTime` resource exposes the current tick and fixed delta to systems.

Scheduled systems cannot directly access the private SlotMap store. They request supported structural changes through `EntityLifecycleQueue`. Commands execute in insertion order at the single commit point. Results are published through `EntityLifecycleResults` for the `PostFixed` phase and game callback. Newly spawned entities cannot be referenced by another command in the same batch in this first version.

### Hot paths and expected scale

- Dense component work stays in native Bevy queries and does not perform SlotMap-to-Bevy lookups per entity.
- Direct `World` component reads and mutations cross the mapping once and are intended for graph, tooling, setup, and other boundary work.
- Transform traversal remains the existing iterative SlotMap traversal.
- Mapping validation on ordinary boundary access is constant-time. Schedule boundaries use a cached archetype query plus an `EntityIdentity` removal hook to reject direct Bevy structural changes without scanning every canonical entity. A full validation API checks every canonical mapping for tests and diagnostics.
- Lifecycle command buffers are reused between ticks. The first implementation may allocate when a new peak queue size is reached.

### Validation

- Prove one-to-one identity mapping after root and child creation.
- Prove arbitrary user components can be inserted, queried by scheduled systems, mutated, removed, and cleaned up with a subtree.
- Prove SlotMap generation reuse never aliases an old Bevy mapping.
- Prove failed hierarchy commands publish explicit results and leave both stores coherent.
- Prove schedule phase order, fixed-time resource values, and the lifecycle commit point.
- Detect unsupported direct Bevy structural changes at a schedule boundary.
- Run formatting, debug and release tests, Clippy with warnings denied, rustdoc, and the existing release benchmarks.

### Deferred

- Mirroring transforms into ECS components or renderer extraction data.
- Pending handles that allow commands in one batch to refer to entities spawned earlier in that batch.
- Parallel schedule execution and multithreaded Bevy features.
- Behaviour instances and typed behaviour links.
- Serialization, durable authored identity, graphics, assets, and project mode.

### Acceptance gate

Iteration 4 is complete when `bevy_ecs` is a normal dependency, the runtime uses the mapping and schedules described above, mapping and command edge cases are tested, production dependencies contain no `hecs`, and the reference game still completes its 120-tick hierarchy demonstration.

---

## Iteration 5 — Behaviour graph foundation

Status: complete.

### Outcome

Add the first behaviour graph that makes Emergent entities influence one another through typed links. Behaviour identity and link ownership remain SlotMap-based. Behaviour evaluation runs once per fixed tick, reads the previous committed output snapshot, stages new outputs and transform writes, then commits them at a defined boundary.

The reference demo becomes a leader and follower chain driven by behaviours rather than manual transform mutation in `Game::fixed_update`.

### Ownership and storage

- `World` owns one `BehaviorGraph` alongside its entity SlotMap and Bevy component world.
- Each behaviour has a world-scoped typed `BehaviorId`, one owning `EntityId`, one built-in behaviour kind, and one committed output in this initial slice.
- The graph keeps stable insertion order for diagnostics and deterministic iteration. Tick semantics do not depend on that order because every link reads the previous output snapshot.
- Entity subtree deletion removes behaviours owned by deleted entities. Links from surviving behaviours become explicitly unavailable; stale behaviour generations never resolve to replacements.
- An entity may own multiple behaviours, but at most one enabled behaviour may write its transform during this iteration.

### Typed ports and links

The initial value types are `f32`, `Vec3`, `bool`, and `EntityId`. Ports use static identifiers and declare a value type. Connecting a link validates source and target worlds, behaviour existence, port existence, direction, and type compatibility before changing the graph.

Initial built-in behaviours:

- `LinearMotion`: no input; advances a position by velocity and publishes `Vec3` on `position`.
- `FollowPosition`: consumes `Vec3` on `target`, moves toward it by a bounded speed, and publishes `Vec3` on `position`.
- `AddScalar`: consumes `f32` on `value`, adds a configured increment, and publishes `f32` on `value`. It provides the bounded feedback-cycle proof.

Each behaviour has an explicit unavailable-input policy. `FollowPosition` suspends and retains its last output. `AddScalar` uses its configured default input. Disabled source behaviours are unavailable to consumers.

### Fixed-tick data flow

1. Run `PreFixed`, the compatibility callback, and `Fixed` systems.
2. Evaluate each enabled behaviour from committed inputs.
3. Commit all behaviour outputs simultaneously.
4. Apply staged behaviour transform writes through the existing `World` transform API.
5. Commit queued entity lifecycle commands. Deletion removes owned behaviours.
6. Propagate the SlotMap transform hierarchy.
7. Run `PostFixed` and `after_fixed_update`.

No behaviour recursively evaluates another behaviour. A feedback link therefore advances once per tick with a one-tick delay.

### Hot paths and expected scale

- Evaluation is iterative and uses a retained order vector and retained transform-write buffer.
- `BehaviorValue` is a small copyable enum; the initial tick path performs no boxing or formatting.
- Links resolve by typed generational key directly into SlotMap storage.
- Transform writers stage one write each; application reuses the graph buffer and existing world dirty-traversal storage.
- The initial enum of built-in kinds is deliberately closed. A later registration design must preserve batched execution and AI-readable schemas before project-defined behaviour types are admitted.

### Validation

- Validate typed connection, disconnection, and rewiring.
- Validate leader/follower motion and simultaneous snapshot commits.
- Validate a self-linked scalar feedback cycle advances exactly once per tick.
- Validate disabled and deleted sources produce the configured unavailable-input result.
- Validate subtree deletion removes owned behaviours and stale IDs never alias reused slots.
- Validate conflicting transform writers are rejected.
- Validate equivalent headless runs produce equivalent outputs.
- Run debug and release tests, Clippy, rustdoc, the reference demo, and a release behaviour benchmark.

### Deferred

- Project-defined behaviour registration and schema export.
- Multiple inputs or outputs on a single built-in behaviour.
- Pending entity or behaviour handles inside one command batch.
- Parallel behaviour evaluation, graph compilation, and same-tick dependency ordering.
- Behaviour serialization, project assets, visual inspection, and scripting.
- Renderer extraction and `wgpu`.

### Acceptance gate

Iteration 5 is complete when the reference demo is behaviour-driven, link and lifecycle edge cases are tested, feedback cannot recurse within a tick, the tick path retains its work buffers, and an optimized representative benchmark is recorded with its limitations.

---

## Iteration 6 — First visible 3D object

Status: complete.

### Outcome

Open a native `winit` window and render one depth-tested cube through `wgpu`. The cube's model matrix is copied from an authoritative Emergent entity global transform after fixed simulation updates. Headless builds and tests remain available without compiling or initializing graphics.

### Ownership and boundaries

- `Engine` continues to own simulation time, schedules, the SlotMap world, and game callbacks.
- A graphics-gated renderer owns the `wgpu` instance, surface, adapter-derived device and queue, surface configuration, render pipeline, depth texture, and fixed cube buffers.
- `winit` owns the platform event loop and window. The visible example coordinates events, fixed simulation advancement, extraction, and rendering.
- Render data contains copied matrices and viewport state. It never holds entity references, Bevy entities, or long-lived borrows into `World`.
- SlotMap `EntityId` remains the object's canonical runtime identity. GPU buffer offsets are transient renderer details.

### Data flow

1. `winit` creates the window after `Resumed`.
2. The renderer selects a surface-compatible adapter and configures the surface.
3. The example advances Emergent's fixed simulation from elapsed wall time with bounded catch-up.
4. The example reads the cube entity's propagated global transform and copies its model matrix into a frame uniform.
5. The renderer writes that small uniform, encodes one indexed draw with depth testing, and presents.
6. Resize recreates the depth target and reconfigures only non-zero surface dimensions. Close requests stop the game and exit cleanly.

### Performance shape

- Cube vertex and index buffers and pipeline state are created once.
- Each frame writes one fixed-size uniform and creates only the command encoder required by `wgpu`.
- Simulation retains its fixed timestep and bounded catch-up. Rendering requests redraws independently and does not change simulation tick semantics.
- This is a smoke workload. No performance target is claimed from a single cube.

### Validation

- Keep existing headless tests passing with default features.
- Compile and lint all targets with the graphics feature.
- Test projection/model matrix creation without a GPU where useful.
- Run the example on macOS, observe the cube, resize the window, and close it cleanly.
- Record the adapter, backend, surface format, and local runtime result. Windows and Linux remain documented but unverified until run on those systems.

### Deferred

- General mesh/material assets, texture loading, lighting, cameras as entities, visibility, culling, batching, and render graphs.
- Multiple objects and incremental extraction buffers.
- Input mapping and project assets.
- Project mode, diagnostics overlays, and renderer profiling.

### Acceptance gate

Iteration 6 is complete when the opt-in example presents a depth-tested cube whose transform comes from the Emergent world, handles resize and surface recovery, exits cleanly, and leaves the default headless validation path intact.

---

## Iteration 7 — First independent game project

Status: complete.

### Outcome

Establish the first honest boundary between the Emergent engine crate and a game project. The game is a separate Cargo package that depends only on Emergent's public API. It owns its manifest, authored and generated Rust, asset folders, output declarations, and runtime assembly.

This iteration validates and runs the project. It does not yet interpret scene or entity asset contents, decode media, package outputs, or provide a general type registry.

### Project layout

```text
projects/first_project/
  Cargo.toml
  emergent.toml
  AGENTS.md
  src/
    main.rs
    game.rs
    generated/
  assets/
    scenes/
    entities/
    data/
    textures/
    models/
    audio/
  tests/
```

`src/generated` is ordinary reviewable Rust and may be produced by an AI coding generator. `AGENTS.md` tells generators to use public Emergent handles and APIs and to avoid engine source edits. Authored asset paths are project-relative. Derived caches and build artifacts do not live under `assets`.

### Manifest contract

The root `emergent.toml` declares:

- schema version, stable project ID, and display name;
- an entry scene path and generated-code directory;
- one or more asset roots;
- named output nodes with target OS, architecture, Rust target triple, and development or release profile.

The engine loads and validates this startup contract. Paths must be relative, portable, free of `..`, and resolve within the project root. Declared roots, the entry scene, and generated-code directory must exist. Output IDs must be unique and at least one output must be enabled.

The manifest uses authored strings and paths rather than runtime SlotMap or ECS keys. Loading it creates no world entities and cannot partially activate a scene.

### Runtime boundary

- The project binary locates its root with `CARGO_MANIFEST_DIR` and calls `Project::load` before creating an engine.
- Project-owned code constructs its first runtime scene through public `World`, behaviour, component, and schedule APIs.
- Generated code supplies a scene builder in this proof. It returns canonical runtime handles to the project-owned game module.
- The project may inspect resolved asset paths, but media loading and authored scene expansion are deferred.

### Performance shape

Manifest parsing and filesystem validation occur once during startup and never enter fixed-tick work. Asset data is not scanned recursively. The runtime engine and SlotMap ownership model are unchanged.

### Validation

- Unit-test valid loading, unsupported schema, traversal/absolute paths, missing paths, duplicate outputs, and no enabled output.
- Build and test the independent project as a workspace member.
- Run the project headlessly and confirm its generated scene advances through public behaviour APIs.
- Keep engine formatting, tests, Clippy, and rustdoc passing.

### Deferred

- Scene/entity schema parsing, durable authored node IDs, composition expansion, registration, and graph export.
- Asset import, decoding, dependency discovery, hot reload, and derived caches.
- Output build planning, packaging, cross-compilation, and native verification.
- Project templates and creation commands beyond this checked-in reference project.

### Acceptance gate

Iteration 7 is complete when the separate project validates its manifest, uses generated and authored project files without engine source edits, builds against public Emergent APIs, runs a small headless scene, and fails malformed project contracts with explicit errors.

---

## Iteration 8 — First-class geometry

Status: complete.

### Outcome

Model and render the first four geometry families: a point, a line, a polygon, and an indexed mesh. CPU-side geometry remains available to headless games and tools. The optional `graphics` feature supplies a native `wgpu` renderer and a visible reference scene containing all four forms.

This iteration establishes geometry ownership and validation. It does not add asset import, materials, textures, lighting, arbitrary concave polygon triangulation, editing, visibility culling, or render batching across materials.

### Ownership and SlotMap integration

- `Point`, `Line`, `Polygon`, `MeshVertex`, `Mesh`, `Color`, and `Geometry` are owned CPU values. They contain no GPU objects or entity identity.
- `GeometryRenderer` owns uploaded vertex and index buffers in a `SlotMap<GeometryId, GpuMesh>`. `GeometryId` is a typed, generational, renderer-scoped runtime handle.
- Removing uploaded geometry invalidates its handle. Rendering a stale or foreign handle returns an explicit error and never aliases a replacement resource.
- Entities remain canonically identified by `EntityId`. A game extracts copied `GeometryInstance` values containing a renderer geometry handle and model matrix; neither storage owns the other.
- Geometry handles are runtime-only. Project files will use durable asset IDs when authored mesh loading arrives.

### Data and validation contracts

- A point has a finite position, positive finite radius, and finite colour. It tessellates to an octahedral marker so it remains visible across native backends.
- A line has distinct finite endpoints, positive finite radius, and finite colour. It tessellates to a square prism rather than relying on implementation-defined line width.
- A polygon has at least three ordered vertices and one colour. The first implementation accepts only finite, planar, non-degenerate, convex polygons and triangulates them as a fan. Unsupported concave or self-intersecting inputs fail instead of rendering incorrectly.
- A mesh contains finite coloured vertices and `u32` triangle indices. Index count must be a non-zero multiple of three; every index must be in range; every triangle must have non-zero area.

### Rendering and hot paths

1. Upload validates and tessellates geometry once, then creates immutable GPU vertex and index buffers.
2. Each frame receives a borrowed slice of copied `GeometryInstance` values.
3. The renderer validates every geometry handle before acquiring the surface frame.
4. Model-view-projection values are written into one reusable CPU staging vector with device-aligned dynamic-uniform strides, then copied with one queue write.
5. One depth-tested colour pipeline draws every instance. GPU geometry buffers stay resident until removed.

The uniform buffer begins with bounded spare capacity and grows geometrically. Steady-state rendering at or below the current high-water mark does not allocate CPU collections or create per-object buffers. Draw-call batching, packed geometry arenas, frustum culling, and dirty-only uniform uploads require representative workloads and measurement before adoption.

### Expected scale

This is a correctness and boundary milestone for small development scenes. The reference workload is four geometry resources and four instances. No production object-count, draw-call, frame-time, or memory target is claimed yet. Later extraction benchmarks must establish those budgets before the public renderer is described as scalable.

### Validation

- Unit-test invalid point/line values, polygon degeneracy/planarity/convexity, mesh indices, degenerate triangles, and successful constructors.
- Unit-test tessellation counts and ensure generated mesh indices remain valid.
- Test SlotMap removal and stale-handle rejection without requiring a GPU where practical.
- Keep formatting, default and all-feature tests, Clippy, and rustdoc clean.
- Run the visible reference scene in smoke mode on the available native platform, recording adapter, backend, resize, presentation, and shutdown.

### Acceptance gate

Iteration 8 is complete when the public CPU types model all four requested geometry forms, invalid input fails explicitly, the renderer displays one instance of each from renderer-owned SlotMap storage, stale geometry handles fail safely, the native smoke run succeeds, and headless builds remain independent of `wgpu` and `winit`.

---

## Iteration 9 — Procedural primitive library

Status: complete.

### Outcome

Add a validated procedural primitive layer that produces the existing indexed `Mesh` contract for planes, cuboids/cubes, pyramids, UV spheres, cylinders, cones, capsules, and toruses. The same generated meshes upload through `GeometryRenderer`; no second rendering or identity path is introduced.

### Ownership and SlotMap integration

- `Primitive` is an owned, headless CPU value with validated construction parameters.
- `Primitive::to_mesh` deterministically produces an owned `Mesh`. Generation allocates only for the result and occurs before GPU upload, never in the render loop.
- `Geometry::Primitive` lets the renderer tessellate and upload a primitive once. The resulting GPU buffers remain owned by `GeometryRenderer` under renderer-scoped `GeometryId` handles.
- Entities continue to own only canonical `EntityId` identity and copied transforms. Primitive parameters and renderer handles do not become alternate entity identities.

### Data and generation contracts

- All sizes and radii must be finite and positive. Half-extents are explicit where a shape is centred on the origin.
- Segment and ring counts have documented lower bounds and a bounded upper limit. Invalid or excessive tessellation requests fail before allocating output buffers.
- Plane and cuboid dimensions, pyramid base/height, sphere radius/stacks/sectors, cylinder and cone dimensions/segments, capsule radius/half-segment/rings/segments, and torus major/minor radii/segments are explicit.
- A torus requires its major radius to exceed its minor radius. A capsule requires a positive straight half-segment; a sphere represents the zero-segment case.
- Generated triangles pass the same finite-position, index-range, and non-degeneracy validation as authored meshes.

### Hot paths and expected scale

Primitive generation is setup/loading work. It does not execute during fixed ticks or frame rendering unless a game explicitly requests regeneration. The initial gallery creates eight resources once and reuses their GPU buffers and the renderer's existing uniform staging allocation.

The segment limit prevents accidental unbounded setup work from authored or generated parameters. No claim is made that maximum-detail shapes are suitable for real-time regeneration. Caching equivalent primitive parameter sets is deferred until workload evidence justifies the lookup and memory costs.

### Validation

- Verify every primitive generates a valid non-empty indexed mesh with the expected broad topology.
- Reject non-finite/zero dimensions, insufficient or excessive subdivisions, invalid capsule lengths, and invalid torus radii.
- Keep default headless builds independent of graphics dependencies.
- Run formatting, default and all-feature tests, Clippy, and rustdoc.
- Run an eight-shape native gallery through the available GPU backend, including resize and clean shutdown.

### Acceptance gate

Iteration 9 is complete when all eight primitive families can be created and converted to valid meshes through public headless APIs, rendered through existing SlotMap geometry storage, exercised together in the native gallery, and documented with planned advanced mesh attributes still clearly deferred.

---

## Iteration 10 — Light-source entity foundation

Status: complete.

### Outcome

Add a validated, headless `LightSource` component for canonical Emergent entities and prove its use from the independent first project. The component models ambient, directional, point, and spot lights with colour, intensity, bounded influence where applicable, spot cone angles, and an explicit shadow-casting request.

This iteration establishes scene and project vocabulary. It does not claim that the current unlit colour renderer evaluates lights or renders shadow maps.

### Ownership and SlotMap integration

- `World` retains canonical entity identity and hierarchy in its SlotMap.
- `LightSource` is dense component data stored on the entity's mapped `bevy_ecs` component entity. It does not introduce a light identity or ownership graph.
- Position comes from the entity's propagated transform. Directional and spot lights use the entity's local negative Z axis after global transform propagation. Ambient lights ignore transform; point lights ignore rotation.
- Runtime `EntityId` values are never persisted. Authored projects identify a light with a durable scene/entity ID and resolve it to a fresh entity during construction.

### Data and validation

- Colour uses the existing validated `Color` value.
- Intensity is a finite, non-negative engine scalar. Physical photometric units are deferred until material and exposure contracts exist.
- Point and spot ranges are finite and positive.
- Spot inner and outer half-angles are finite, non-negative, ordered, and no wider than 90 degrees.
- Shadow casting is an explicit disabled/enabled request. Ambient lights cannot request shadows. Renderer support and quality settings remain deferred.
- Private fields and named constructors prevent unchecked combinations.

### Data flow and hot paths

Projects construct a canonical entity, set its transform, and insert a `LightSource` through `World::insert_component`. Construction and validation occur outside the fixed-update hot path. Future render extraction will query light components and copy bounded frame data without changing canonical identity.

This iteration adds no per-frame light traversal, GPU buffer, shader branch, or shadow work, so it makes no rendering-performance claim.

### Expected scale

The component representation is suitable for ordinary ECS storage. A future renderer must define visible-light limits, selection/culling, clustered or tiled evaluation, and shadow budgets from measured workloads before large light counts are claimed.

### Validation

- Accept each supported light kind and expose its validated properties.
- Reject non-finite/negative intensity, invalid range, invalid spot cones, and ambient shadow requests.
- Verify a light component follows canonical entity lifecycle and becomes unavailable with its entity.
- Add a point-light entity to `projects/first_project` using only public Emergent APIs.
- Keep default builds headless and run formatting, workspace tests, optimized all-feature tests, Clippy, and rustdoc.

### Acceptance gate

Iteration 10 is complete when the public headless API can represent validated light-source entities, the independent project constructs and verifies one, documentation distinguishes the available component contract from planned illumination and shadows, and the full repository validation suite passes.

---

## Iteration 11 — Lit geometry and first directional shadows

Status: complete. The 2026-10-23 Metal smoke created the lit pipeline, submitted the directional shadow path, presented 239 frames, observed resize, and shut down cleanly; see `docs/platform/iteration-11-macos-smoke.md`. Visible image quality and frame timing remain unmeasured.

### Outcome

Extend the optional `wgpu` geometry renderer with normals, copied light inputs, bounded GPU light data, diffuse lighting, and one directional shadow map. Prove the path in a native lighting gallery built entirely from canonical entities, `LightSource` components, and renderer-owned geometry handles.

### Ownership and SlotMap integration

- `MeshVertex` remains an owned CPU mesh value. An authored normal is optional; missing normals are generated during renderer tessellation from indexed triangle topology.
- `GeometryRenderer` continues to own vertex/index buffers, the light uniform buffer, depth targets, and the shadow texture. `GeometryId` remains the only renderer resource handle and stays renderer-scoped and generational.
- Light identity remains the canonical `EntityId` plus its `LightSource` component. `LightInstance` is copied render input derived from a component and a propagated transform; it is neither durable identity nor a second light store.
- The renderer accepts no more than eight light inputs per frame and selects at most one enabled directional light for shadow mapping. The selection is deterministic in supplied-input order.

### Data flow

1. A game constructs entities and attaches `LightSource` components through public `World` APIs.
2. After transform propagation, the game copies required world transforms and light components into `GeometryInstance` and `LightInstance` render inputs.
3. The renderer validates input matrices, writes one bounded light uniform buffer and reusable per-object dynamic uniforms, then runs an optional directional depth pass followed by the lit colour pass.
4. Fragment shading evaluates a simple Lambert response. Ambient lights add colour directly; directional, point, and spot lights use transformed normals. The one selected directional shadow light samples its depth map with comparison sampling.

### Hot paths and limits

- The per-frame light upload uses a fixed-size eight-light uniform payload; it does not allocate in steady state.
- Geometry upload and missing-normal generation occur during `insert`, not per frame.
- Per-instance dynamic uniforms and render loops reuse existing renderer staging memory.
- The first shadow map is a fixed 2048-square depth texture. Its pass draws every submitted shadow caster; no visibility culling, atlas, cascade, point-light cube map, or resolution scaling is claimed.
- Light limits and shadow-map resolution are deliberate first bounds, not measured production targets.

### Contracts

- `MeshVertex::with_normal` accepts a finite, non-zero normal and stores a normalized value. `MeshVertex::new` remains available for meshes whose normals are generated during renderer upload.
- `GeometryRenderer::insert_with_shading` accepts `ShadingMode::Smooth` (the default) or `ShadingMode::Flat`. Flat mode expands indexed triangles during upload and assigns each face one normal; it deliberately trades GPU vertex memory for clear face boundaries and a simpler fragment path.
- `GeometryRenderer::render` preserves the existing unlit-compatible call surface. `render_lit` accepts explicit copied `LightInstance` values.
- A shadow-casting request only becomes a shadow when it belongs to a directional light selected for the bounded shadow pass. Point and spot shadow requests remain data for future work.
- Non-uniform model scaling is supported by an inverse-transpose normal matrix stored in the per-object uniform.

### Validation

- Validate authored normals and missing-normal generation, including finite unit-length GPU normals.
- Validate light-input bounds, transform validation, deterministic directional-shadow selection, and WebGPU depth conventions mathematically.
- Retain geometry and primitive tests, then run default and all-feature release suites, Clippy, rustdoc, docs validation, and the independent first project.
- Run the native lighting gallery with resize and clean shutdown on the available backend. This proves resource creation, light upload, shadow pass submission, presentation, and shutdown; it is not a frame-time, quality, or Windows/Linux result.

### Acceptance gate

Iteration 11 is complete when the renderer uploads usable normals and bounded light data, a lit native gallery visibly submits a directional shadow pass over canonical entity geometry, the public API documents exact light/shadow limits, and validation evidence is recorded without overstating the incomplete rendering stack.
