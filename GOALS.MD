# Emergent — Foundation Goals

Last reconciled: **2026-10-30**. This file holds the vision, architecture contracts and
stage gates. It is not a changelog. Where things stand:

- [plan.md](plan.md): what is being worked on now and next
- [docs/implementation-status.md](docs/implementation-status.md): delivered slices with design/evidence links
- [docs/README.md](docs/README.md): documentation map and update rules

`[x]` means the stated slice is implemented; it does not imply that the enclosing
stage's exit gate has passed. Windows/Linux runtime validation, authored
compositions/behaviour links, play/pause/step controls and production performance
acceptance remain open across stages.

## Vision

Build Emergent as a modular Rust game engine in which richer entities are normally constructed by composing simpler sub-entities. Each entity can carry behaviour, and behaviours can reference and influence other behaviours to produce broader emergent behaviour.

Start with the foundations: mathematical values and points, transforms, entities and objects, hierarchical composition, scenes, behaviours, and a game loop. Prove these together in a small working example before expanding into joystick input, richer 3D objects, sound, and other game systems.

Gaming-grade speed, predictable frame times, and efficient memory use are architectural requirements. Follow `AGENTS.md`; SlotMap-based identity and hierarchies remain core to Emergent.

There are two equally important parts to the product:

1. **The engine framework:** the runtime entity system, graphs, behaviours, modules, simulation, and rendering foundations.
2. **The game development framework:** an independent project and asset-folder workflow through which developers build games, author scenes, and extend entities and behaviours on top of Emergent.

AI coding generators are first-class users of the development framework. They must be able to understand the underlying graphs, generate ordinary project files and Rust code, validate their changes, and run the resulting game. Success requires proving this workflow from a separate game project, not only from examples embedded inside engine internals.

## Foundational model

### Modules

A module is a cohesive engine capability with explicit dependencies, owned resources, and lifecycle hooks. Begin with ordinary Rust modules and static registration; split into crates when dependency boundaries justify it. Runtime plugin loading and a general plugin ABI are deferred.

Modules register their data and scheduled work through a small application builder. Dependency validation happens at startup, with useful errors for missing dependencies or cycles. Initialization follows dependency order; shutdown reverses it. Avoid hidden global state and unrestricted service locators.

Proposed boundaries:

| Module | Responsibility |
| --- | --- |
| Math | Points, vectors, rotations, transforms, coordinate conventions |
| World | SlotMap identity, entity lifecycle, parent/child hierarchy, scene membership |
| Behaviour | Behaviour instances, typed links, tick inputs and outputs, execution rules |
| Runtime | Module lifecycle, clock, fixed simulation steps, command commit points |
| Platform | Window lifecycle and platform events |
| Render | Camera, minimal geometry, GPU resources, scene extraction and presentation |

Math and headless simulation must not require a window or graphics device. Future input, audio, physics, and asset modules should attach through these boundaries without rewriting the foundation.

### Values, entities, and objects

- A vector or point is normally a small value, not a separately allocated entity. Introduce an entity only when identity, lifecycle, hierarchy, or behaviour is needed.
- An entity has a typed generational identity and optional data and behaviours. An object is a useful composition of these capabilities, not a parallel identity system or inheritance tree.
- Build composite objects using ordinary entities and child entities. A composite can itself become a child of a larger composite.
- An entity has at most one parent; a scene can have multiple roots. Cross-references do not imply parenthood or ownership.
- A world owns entity, scene, and behaviour storage. SlotMap keys are scoped to their owning world; APIs must prevent or reject cross-world references.
- Scenes group entity roots for lifecycle and simulation participation. Start with one active scene, with explicit activation and teardown rules; streaming and simultaneous scene simulation are deferred.
- Define local and world transforms, parent propagation, and reparenting semantics. Choose and document handedness, units, axes, matrix conventions, and quaternion composition before renderer integration.
- Default reparenting preserves the local transform. A later preserve-world option must explicitly handle non-invertible transforms and transforms that cannot be represented without shear.
- Default entity destruction removes its owned subtree and behaviours. External links become unresolved safely; they never retain ownership or resolve to replacement objects.

### SlotMap and ECS responsibilities

SlotMap is authoritative for Emergent entity identity, parent/child relationships, scene membership, and behaviour identities. Keep these contracts stable while integrating the selected component and system layer.

Iteration 3 evaluated `hecs`, standalone `bevy_ecs`, and focused SlotMap storage:

| Candidate | Why evaluate it | Questions to resolve |
| --- | --- | --- |
| `hecs` | Minimal ECS with direct component queries from ordinary Rust code | How much scheduling, access control, and change tracking must Emergent supply? |
| `bevy_ecs` | ECS with systems, scheduling, and change detection | Do its capabilities justify dependency, integration, and execution costs for our workloads? |
| SlotMap plus focused component storage | A small comparison baseline using our existing identity model | Where does custom query and scheduling work outweigh its simplicity? |

ADR 0002 selects standalone `bevy_ecs` for general component storage, queries, resources, and scheduling. `hecs` is deferred, and SlotMap remains the focused identity, hierarchy, and specialized-storage foundation. Do not build a universal ECS abstraction.

This foundation decision is locked in for implementation:

- Use native Bevy queries and schedules for dense frame work.
- Use typed SlotMap keys for public identity, ownership graphs, parent/child relationships, scene membership, and behaviour identity.
- Map SlotMap keys to internal Bevy entities at explicit lifecycle commit points; canonical lookup is a boundary operation, not the normal dense-update path.
- Keep specialised packed or subsystem-owned storage available when profiling demonstrates a need.
- Do not continue routine comparison work or retain interchangeable ECS backends. Reopen ADR 0002 only if production integration cannot preserve the identity and hierarchy invariants, a representative engine workload shows an unacceptable frame-time or memory regression against an agreed budget, or a required target cannot support the selected dependency.

The release comparison on 2026-10-22 used 100,000 entities and produced the same checksum (`20624950.238`) for every candidate. The confirmation run measured Bevy scheduled dense updates at 63.82 us per iteration, Bevy direct queries at 78.51 us, SlotMap dense updates at 126.56 us, and `hecs` dense updates at 217.70 us. SlotMap remained strongest for canonical lookup (159.73 us) and spawn/despawn churn (6.70 ns per operation), supporting the split of responsibilities above. These are local microbenchmark results, not universal performance guarantees; the complete method, repeated-run medians, memory observations, and limitations are in `docs/research/iteration-3-ecs-comparison.md`.

During integration, Bevy entity IDs remain internal storage handles mapped to Emergent SlotMap keys. Define creation, deletion, rollback on failed creation, and mapping cleanup at explicit commit points. Keep hierarchy data authoritative in one place; do not maintain independent competing parent/child graphs in SlotMap and the ECS. Measure the indirection and memory cost of this arrangement. If it proves unsuitable, raise the architectural conflict before changing SlotMap's role.

Continue measuring release-build query/update throughput, hierarchy propagation, structural churn, behaviour-link lookup, allocation and memory costs as the real engine integration develops. Include mapping costs where they occur, pin dependency versions, and record the Rust toolchain with each performance result. These measurements validate and tune the chosen architecture; they do not keep the original candidate selection perpetually open.

### Behaviours and emergence

- Every entity can have zero or more behaviour instances. Behaviour data belongs to an entity; systems execute compatible behaviour data in batches where useful. A behaviour need not be a boxed object with its own virtual update method.
- Give each behaviour a typed SlotMap key and explicit input/output contracts. Links target behaviour instances and validated ports or capabilities, rather than raw pointers or implicit name lookups.
- Separate the ownership tree from the behaviour dependency graph. Links may connect siblings, ancestors, or unrelated entities within the active scene; cross-scene links are deferred.
- Begin with deterministic tick semantics: behaviours read the previous committed output snapshot and publish outputs for the next tick. Supply explicit initial output values. This makes feedback cycles well-defined with a one-tick delay and prevents recursive evaluation.
- Within a tick, each enabled behaviour runs at most once. Stage writes and structural commands, then commit them in a stable, documented order. Define conflicts explicitly; do not let incidental ECS iteration order decide multiple writes to the same property.
- Validate link types when connecting behaviours. A deleted, disabled, or unavailable target produces an explicit unavailable input; each input declares whether to use a default, retain a value, or suspend its consumer. Record diagnostics without flooding frame-time logs.
- Start with a simple deterministic schedule. Same-tick dependency evaluation, graph compilation, scripting, and parallel execution require later evidence and explicit semantics.

The first emergence example should compose several moving entities into a linked chain: a leader generates a target, followers consume other behaviours' committed positions, and child markers inherit their owners' transforms. Rewiring links changes the group motion without rewriting each follower. Include a bounded feedback example to prove cycles advance across ticks safely. This demonstrates composition and interaction; sophisticated autonomous behaviour is a later goal.

## Engine capability catalogue

Treat the major facilities expected of a game engine as first-class capability families, even before they have runtime implementations. The canonical, machine-readable catalogue is `docs/capabilities/engine-capabilities.toml`; the user guide presents the same roadmap by category. Every capability has a stable ID, owning module, delivery status, dependencies, and a concise contract. Status is one of `available`, `preview`, or `planned`; a planned entry must never be presented as a callable API.

The catalogue guides module boundaries, project manifests, asset folders, type registration, documentation, and AI coding generators. Capability IDs describe engine concepts and may later appear in registries or target requirements, but they are not Rust type names, serialized SlotMap keys, or a promise that the final public API will use the current wording. Update the catalogue when an implementation lands, and link its status change to validation evidence.

| Family | First-class scope |
| --- | --- |
| Render foundation | Windows and surfaces, adapters/devices, cameras, viewports, render extraction, render passes, visibility, culling, batching, and presentation |
| Vector and debug geometry | Points/dots, line segments, polylines, polygons, grids, axes, bounds, gizmos, and selectable debug overlays |
| Procedural primitives | Quads and planes, boxes/cubes, spheres, capsules, cylinders, cones, pyramids, toruses, and configurable primitive mesh generation |
| Meshes and models | Vertex/index data, attributes, submeshes, bounds, mesh handles, model instances, level of detail, skinning data, and import-ready formats |
| Terrain and procedural generation | Heightmap terrain, polygonal terrain, chunking, terrain level of detail, seeded Perlin/Simplex/value/Worley noise, fractal combinations, domain warping, and deterministic generation inputs |
| Materials and textures | Shaders, materials, textures, samplers, colour spaces, normal/PBR maps, render states, and reusable GPU resource handles |
| Lighting and shadows | Ambient, directional, point, spot, area, and environment lighting; shadow maps, cascades, point-light shadows, filtering, and explicit quality budgets |
| Particles and effects | CPU and GPU emitters, spawn shapes, lifetime/curve data, billboards, mesh particles, trails, collision hooks, pooling, and deterministic seeds where required |
| Animation | Skeletal clips, state machines and blending, morph targets, procedural animation, timelines, and root-motion contracts |
| Image and post processing | HDR, exposure and tone mapping, anti-aliasing, fog, bloom, colour grading, depth effects, screen-space passes, and render-to-texture |
| 2D and text | Sprites, atlases, tilemaps, 2D cameras, vector shapes, fonts, text layout, and world-space or screen-space text |
| Audio | Output devices, clips and streaming, mixer buses, spatial sources/listeners, attenuation, Doppler, effects, music transitions, and bounded voice management |
| Input | Keyboard, pointer, gamepad/joystick, touch, action mapping, rebinding, dead zones, focus, and deterministic input snapshots for fixed simulation |
| Physics and navigation | Collision shapes and queries, rigid bodies, joints, character controllers, triggers, navmeshes, pathfinding, steering, and spatial queries |
| UI and interaction | Retained UI hierarchy, layout, styling, widgets, HUDs, focus/navigation, accessibility hooks, picking, and project-mode inspectors |
| Assets and authored worlds | Import, metadata, dependency tracking, caching, hot reload, scenes, reusable entities, durable IDs, validation, and platform variants |
| Game services | Save data, localization, replay, deterministic recording, networking, replication, and platform service boundaries |
| Tools and diagnostics | Structured logging, metrics, CPU/GPU profiling, frame capture hooks, graph inspection, debug drawing, console commands, build diagnostics, and project mode |
| Platform and delivery | Windows/Linux/macOS adapters, output selection, packaging, capability checks, target-specific settings, and native validation |

SlotMap handles remain canonical runtime identity across these families. Subsystems such as rendering, audio, physics, navigation, and assets own their resource stores and expose typed handles; entities refer to those handles through components or behaviours. Dense GPU, audio, physics, or particle work may use packed subsystem storage and internal indices, with explicit mapping back to canonical engine identity where identity is needed. Durable project assets use stable authored IDs and resolve to fresh runtime handles during loading.

Keep expensive import, shader compilation, mesh processing, terrain baking, audio decoding preparation, and dependency analysis away from frame-critical paths. Runtime facilities define budgets and degradation rules for lights, shadows, particles, audio voices, streaming, and procedural work before claiming production readiness. Seeded generation must record the algorithm/version and parameters when reproducibility matters; the word “Perlin” or “random” alone is not a reproducibility contract.

### Game loop

Use a fixed simulation step and independent render presentation. Define the initial simulation frequency as configuration, not an implied performance guarantee.

The initial loop should:

1. Collect platform events and accumulate elapsed time.
2. Run a bounded number of fixed ticks, reading committed behaviour state, evaluating behaviours, committing commands and outputs, and propagating transforms.
3. Extract a read-only render view and present it, with interpolation where needed.
4. Record timing and backlog diagnostics.

Specify elapsed-time clamping, a maximum catch-up count, and what happens to excess backlog so slow frames cannot produce unlimited work. Define pause, shutdown, and scene teardown. Headless tests advance exact tick counts without wall-clock sleeps. Repeatability means equivalent results under the documented configuration; cross-platform bitwise floating-point determinism is not promised.

## Game projects and AI-assisted authoring

### Project boundary and ownership

A game is a project that depends on Emergent through its public Rust API and a versioned content contract. The developer owns its source, scenes, reusable entity compositions, behaviour implementations, and assets. Developers must be able to create and run it without editing engine source or accessing private engine storage.

Provide a minimal project template and a documented create, validate, build, run, and debug workflow. Begin with Cargo, a small authoring tool, and an optional visual project mode; exact command names and file formats are Stage 0 decisions. The same files and tools serve human authors and AI coding generators. A full visual editor or embedded AI service is not required.

Illustrative layout, to be finalized in the project-contract spike:

```text
my_game/
  Cargo.toml                 # Engine dependency and game build configuration
  emergent.toml              # Project/schema version, entry scene, asset roots, build outputs
  AGENTS.md                  # Project conventions and graph authoring instructions
  src/
    main.rs                  # Application assembly and game module registration
    entities/                # Custom components, entity builders, game systems
    behaviours/              # Behaviour implementations and type registration
  assets/
    scenes/                  # Scene roots, instances, hierarchy, behaviour links
    entities/                # Reusable entity/sub-entity compositions
    data/                    # Authored game configuration
  tests/                     # Graph validation and headless scenario tests
```

Add mesh, texture, and audio folders when those features arrive. Derived caches and build output remain separate from authored assets and can be regenerated. Paths are project-relative with portable separator and casing rules; validate them consistently on Windows, Linux, and macOS.

### Project graph and multiple build outputs

The game project graph includes shared source modules, scenes, reusable entity definitions, assets, and named build-output nodes. One game project can select any subset of the supported operating systems: Windows, Linux, and macOS. Each selected output depends on the shared game content and its explicit target configuration; producing another platform build must not require duplicating the project or its scene graphs.

- Define outputs in the project manifest with a stable output ID, target OS, architecture/Rust target triple, build profile, entry scene, asset selection, and packaging configuration. Allow multiple outputs for the same OS, such as different architectures or profiles. Resolve defaults explicitly and validate conflicting or unsupported combinations.
- Model build dependencies as an acyclic graph. An output selects its scene/content roots and includes their transitive asset and code dependencies. Validate the resolved graph for each output so platform-specific choices cannot leave missing assets, types, or behaviour links.
- Keep common settings shared and target overrides explicit. Platform adapters, feature requirements, and asset variants must be inspectable by authoring tools and AI generators. Optional GPU features need capability checks or a declared minimum requirement.
- Let the developer build one output, a selected group, or all enabled outputs. Produce separate artifact directories, for example `dist/<output-id>/`, containing the executable or application bundle, required assets, and a build manifest identifying the target and input versions. Never overwrite another output's artifacts.
- Distinguish the build host from each output target. Record required toolchains, linkers, SDKs, and native or CI runners. Selecting an OS does not guarantee cross-compilation from every host; unavailable prerequisites produce an actionable per-output result.
- Report validation, compilation, packaging, and native runtime verification separately for every output. A successful build for one platform cannot mark another as verified. The engine targets all three operating systems; individual games choose which to distribute.
- Begin with runnable development packages and a small Cargo-based build plan. Store publishing, installer automation, and release signing/notarization automation are later distribution work.

Expose output nodes, resolved dependencies, target configuration, and build results through the same structured project-graph tooling used by AI generators. Test output selection and isolation, including a project configured for all three operating systems.

### Developer extension contract

- Developers define custom entity compositions, component data, behaviours, and scheduled game systems in their own project or reusable Rust crates. Offer a documented module registration API rather than requiring edits to a central engine source file.
- Register stable, namespaced type identifiers and configuration schemas for content-facing types. Include behaviour input/output types, defaults, lifecycle rules, and declared scheduling/access requirements.
- Scene files instantiate registered types and wire their exposed behaviour ports. Executable behaviour logic starts as compiled Rust; scene data does not contain arbitrary executable code.
- Game systems use supported queries, commands, and lifecycle hooks while respecting SlotMap identity, hierarchy invariants, and tick semantics. Developers can extend the model without substituting a second authoritative identity graph.
- Reusable entity definitions compose sub-entities with explicit parameters and overrides. Define instance-local identities and reference resolution; repeated instances must not collide. Detect recursive composition dependencies. Advanced inheritance and unrestricted override languages are deferred.

### Authored graphs and runtime graphs

Make the following structures explicit and inspectable:

| Graph | Meaning | Initial constraints |
| --- | --- | --- |
| Entity hierarchy | Ownership and transform inheritance | Acyclic; at most one parent per entity |
| Behaviour links | Typed data flow and feedback | Valid ports; cycles follow committed-tick semantics |
| Composition and asset dependencies | Reusable definitions and referenced project content | Resolvable references; reject recursive composition expansion |
| Module and system dependencies | Startup and execution requirements | Valid dependencies and unambiguous supported scheduling |
| Project build outputs | Shared game source/content feeding selected OS and architecture artifacts | Acyclic build dependencies; valid target configuration; isolated outputs |

Authored scenes, entities, behaviour instances, and assets use durable identifiers, never serialized SlotMap or ECS keys. Define identifier scope explicitly. Instantiate reusable definitions with an instance namespace, then resolve authored identifiers into fresh runtime keys. Renaming or moving an asset should preserve its durable identity; duplicate identifiers and unresolved references are validation errors.

Load through explicit parse, schema-check, graph-validation, instantiation, and link-resolution phases. Validate the complete candidate scene before activating it; failure must not leave a partially active scene. Authoring metadata and expensive graph checks belong in tools or loading, not repeated frame-critical work. Runtime mutation still enforces the engine's invariants.

### Contract for AI coding generators

Provide versioned, machine-readable schemas and a registry export describing available entity/component types, behaviour ports, module dependencies, and supported graph operations. Derive metadata from registration where practical and test it against runtime contracts to prevent documentation drift.

Provide a structured graph inspection/export operation so generators can discover existing nodes, stable identifiers, edges, configuration, and source locations before editing. Keep authored structure distinguishable from instantiated runtime state. Inspection output is derived information, not a competing editable source of truth.

The intended generator workflow is:

1. Read the project manifest, selected build outputs, local guidance, API/schema versions, registered capabilities, and existing graph export.
2. Create or extend Rust entity/behaviour implementations and register their public contracts.
3. Generate or edit scene and reusable-composition files with explicit identifiers and typed references.
4. Run schema and graph validation for the selected outputs, compile with the available target toolchains, and execute focused headless scenarios on supported runners.
5. Inspect diagnostics and runtime graph results, correct failures, then run the visible game when relevant.

Diagnostics must offer both readable text and structured output with stable error codes, file locations, node/edge identifiers, and expected versus actual types where applicable. Unsupported schema versions must fail clearly; never silently reinterpret a project. Generated files remain ordinary reviewable source and assets under version control. The workflow must not depend on a particular AI vendor, model, or live AI connection at runtime.

### First end-to-end proof

Create a separate sample game using only public engine interfaces. Supply project guidance and schemas to an AI coding generator and have it add a custom behaviour, a reusable composite entity, and an authored scene using both. The scene must include multiple instances and behaviour links between them. Record the generation instructions, resulting artifacts, validation results, and any manual corrections; do not claim unattended success if corrections were needed.

Prove that the project builds and runs without engine source changes, its graph export matches the intended composition, and invalid links produce actionable diagnostics. This is an acceptance gate for the developer framework, not merely a future tooling aspiration.

## Build, run, debug, and visual project mode

Offer an optional project-mode application alongside the developer's editor and AI coding tools. It uses the game's compiled modules, authored scene graph, and Emergent renderer to display and inspect a constructed world with gameplay time stopped. This is a practical visual companion to code-driven development, with a small initial scope rather than a requirement to build a complete editor first.

### Build and launch workflow

- Validate the project and selected output, compile its game modules, and launch either project mode, play mode, or a headless scenario. Expose these operations through documented commands and launch configurations; exact command syntax remains a design decision.
- Record the project revision, build ID, target, entry scene, generation parameters, and seed in each session. Clearly show whether the viewport represents the current source or the last successful build.
- Support a debuggable development profile with symbols and documented native debugger launch/attach instructions. Keep optimized profiling runs separate from debug measurements.
- Rebuild and restart explicitly after Rust changes in the first implementation. Allow validated scene/configuration reload where supported. Failed builds or reloads retain the last valid preview and display diagnostics; they must not silently show stale content as current.
- Project mode runs on a compatible host output. Other selected OS outputs retain their own build and native verification requirements; the viewport is not an emulator for them.

### Separate construction, inspection, and simulation

Define explicit execution phases so procedural content can exist before gameplay starts:

| State or phase | Work allowed | Gameplay clock |
| --- | --- | --- |
| Construct / regenerate | Load assets, run registered scene generators, create entities, resolve links, propagate transforms | Stopped |
| Project mode | Render, navigate an inspection camera, select objects, inspect graphs, edit authored parameters and placement | Stopped |
| Play | Execute the normal fixed-step gameplay schedule and present the world | Advances |
| Paused play / step | Inspect the current simulated world; advance exactly the requested number of fixed ticks | Stopped except for explicit steps |

Construction hooks are registered explicitly and have documented inputs, outputs, dependencies, and lifecycle. Do not run every gameplay behaviour with `delta_time = 0` to approximate construction. Keep gameplay timers, physics, AI, spawning, and game input inactive in project mode unless explicitly invoked through a supported preview operation. Rendering, tool input, and construction jobs use separate scheduling from gameplay time. Clear wall-clock backlog when resuming so time spent inspecting does not become catch-up simulation.

Generators accept recorded parameters and seeds. Re-running construction replaces the previous generated result coherently instead of appending duplicate entities. Long jobs expose progress and cancellation; incomplete or failed output does not replace the last valid scene. Record source/generator provenance and stable logical identifiers where available so inspected results can be traced to their definition. Repeatability is tested under the documented build and numerical assumptions.

Use the same construction pipeline for headless builds, project previews, and game startup. If content inherently needs simulation to develop, expose an explicit bounded pre-roll or bake operation with tick count and seed, followed by a pause; never advance gameplay silently merely to show a preview.

### Inspect terrain and place a home base

The first project-mode example should use a small game-defined procedural heightfield or similarly bounded terrain generator. It proves the workflow without committing to a production terrain engine.

1. Open the game project and construct terrain from its saved seed and parameters while the simulation tick remains zero.
2. Navigate with an independent orbit/fly camera. Display a grid, axes, world coordinates, and selectable generated objects or terrain regions.
3. Pick a point on the terrain using a surface query and display its position and normal. Allow the game module to expose overlays such as slope, buildable regions, resource locations, or candidate spawn sites.
4. Preview a home-base marker or entity footprint at the selected location. A game-defined placement rule reports acceptance or rejection with a reason; the engine does not hard-code what makes a valid home base.
5. Save an explicit placement definition or generator parameter in the project's authored assets, then regenerate to verify it. Represent the placement with a durable authored ID and a documented anchor policy, such as world coordinates or a generator-provided logical anchor. If terrain changes invalidate it, report that instead of silently relocating it.
6. Start play from the constructed scene, pause or single-step to inspect behaviour, then stop and reconstruct the authored scene.

### Inspection, debugging, and edit persistence

- Provide an entity hierarchy, selection highlighting, property inspection, behaviour state and input/output inspection, and a focused view of incoming/outgoing graph links. Show authored IDs, runtime handles, and source locations as distinct fields.
- Offer logs and diagnostics linked to entities, behaviours, files, and build outputs, plus simulation tick and basic timing counters. Use existing native Rust debuggers for code breakpoints; semantic pause/step controls operate at coherent engine tick boundaries.
- Keep authored project data separate from generated preview data and transient play state. Save deliberate authoring edits through validated commands with undo/redo; do not serialize arbitrary runtime SlotMap state back into scene files.
- Starting play creates a transient session from the authored configuration. Stopping play discards gameplay mutations and reconstructs the preview. Arbitrary runtime-state promotion and rewind/time-travel debugging are deferred.
- Route property and placement edits through the same schema and graph validation contracts used by coding tools. Initial editing can be limited to generator parameters, transforms, and placement markers.
- Detect external source/asset changes made by code builders. Use file revision checks before saving, surface conflicting edits, and require reload or reconciliation rather than overwriting newer files. A visible refresh/rebuild action is sufficient initially; seamless code hot-reload is not required.
- Expose structured inspect, regenerate, select, pause, step, and diagnostic operations for external tools, using durable references and session/revision checks. An AI coding generator should be able to inspect the same scene and results that the developer sees. Mutations are validated and committed at safe boundaries.
- Keep project-mode tooling optional and out of release builds unless explicitly requested. Its inspector metadata and overlays must not impose recurring work on normal gameplay hot paths.

## Windows, Linux, and macOS 3D foundation

Chosen graphics foundation: `wgpu` for GPU access and `winit` for native window events, with Emergent providing the 3D scene and rendering layer. The first integration pins `wgpu` 30.0.1 and stable `winit` 0.30.13 behind the optional `graphics` feature. `wgpu` is a graphics foundation, not a complete scene engine. This choice retains control of the SlotMap/entity architecture and enables a deliberately small renderer first.

Bevy's renderer is built on `wgpu`, making Bevy a relevant reference for ECS-to-render extraction and rendering architecture. ADR 0002 selects standalone `bevy_ecs` to supply component storage and scheduling while Emergent builds its own `wgpu` renderer. That integration requires explicit extraction of simulation data into render data; using `bevy_ecs` alone does not supply Bevy's renderer or an automatic graphics bridge. Reusing `bevy_render` would be a separate architectural decision with its own dependency and ownership implications.

The upstream `wgpu` platform matrix documents native Windows, Linux, and macOS support: Vulkan on Windows and Linux, Direct3D 12 on Windows, and Metal on macOS. `winit` also supports all three operating systems. Emergent's first window, indexed cube, depth target, resize, fixed simulation, and clean shutdown have been verified locally on Apple Silicon through Metal. Windows and Linux remain documented capabilities pending Emergent runtime evidence.

- Target Windows, Linux, and macOS as required native platforms. Validate Linux under both X11 and Wayland where available, and macOS through Metal. Record the OS version, architecture, backend, and GPU actually tested.
- Define minimum OS versions and supported CPU architectures during Stage 0, including whether macOS outputs cover Apple Silicon, Intel, or both. Keep graphics requirements within a tested common feature set and validate optional capabilities per adapter.
- Start with a camera, depth buffering, transforms, and simple coloured geometry sufficient to inspect composed entities. Use debug geometry or a procedural cube; rich object rendering comes later.
- Handle resize, minimization, surface recreation, and unsupported adapters with explicit behaviour and useful errors.
- Keep GPU resources owned by rendering, referenced through handles, and separate from simulation entity ownership. Extract and batch render data without forcing simulation to traverse GPU objects.
- Select a focused math crate during the foundation spike; preserve documented engine coordinate conventions at the API boundary.
- Pin compatible dependency versions when scaffolding and validate them together. Native launch and visual checks on all three operating systems are engine release gates; cross-compilation alone does not prove runtime compatibility.

If a higher-level rendering framework becomes necessary, evaluate it against these ownership and performance boundaries before adoption. `bevy_ecs` evaluation does not imply adopting the full Bevy engine or its renderer.

## Staged delivery and acceptance gates

### Stage 0 — Resolve the foundation choices

- [x] Record identity/ECS boundaries, coordinate conventions, lifecycle commit points, and behaviour tick semantics in ADRs 0001–0004.
- [ ] Define a dependency-validated application/module builder with ordered startup and shutdown.
- [x] Define a versioned project manifest, durable project/output IDs, asset roots, and a tiny independent project fixture.
- [ ] Define the game project manifest, authored scene/composition format, durable identity scopes, registration/schema export, and graph inspection/validation contract with a tiny project fixture.
- [ ] Define build-output graph nodes, OS/architecture support, minimum OS versions, per-target configuration, artifact layout, and required build/verification runners.
- [ ] Define construction versus simulation schedules, project/play transitions, authoring edit persistence, external-file conflict handling, and the minimum project-mode tool interface.
- [x] Prototype equivalent representative workloads with `hecs`, `bevy_ecs`, and a focused SlotMap baseline; select and justify the component/system direction.
- [ ] Include simulation-to-render extraction in the ECS comparison, using the chosen `wgpu` foundation and retaining authoritative SlotMap identities.
- [ ] Verify the chosen graphics/window dependency combination with a minimal Windows, Linux, and macOS smoke example.
- [ ] Record target hardware, workload sizes, simulation rate, and provisional frame-time and memory budgets before making performance claims.

Exit: evidence-backed decisions and a bounded implementation plan. Separate measured, documentation-only, and unverified results. An unavailable target machine leaves its runtime gate pending.

### Stage 1 — Headless world and composition

- [x] Establish a Rust workspace and minimal application start/fixed-update/shutdown lifecycle.
- [x] Implement mathematical conventions, transforms, typed SlotMap identities, entities, scene roots, and hierarchy operations.
- [x] Implement the selected component storage direction and internal identity mapping.
- [x] Demonstrate nesting, reparenting, transform propagation, and subtree destruction without graphics dependencies.

Exit: tests cover cycle rejection, stale keys, world boundaries, deep hierarchies, mapping cleanup if applicable, and transform correctness. Traversal avoids unbounded call-stack recursion.

### Stage 2 — Behaviour composition and simulation

- [x] Implement behaviour attachment, typed linking, initial values, committed snapshots, and stable execution/commit rules.
- [x] Add configurable fixed-step timing, bounded catch-up, and exact headless tick control.
- [ ] Add interactive pause/resume and single-step controls with explicit backlog reset.
- [x] Build the linked-motion composition example and a feedback-cycle example.
- [x] Exercise link rewiring, target deletion, disabled behaviours, and scene teardown.

Exit: behaviour changes arise from composition and links; equivalent runs with the same initial state and inputs reproduce results within the documented numerical tolerance. Feedback cannot trigger recursive or unbounded within-tick execution.

### Stage 2A — Game project and authoring foundation

- [x] Supply an independent game project template using only public engine APIs, with project guidance for AI generators.
- [x] Define macOS, Windows, and Linux output declarations in the sample manifest; enable the locally verified macOS development output.
- [x] Reproduce the linked-motion example in project-owned generated Rust and run it headlessly and visibly without engine-source edits.
- [x] Load bounded schema-1 scenes referencing primitive/light/empty-node assets, resolve instance IDs to fresh SlotMap handles, and validate paths, schemas, values and parent graphs before construction.
- [ ] Extend loading to nested reusable compositions, authored behaviour links, full type registration and registry/schema exports.
- [ ] Provide structured registry and graph exports plus actionable validation diagnostics.
- [ ] Add manifest-driven output selection and build planning for Windows, Linux, and macOS, with isolated artifacts and explicit per-output prerequisite and result reporting.
- [ ] Test single-output and multi-output selection, shared content resolution, target overrides, unsupported targets, and artifact isolation.
- [ ] Move or reproduce the linked-motion example in authored project assets, including a project-defined behaviour and multiple composite instances.
- [ ] Test duplicate IDs, missing assets/types/ports, invalid link types, recursive composition, hierarchy cycles, unsupported schemas, and failed-load cleanup.
- [ ] Complete the AI-generated project proof and record any manual corrections.

The narrower live AI proof is complete: a cube Statement generated a project-owned spin helper, passed staged compilation and an independent check, and was tested in the reference game. Manual cleanup is recorded. The full authored-composition/type-registry proof above remains open.

Exit: a developer or AI generator can extend, validate, build, and run a headless game project without editing the engine. Minimal authored-scene persistence is included; full runtime save games remain deferred.

### Stage 3 — Minimal visible 3D foundation

- [x] Connect platform events and the renderer to the existing headless-capable runtime.
- [x] Display the composition example with a camera, depth-tested geometry, and inherited transforms.
- [x] Launch the independent reference game in a native window, with headless and bounded smoke modes.
- [x] Resolve the reference entry scene to its registered generated Rust builder for paused previews.
- [x] Launch the independent sample game through its entry scene and referenced primitive/light assets; preserve project Rust behaviour extensions.
- [ ] Build and package the sample project's selected Windows, Linux, and macOS outputs from the same source and authored scene graph using suitable runners.
- [ ] Verify native launch, resize, minimize/restore, presentation, asset loading, and clean shutdown on Windows, Linux, and macOS; record results per output.

Exit: the same simulation drives both headless and visible examples; platform/runtime evidence is recorded. No rich asset pipeline is required.

### Stage 3B — Render and media building blocks

- [x] Implement validated CPU models and renderer-owned SlotMap resources for a point, solid line segment, convex polygon, and indexed colour mesh.
- [x] Generate and render validated planes, cubes/cuboids, pyramids, spheres, cylinders, cones, capsules, and toruses through the indexed mesh contract.
- [x] Add validated ambient, directional, point, and spot light components on canonical entities, including colour, intensity, range/cone data, and shadow-casting intent.
- [ ] Promote cameras and renderable mesh/material instances to public, SlotMap-compatible engine concepts with explicit render extraction.
- [ ] Extend vector and debug geometry with polylines, concave and outlined polygons, grids, axes, bounds, overlays, and gizmos.
- [x] Own uploaded geometry through renderer-scoped generational handles and reuse per-instance uniform staging.
- [ ] Add texture resource ownership, material/shader contracts, visibility, culling, batching, and measured GPU upload rules. *(Diffuse textures, materials and a shared material library are delivered; culling, batching and upload budgets remain open.)*
- [x] Add versioned seeded Perlin, OpenSimplex and Worley noise filters, bounded 2D/3D generation, editable inspector inputs/seeds and grayscale slice previews.
- [x] Add a heightmap terrain consumer: generated/sculpted fields, painted materials, scene placement and heightfield collision. See [terrain plan](docs/plans/terrain.md).
- [ ] Add polygonal terrain and chunk/level-of-detail/streaming contracts (terrain plan stages 0–6). An experimental ray-marched variant is prototyped.
- [x] Extract ambient, directional, point, and spot lights into a bounded eight-light Lambert renderer, with authored/generated normals and inverse-transpose normal matrices.
- [x] Provide flat/smooth mesh shading and one directional shadow map with explicit first bounds (2048-square map, first eligible directional light).
- [ ] Establish measured shadow quality/performance budgets and extend beyond the first directional shadow path as required.
- [x] Add a bounded particle system with pooled CPU emitters, instanced billboards and flipbooks.
- [ ] Add a measured GPU particle path when workload evidence justifies it.
- [ ] Establish the audio device, clip/stream, mixer-bus, and spatial source/listener boundaries without coupling audio identity to entities.
- [ ] Publish asset formats and project folders for meshes, textures, terrain data, shaders, particles, and audio as each capability becomes available. *(Textures, materials, terrain, structures, particles, tweens and environment are authored; native GLB meshes, shaders and audio are not.)*

Exit: a game project can assemble a small lit scene from procedural and asset-backed geometry, terrain, particles, and spatial sound through public engine APIs. Each shipped capability has explicit ownership, loading, performance, and platform validation evidence.

### Stage 3A — Visual project mode and game debugging

- [x] Assess Selkies as a viewport transport: decline its current Linux-hosted stack for native Mac development; keep the persistent inspection session independent of transport. See `docs/research/selkies-macos-feasibility.md`.
- [x] Provide IDE Run/Stop, Cargo build diagnostics, bounded logs, and process status for the linked local game.
- [x] List project scenes in a selector and Load a real offscreen Rust/wgpu capture into the middle IDE canvas.
- [x] Construct the registered reference scene at tick zero without advancing gameplay; retain the last valid frame when a reload fails.
- [x] List authored scene instances and edit parent-relative position, rotation and scale with uniform scale, scene overrides, reset to asset defaults, guarded publication and Rust candidate validation.
- [x] Make the authored scene graph the primary IDE tree, with project files in a collapsible section, drag/drop parenting and keyboard parent selection; inherit parent translation, rotation and scale.
- [x] Edit Local/Global coordinates using Rust world math; retain local values on reparent and reject lossy shear conversions.
- [x] Edit scene and shared entity TOML beside Overview/Connections/Source, validating staged project scenes and rejecting stale or invalid saves.
- [ ] Provide build/launch/debug instructions and project, play, paused-play, and fixed-tick step controls with visible build/session status.
- [ ] Implement construction hooks and explicit regeneration with seeds, progress/cancellation, and coherent replacement of generated content.
- [x] Retain a native inspection world/renderer at tick zero with bounded browser frames, acknowledged controls, session generations and owned-process cleanup.
- [x] Navigate an orbit/pan/zoom inspection camera and apply saved instance transforms without restarting the worker; record initial transport measurements.
- [x] Target 30 fps for the retained inspection viewport, with measured frame delivery and no simulation advance or frame backlog.
- [x] Pick authored geometry and light/node markers in the viewport, synchronize scene selection, and edit with XYZ translate/rotate/scale gizmos in Local/Global space. Preview during drag, validate/save on release, and restore on cancellation or conflict.
- [ ] Add live runtime hierarchy and behaviour-state/link inspection, and source-linked diagnostics; authored hierarchy, picking and first axis gizmos are delivered above.
- [ ] Demonstrate procedural terrain construction while gameplay stays at tick zero, with a game-defined placement rule and a saved home-base placement.
- [ ] Verify reload/regeneration preserves valid authored placement and reports invalid anchors, failed generation, and external-file conflicts.
- [ ] Verify play/stop isolation, exact tick stepping, no accumulated pause-time catch-up, and undo/redo for supported authored edits.
- [ ] Let an external coding tool inspect the preview and invoke a bounded regenerate/inspect workflow through the documented interface.
- [ ] Exercise the project-mode workflow on Windows, Linux, and macOS and confirm release builds can omit the tooling.

Exit: a developer can build a game, inspect its generated world without running gameplay, choose and persist a home-base location, debug a play session, and return to the authored preview while working alongside code builders.

### Stage 3C — IDE authoring and AI workflows (added delivered scope)

- [x] Create a pnpm/Next.js IDE under `src/emergent-ide`, linked to a game project outside the web application.
- [x] Inspect real project files, inferred graph connections, asset details, and source through a minimal searchable tree and selected-file views.
- [x] Provide full-width lower chat bound to the selected file, with server-local AI URL/key settings and a working Responses connection.
- [x] Persist per-file Statements; check new Statements immediately and preserve them when AI checking fails.
- [x] Report Statement implementation evidence/confidence and offer explicit reuse-first Implement: inspect project/engine capabilities, compile bounded proposed project changes in isolation, check conflicts, publish with rollback protection, and independently recheck.
- [x] Demonstrate the workflow with a project-owned slow Z-axis cube rotation; record tests and manual corrections.
- [x] Add named asset templates through a categorized modal, respecting declared asset roots and showing available/preview/planned capabilities.
- [x] Optionally include new primitive/light/empty-node assets in the selected scene; asset edits drive real Rust captures and native Run, and invalid reloads preserve the last valid frame.
- [x] Refine modal visibility/scroll containment, simplify the tree and inspector, bind chat scope to selection, and deduplicate chat context keys.
- [x] Publish the HTML user guide (intro, getting started, Hello World, projects, capabilities, API) and machine-readable capability catalogue.
- [x] Expose the project to MCP clients (Claude Code and Codex, both verified) through durable `asset:`/`scene:`/`instance:` refs: read and view (MCP-1) plus revision-guarded, Rust-validated, undoable scene edits that appear live in the IDE (MCP-2). Asset creation, run and selection context (MCP-3) are next. See [design](docs/design/project-mcp-server.md).
- [x] Let the IDE's AI use Claude or Codex, with per-provider model selection.
- [x] Provide the installed `emergent` coding skill for idiomatic project and engine extension; this is local agent tooling, not a bundled runtime feature.

### Stage 3D — Object Creator (added delivered scope)

- [x] Add `/objects` with server-local fal credentials and hosted Microsoft TRELLIS image-to-3D generation.
- [x] Persist project-scoped jobs, request IDs, settings, history, recoverable polling, cached artifacts, and explicit failure states without automatic duplicate paid submissions.
- [x] Validate bounded image uploads and downloaded GLB output; preview cached and saved models with browser Model Viewer orbit/zoom/reset controls.
- [x] Save complete model/reference/descriptor bundles with durable asset IDs and provider provenance, collision protection, and explicit project-graph edges.
- [x] Observe a real hosted result saved as `assets/models/a-robot-arm/` in the reference project; retain the numeric texture-size provider fix and regression coverage in the working tree.
- [x] Verify the real hosted robot-arm GLB in the running IDE, reload and reopen via its descriptor; retain automated viewer/save/restart coverage with a GLB fixture and mocked generation.
- [ ] Add text-to-reference-image generation, native engine GLB loading/materials, and generated-model placement in scenes in later increments.

The detailed OC-1 acceptance checklist is archived in [the epic log](docs/history/epic-log-2026-10.md). Browser model preview does not establish native engine model support.

### Stage 3E — Additional delivered editors (added scope)

Materials, Object Builder, Terrain Builder (heightfield and ray-marched), Texture and Image/Sprite/Video library, Particle Factory, Tween Editor, scene Lighting/skybox and Player settings pages are delivered. Each has a row with links in [implementation status](docs/implementation-status.md). Player settings are not yet consumed at runtime.

### Stage 4 — Performance baseline and extension readiness

- [x] Record release baselines for transform propagation, ECS comparisons/integration, and behaviour graphs with workload and measurement limitations.
- [ ] Profile optimized builds across increasing scene sizes, hierarchy shapes, behaviour-link density, and structural churn.
- [ ] Record frame-time distributions, simulation/transform/render extraction costs, memory use, and steady-state allocations against agreed budgets.
- [ ] Improve demonstrated bottlenecks with dense iteration, buffer reuse, dirty propagation, or batching, then compare against the baseline.
- [ ] Document how a new module and behaviour are added without modifying core identity or hierarchy contracts.

Exit: a small working foundation with recorded limits and a clear next-feature boundary. Passing correctness tests alone does not establish gaming-grade performance.

## Explicitly deferred

The capability catalogue makes future systems first-class planning concepts without moving them into the current implementation milestone. Joystick/gamepad support and input mapping; rich 3D meshes and materials; rich media importing and streaming; production sound and spatial audio; physics beyond static/dynamic boxes and heightfield colliders; animation beyond reusable scalar tweens; a full visual editor beyond the delivered terrain sculpt/paint tools; runtime scripting; networking; runtime save-game/world-state serialization; time-travel debugging; seamless Rust code hot-reload; and dynamic plugins remain deferred until their staged work begins.

These inform module boundaries but are not tasks for the first foundation milestone. Basic platform event handling and minimal 3D debug geometry prove the runtime and renderer. Project folders, authored scene/composition files, asset reference resolution, custom Rust behaviours, and AI-facing graph contracts are included in the foundation. A bounded visual project mode, procedural terrain demonstration, placement authoring, and build/run/debug workflow are included in Stage 3A; advanced editor features remain deferred.

## Instructions for implementation agents

Work through the stages in order and keep this checklist honest. Complete the foundation decision spike before broad implementation. Keep proposed API sketches provisional until tested in the composition example. Update decisions with measured evidence, report missing validation explicitly, and preserve the SlotMap requirements in `AGENTS.md`.

Checked items record delivered slices, with evidence and limitations in the implementation status document. Unchecked items remain planned or partially implemented; completing individual slices does not automatically pass a stage's broader acceptance gate.

## Primary references

Checked while preparing this brief; select and record exact dependency versions during Stage 0.

- [wgpu upstream platform support and examples](https://github.com/gfx-rs/wgpu)
- [winit platform documentation](https://docs.rs/winit/latest/winit/platform/index.html)
- [wgpu backend features, including Metal on macOS](https://wgpu.rs/doc/wgpu/documentation/features/index.html)
- [winit macOS support](https://docs.rs/winit/latest/winit/platform/macos/index.html)
- [hecs overview and design](https://docs.rs/crate/hecs/latest/source/README.md)
- [bevy_ecs APIs, systems, and scheduling](https://docs.rs/bevy_ecs/latest/bevy_ecs/)
- [Bevy ECS standalone use](https://docs.rs/bevy/latest/bevy/ecs/)
- [Bevy renderer and wgpu](https://docs.rs/bevy/latest/bevy/render/)

## Architecture decisions recorded after the original brief

- **Physics:** standalone `rapier3d` behind the optional `physics` feature. Emergent owns
  scheduling and identity; backend handles stay private and are never authored.
  Physics steps at the fixed timestep after gameplay commands and before final transform
  propagation; inspection never advances it. See [physics design](docs/design/physics-foundation.md).
- **Units:** one world unit is one metre. See [units audit](docs/design/world-units-and-scale-audit.md).
- **Materials:** engine values on canonical entities; GPU resources are owned per renderer
  `GeometryId`. See [material design](docs/design/material-editor.md).
- **Terrain:** heightfield remains the production representation; ray marching is an
  experimental variant pending collision and budgets. The first performance increment
  adds bounded visibility, preview quality and diagnostics, followed by optional
  GPU pass profiling and native camera-route evidence; see [terrain plan](docs/plans/terrain.md)
  and [measured limits](docs/benchmarks/terrain-performance/README.md). Continuous-world
  stage gates remain open.
