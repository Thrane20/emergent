# ADR 0002: Use standalone Bevy ECS for components and systems

Status: accepted and locked in as the production direction; integration is deferred to a separate iteration.

## Decision

Emergent will plan around standalone `bevy_ecs` for general component storage, queries, resources, and scheduled system execution.

SlotMap remains authoritative for engine identity, entity hierarchy, scene membership, and behaviour identity. A Bevy entity is an internal component-storage handle mapped from an Emergent key. Emergent will not mirror its parent/child graph into Bevy relationships.

Use minimal Bevy ECS features initially: `default-features = false` with `std`. Add multithreading, reflection, serialization, tracing, or debug stepping only through separate evidence-backed decisions.

This decision ends the routine ECS comparison. Reconsider it only if production integration cannot preserve Emergent's SlotMap identity and hierarchy invariants, representative workloads exceed an agreed frame-time or memory budget because of the integration, or a required platform cannot support the dependency.

## Why

The Iteration 3 spike found that Bevy's direct and scheduled dense updates were fastest among the tested candidates. More significantly, Bevy supplies the scheduling, access analysis, change detection, resources, and deferred-work mechanisms that Emergent would otherwise have to design and maintain.

`hecs` has a much smaller dependency tree, but it does not supply the scheduling layer needed by the planned engine and did not win this workload. Focused SlotMap storage remains valuable for canonical identity and specialized hot data, but extending it into a general ECS would duplicate substantial infrastructure.

## Consequences

- Emergent must own a transactional mapping between canonical SlotMap keys and Bevy entities.
- Native Bevy queries should handle dense frame work; repeated canonical-to-ECS lookup is measurably more expensive.
- Entity creation and deletion go through Emergent lifecycle commands so the two stores cannot diverge.
- The engine accepts Bevy ECS's larger build, dependency, memory, and minimum-Rust-version costs.
- Public game APIs should expose Emergent concepts and documented system contracts without promising compatibility with every Bevy engine subsystem.
- Adopting `bevy_ecs` does not adopt the Bevy renderer, application framework, scene format, or entity hierarchy.

## Evidence

See `docs/research/iteration-3-ecs-comparison.md` and `examples/ecs_compare.rs`.
