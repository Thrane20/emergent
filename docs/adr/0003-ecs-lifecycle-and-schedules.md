# ADR 0003: ECS lifecycle and fixed schedules

Status: accepted for the first production ECS integration.

## Decision

`World` owns both the canonical SlotMap graph and a private Bevy component world. A `SecondaryMap` records the one-to-one mapping from each live SlotMap key to its internal Bevy entity. Every mapped Bevy entity has an engine-owned `EntityIdentity` component containing the world-scoped canonical `EntityId`.

Game code may add ordinary Bevy components and resources through Emergent's `World` boundary. It may register Bevy systems in `PreFixed`, `Fixed`, and `PostFixed` schedules. Bevy entities are internal implementation handles and are not exposed by the component API.

Scheduled systems must not spawn or despawn Bevy entities directly or remove `EntityIdentity`. They request supported entity creation and subtree deletion through `EntityLifecycleQueue`. The engine commits that queue once per tick after `Fixed`, publishes ordered `EntityLifecycleResults`, propagates SlotMap transforms, and then runs `PostFixed`.

The first queue contract does not provide pending handles. A command batch cannot refer to an entity spawned earlier in the same batch. Direct world setup and game callbacks may use the synchronous `World` lifecycle methods.

## Tick order

1. Publish `SimulationTime` for the current fixed tick.
2. Run `PreFixed`.
3. Run the compatibility `Game::fixed_update` callback.
4. Run `Fixed`.
5. Commit `EntityLifecycleQueue` in insertion order and publish results.
6. Propagate the authoritative SlotMap transform graph.
7. Run `PostFixed`.
8. Run `Game::after_fixed_update` and advance the tick counter.

## Integrity and failure handling

- Component boundary access validates the requested mapping and identity in constant time.
- Subtree removal validates the entire affected mapping before unlinking or deleting anything.
- Schedule boundaries use a cached query for archetypes lacking both `EntityIdentity` and Bevy's resource marker, plus an `EntityIdentity` removal hook. This rejects direct Bevy structural changes without iterating every canonical entity. Debug builds additionally validate every SlotMap mapping after each schedule.
- A detected direct ECS lifecycle mutation stops the engine with `EngineError::World`; the world is considered unusable after that integrity error.
- Failed queued lifecycle commands do not mutate either store and appear as explicit ordered results. Other commands in the batch continue.
- Stale SlotMap generations cannot resolve through `SecondaryMap` to replacement Bevy entities.

## Performance implications

Dense component systems stay in Bevy queries and avoid mapping lookups. Graph crossings and direct component access pay one mapping validation. Schedule-boundary structural validation checks only archetypes that could contain unauthorized entities and an integrity flag; it does not iterate canonical component entities. Full diagnostic validation remains linear in the number of canonical entities.

## Consequences

- `bevy_ecs` becomes a production dependency with minimal features (`std` only).
- `hecs` remains a development-only comparison dependency and does not enter the runtime graph.
- The SlotMap hierarchy is not mirrored into Bevy relationships.
- Transform data remains in the SlotMap graph during this iteration.
- The `Game` callbacks remain available while scheduled systems become the normal path for dense simulation work.
