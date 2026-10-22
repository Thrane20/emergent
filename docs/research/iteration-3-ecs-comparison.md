# Iteration 3 ECS comparison

Recorded: 2026-10-22

## Result

Adopt standalone `bevy_ecs` as the planned component-storage and system-scheduling layer in a later production iteration. Keep SlotMap as the authoritative identity and hierarchy layer. Do not adopt `hecs` or build a general interchangeable-ECS abstraction.

This is an architecture direction based on a bounded spike. `bevy_ecs` remains a development-only dependency until a separate integration design establishes lifecycle transactions, scheduling phases, and the exact mapping API.

## Versions and configuration

- `slotmap` 1.1.1, already used by the engine.
- `hecs` 0.11.2 with its default `std` feature.
- `bevy_ecs` 0.20.0 with default features disabled and only `std` enabled.
- Rust 1.99.1 on `aarch64-apple-darwin`.
- Apple M3 Pro, 12 CPU cores, 36 GB memory.
- macOS 27.0.1 (26A434).

The crates were added under `[dev-dependencies]`, so the production engine dependency tree is unchanged.

## Benchmark method

The release-mode harness is `examples/ecs_compare.rs`:

```sh
cargo run --release --example ecs_compare -- all
```

Each candidate stored 100,000 identical position/velocity bodies and performed:

- 200 native dense-update iterations;
- 50 full random-access iterations through canonical SlotMap handles;
- 50,000 spawn/despawn operations including canonical mapping maintenance; and
- for Bevy, 200 executions through a standalone single-system `Schedule` after warm-up.

The ECS cases used a `SecondaryMap<CoreEntity, EcsEntity>` so handle-lookup and churn measurements include the mapping required by Emergent's architecture. All candidates produced the same checksum, `20624950.238`.

Three sequential complete runs produced these median values:

| Candidate | Native dense update | Canonical-handle lookup | Spawn/despawn churn |
| --- | ---: | ---: | ---: |
| SlotMap + `SecondaryMap` | 87.36 us | 128.84 us | 5.32 ns/op |
| `hecs` | 196.35 us | 382.42 us | 16.25 ns/op |
| `bevy_ecs` direct query | 75.58 us | 528.13 us | 26.82 ns/op |
| `bevy_ecs` scheduled system | 61.66 us | n/a | n/a |

Observed ranges across those runs:

| Candidate | Native range | Lookup range | Churn range |
| --- | ---: | ---: | ---: |
| SlotMap + `SecondaryMap` | 83.52–93.28 us | 124.69–133.10 us | 5.29–5.44 ns/op |
| `hecs` | 194.85–199.27 us | 382.00–385.32 us | 16.19–16.27 ns/op |
| `bevy_ecs` direct query | 75.55–75.59 us | 518.70–810.83 us | 26.59–27.30 ns/op |
| `bevy_ecs` scheduled system | 61.41–62.72 us | n/a | n/a |

The Bevy lookup result had one large outlier; its median and range are both retained rather than hiding that variability.

A later confirmation run, invoked through the documented command on the same machine, produced the same checksum for all candidates and measured:

| Candidate | Native dense update | Canonical-handle lookup | Spawn/despawn churn |
| --- | ---: | ---: | ---: |
| SlotMap + `SecondaryMap` | 126.56 us | 159.73 us | 6.70 ns/op |
| `hecs` | 217.70 us | 375.74 us | 16.22 ns/op |
| `bevy_ecs` direct query | 78.51 us | 523.25 us | 26.89 ns/op |
| `bevy_ecs` scheduled system | 63.82 us | n/a | n/a |

Run-to-run variation changed the absolute values but preserved the decision-relevant result: Bevy led dense scheduled work, while SlotMap led canonical lookup and structural churn.

Separate `/usr/bin/time -l` runs measured maximum resident set size for the same binary operating in one mode. Because the binary contains every candidate and macOS demand-pages code, these values are comparative observations rather than precise storage accounting:

| Mode | Maximum resident set size |
| --- | ---: |
| SlotMap | 8.0 MiB |
| `hecs` | 12.0 MiB |
| `bevy_ecs` | 18.2 MiB |

## Dependency and toolchain cost

`cargo tree` showed four unique enabled packages in the `hecs` subtree and 54 in the minimal `bevy_ecs` subtree. Adding both spike dependencies caused Cargo to lock 102 additional packages, including optional packages that were resolved but not necessarily built. The first combined release build completed in 11.07 seconds on the recorded machine; that figure is not attributable to either crate independently.

`cargo info` reports a minimum Rust version of 1.81 for `hecs` and 1.95 for `bevy_ecs` 0.20.0. Emergent currently builds with Rust 1.99.1.

## Qualitative findings

### SlotMap plus focused storage

Strengths:

- smallest dependency and memory footprint;
- fastest canonical-handle access and structural churn in this workload;
- direct alignment with Emergent identity and hierarchy ownership.

Costs:

- no archetype queries, access analysis, change detection, resources, deferred commands, or scheduler;
- expanding it into a general ECS would duplicate difficult engine infrastructure;
- an ad hoc component system would be harder for game developers and coding generators to target consistently.

Keep focused SlotMap or packed storage for identity, hierarchy, behaviours, renderer-owned data, and measured special cases. Do not turn it into a general ECS during the next iteration.

### `hecs`

Strengths:

- compact dependency tree and lower minimum Rust version;
- small API surface and ordinary Rust component types;
- useful direct-query and random-access facilities.

Costs:

- no built-in schedule, system dependency graph, resources, or change-detection contract;
- Emergent would need to design and maintain its own scheduler before reaching the behaviour and module goals;
- it did not outperform the focused SlotMap baseline or Bevy dense query in this workload.

`hecs` is a reasonable minimal ECS, but it is deferred because its main advantage is size while its missing scheduling layer is central to Emergent's direction.

### `bevy_ecs`

Strengths:

- fastest dense component update in this spike;
- standalone schedules, explicit ordering, resources, deferred commands, access analysis, and change detection;
- system function signatures form a clear contract for game code and generated code;
- mature escape hatches remain available for exclusive systems and specialized storage outside ECS.

Costs:

- substantially larger dependency and memory footprint;
- slower canonical-handle random access and structural churn in this spike;
- a higher minimum Rust version and a broader API surface;
- Bevy entity IDs and relationships must not become a second authoritative identity or hierarchy graph.

The scheduling and system model outweigh these costs for Emergent. Most frame-critical game work should use native ECS queries, where the measured mapping penalty does not apply. Canonical handle lookup remains a boundary operation for graph links, tools, and cross-subsystem references.

## Required integration rules

The production design must enforce these rules:

1. A core SlotMap insertion creates the canonical `EntityId`.
2. The Bevy entity is spawned as internal component storage and receives the canonical ID as a component.
3. A `SecondaryMap` maps canonical keys to Bevy entities. The complete mapping is committed before the entity becomes visible to scheduled systems.
4. Creation failure rolls back every partial insertion.
5. Deletion despawns the Bevy entity and removes the mapping before the canonical SlotMap key is removed.
6. Stale or missing mappings are explicit errors; they never resolve to a replacement entity.
7. Parent/child relationships remain only in Emergent's SlotMap hierarchy. Bevy relationship components are not used for the authoritative scene graph.
8. Game systems request lifecycle changes through Emergent commands committed at defined tick boundaries. They do not independently spawn or despawn unpaired Bevy entities.
9. Dense systems use Bevy queries. Canonical lookups are reserved for graph crossings and external references.
10. Specialized packed data and renderer resources may remain outside ECS when profiling supports that choice.

## Sources

- [`hecs` 0.11.2 documentation](https://docs.rs/hecs/0.11.2/hecs/)
- [`bevy_ecs` 0.20.0 documentation](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/)
- [Bevy ECS schedule APIs](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/schedule/)
- [Bevy ECS crate features and dependencies](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ecs/Cargo.toml)
