# Iteration 3 — ECS comparison spike

Status: complete and validated on 2026-10-22. The recommendation and evidence are recorded in `docs/research/iteration-3-ecs-comparison.md` and ADR 0002.

Version boundary: `0.3.0`, tagged `v0.3.0-ecs-direction`.

## Question

Should Emergent use `hecs`, standalone `bevy_ecs`, or focused SlotMap-owned component storage for component queries and system execution?

SlotMap remains authoritative for Emergent entity identity, hierarchy, scenes, and behaviour identities throughout this spike. ECS-native entity IDs are internal storage handles mapped from an Emergent-owned typed key.

## Candidates

- `hecs` 0.11.2 with its default `std` feature: a small archetype ECS with direct query and random-access APIs, but no built-in schedule.
- `bevy_ecs` 0.20.0 with default features disabled and only `std` enabled: an archetype ECS with queries, change detection, resources, commands, and a system scheduler.
- `slotmap` 1.1.1 plus `SecondaryMap`: the smallest baseline with direct control and no archetype query or scheduler implementation.

The spike dependencies remain under `[dev-dependencies]`; completing the comparison does not place either ECS in the engine's production dependency graph.

## Workloads

Use the same position/velocity component and deterministic update for every candidate:

1. Native dense update over 100,000 entities.
2. Random access through 100,000 Emergent-owned SlotMap handles and, for ECS candidates, a `SecondaryMap` to ECS-native IDs.
3. Repeated spawn/despawn churn including canonical identity and mapping maintenance.
4. Bevy scheduled-system execution, measured separately from direct query iteration.

Run optimized builds and record checksums so each implementation is shown to perform equivalent work. Record process memory separately where practical. These are local microbenchmarks, not universal ECS rankings.

## Qualitative review

Compare:

- query and mutation ergonomics;
- explicit scheduling and deterministic ordering;
- change detection and deferred commands;
- ability to keep one authoritative hierarchy;
- stale-handle and mapping cleanup semantics;
- dependency count, feature selection, minimum Rust version, and build cost;
- suitability for AI-generated game systems and machine-readable registrations;
- escape hatches for specialized packed or renderer-owned data.

## Acceptance gate

The spike is complete when:

- all three equivalent workloads compile and run in release mode;
- dependency and benchmark evidence is recorded with its limitations;
- a small Bevy schedule proves standalone system execution;
- mapping creation and deletion rules are documented;
- the recommendation identifies a production direction and explicitly rejects or defers the alternatives; and
- neither `hecs` nor `bevy_ecs` has become a production dependency merely because it was evaluated.

## Out of scope

- migrating the current `World` implementation;
- implementing behaviour links;
- parallel schedule benchmarking;
- serialization and project schemas;
- rendering or `wgpu` extraction;
- a permanent abstraction supporting multiple interchangeable ECS backends.

Any production adoption follows in a separate iteration with a focused design and migration plan.
