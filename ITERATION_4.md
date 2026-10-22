# Iteration 4 — Production ECS integration

Status: complete and validated on 2026-10-22. Runtime measurements are recorded in `docs/benchmarks/iteration-4-runtime-integration.md` and the lifecycle contract is recorded in ADR 0003.

Version boundary: `0.4.0`, tagged `v0.4.0-ecs-integration`.

## Outcome

Integrate standalone `bevy_ecs` as Emergent's production component and schedule layer without weakening the existing SlotMap identity and hierarchy contracts. Every canonical entity receives exactly one internal Bevy entity. Game systems operate on Bevy components in explicit fixed-tick phases and request structural changes through an Emergent-owned lifecycle queue.

## Ownership and identity

- `World` continues to own the SlotMap entity store, hierarchy, local transforms, global-transform cache, and canonical `EntityId` values.
- `World` also owns a private Bevy component world and a `SecondaryMap<EntityKey, bevy_ecs::Entity>`.
- Each internal Bevy entity carries a read-only-by-construction `EntityIdentity` component containing its canonical `EntityId`.
- Bevy entity IDs never appear in Emergent's public identity or hierarchy APIs.
- Spawn commits the SlotMap entity, Bevy entity, identity component, and mapping as one world operation. Subtree removal validates every mapping first, then removes Bevy entities and mappings before releasing SlotMap keys.
- Public component access validates the canonical mapping and reports missing or corrupt mappings explicitly.

## Data flow and schedules

The fixed tick has three Bevy schedule phases:

1. `PreFixed`: prepare component state for the tick.
2. The existing game `fixed_update` callback, followed by `Fixed`: execute normal simulation systems.
3. Commit queued Emergent entity lifecycle commands, propagate the SlotMap transform graph, then run `PostFixed` for derived component work and observation.

The existing `after_fixed_update` callback runs after `PostFixed`. A `SimulationTime` resource exposes the current tick and fixed delta to systems.

Scheduled systems cannot directly access the private SlotMap store. They request supported structural changes through `EntityLifecycleQueue`. Commands execute in insertion order at the single commit point. Results are published through `EntityLifecycleResults` for the `PostFixed` phase and game callback. Newly spawned entities cannot be referenced by another command in the same batch in this first version.

## Hot paths and expected scale

- Dense component work stays in native Bevy queries and does not perform SlotMap-to-Bevy lookups per entity.
- Direct `World` component reads and mutations cross the mapping once and are intended for graph, tooling, setup, and other boundary work.
- Transform traversal remains the existing iterative SlotMap traversal.
- Mapping validation on ordinary boundary access is constant-time. Schedule boundaries use a cached archetype query plus an `EntityIdentity` removal hook to reject direct Bevy structural changes without scanning every canonical entity. A full validation API checks every canonical mapping for tests and diagnostics.
- Lifecycle command buffers are reused between ticks. The first implementation may allocate when a new peak queue size is reached.

## Validation

- Prove one-to-one identity mapping after root and child creation.
- Prove arbitrary user components can be inserted, queried by scheduled systems, mutated, removed, and cleaned up with a subtree.
- Prove SlotMap generation reuse never aliases an old Bevy mapping.
- Prove failed hierarchy commands publish explicit results and leave both stores coherent.
- Prove schedule phase order, fixed-time resource values, and the lifecycle commit point.
- Detect unsupported direct Bevy structural changes at a schedule boundary.
- Run formatting, debug and release tests, Clippy with warnings denied, rustdoc, and the existing release benchmarks.

## Deferred

- Mirroring transforms into ECS components or renderer extraction data.
- Pending handles that allow commands in one batch to refer to entities spawned earlier in that batch.
- Parallel schedule execution and multithreaded Bevy features.
- Behaviour instances and typed behaviour links.
- Serialization, durable authored identity, graphics, assets, and project mode.

## Acceptance gate

Iteration 4 is complete when `bevy_ecs` is a normal dependency, the runtime uses the mapping and schedules described above, mapping and command edge cases are tested, production dependencies contain no `hecs`, and the reference game still completes its 120-tick hierarchy demonstration.
