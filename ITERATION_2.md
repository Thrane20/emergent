# Iteration 2 — Transform Graph Foundation

Status: complete and validated on 2026-10-22. Benchmark evidence is recorded in `docs/benchmarks/iteration-2-transform-propagation.md`.

## Outcome

Turn the current entity tree into a useful spatial graph. An entity keeps an authored local transform, while the world computes and caches its global transform from its ancestors. The result remains headless and becomes the stable input needed by a later renderer, behaviour system, and project-mode inspector.

This is the next increment because every visible scene depends on transform semantics. It also exercises the SlotMap hierarchy under realistic traversal without committing Emergent to `hecs`, `bevy_ecs`, or a rendering architecture yet.

## Decisions to record first

Add a short architecture decision record covering:

- right- or left-handed coordinates;
- the up and forward axes;
- world units;
- rotation representation and composition order;
- local transform composition order;
- matrix layout at public API boundaries;
- how engine coordinates will map to `wgpu` clip-space conventions later.

Choose a focused SIMD-friendly Rust math crate after checking its native support and benchmark characteristics. Prefer using its vector, quaternion, and affine/matrix types internally over creating a broad custom mathematics library. Pin the selected crate version in `Cargo.lock`.

## Scope

### Transform data

- Expand `Transform` from translation-only data to local translation, rotation, and scale.
- Keep local transform data separate from the derived global transform cache.
- Give newly spawned entities identity transforms.
- Reject or explicitly handle transform values that cannot be represented safely. Define the policy for zero scale and non-finite values before exposing setters.

### Hierarchy propagation

- Add an explicit `World::propagate_transforms()` phase.
- Traverse roots before descendants without recursive calls, so deeply nested user scenes cannot exhaust the call stack.
- Compose every child's global transform from its parent's current global transform and its own local transform.
- Mark the affected subtree dirty when a local transform changes or an entity is reparented. Skip clean subtrees when doing so is demonstrably simpler or faster for the current layout.
- Preserve the current default reparenting rule: the entity keeps its local transform, so its global transform may change under its new parent.
- Keep hierarchy mutations coherent before propagation begins. A failed mutation leaves hierarchy links and cached transforms unchanged.

### Read and mutation APIs

- Replace unrestricted mutable access to transform fields with world methods or a mutation guard that reliably marks the entity and descendants dirty.
- Provide explicit local-transform and global-transform reads.
- Keep `EntityId` world-scoped and generational. Derived transform storage must use the same authoritative identity mapping and must not introduce another ownership graph.
- Avoid allocating during steady-state propagation after world capacity has settled.

### Reference example

Extend the small executable to build a three-level hierarchy:

```text
Scene
└── Carrier
    └── Scout
```

Move or rotate the carrier during fixed ticks and report the scout's derived world position. The example should make transform inheritance visible in console output without adding a window or renderer.

## Validation

- Test translation, rotation, scale, and combined parent/child composition using numerical tolerances appropriate for floating-point values.
- Test a multi-level hierarchy, multiple roots, reparenting, detaching to a root, subtree deletion, and propagation after stale-handle errors.
- Test that a local mutation invalidates all descendants and does not invalidate unrelated branches.
- Test a hierarchy deep enough to prove propagation and deletion do not rely on call-stack recursion.
- Verify clean propagation performs no entity changes and produces identical results.
- Run `cargo fmt --check`, `cargo test`, and Clippy with warnings denied.
- Add a small release-mode propagation benchmark for representative deep and wide hierarchies. Record the workload, hardware, toolchain, and results as a baseline; do not describe it as a performance target.

## Acceptance gate

Iteration 2 is complete when:

- coordinate and transform conventions are documented;
- global transforms remain correct after spawn, mutation, reparent, and deletion;
- deep hierarchy operations are iterative and tested;
- the reference game demonstrates inherited movement for 120 fixed ticks;
- the propagation benchmark and validation commands are recorded; and
- no ECS, renderer, window, asset, or project-mode dependency has been introduced.

## Deferred from this iteration

- `hecs` versus `bevy_ecs` selection;
- behaviour instances and behaviour links;
- `wgpu`, `winit`, cameras, and rendering;
- scene serialization and durable authored IDs;
- game project folders and build outputs;
- visual project mode and editing tools;
- preserve-world reparenting, shear decomposition, and animation.

These remain in `GOALS.MD`. The likely following iteration is a narrow behaviour-graph prototype or the ECS comparison spike, using this transform workload as one of its representative cases.
