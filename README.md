# Emergent

Emergent is an experimental Rust game engine built around SlotMap-owned entity and behaviour graphs. The current foundation is deliberately headless and small: it provides world-scoped handles, parent/child composition, local and global 3D transforms, iterative dirty-subtree propagation, Bevy ECS component storage and fixed schedules, typed behaviour links, previous-snapshot evaluation, and a fixed-step game loop.

## Prerequisite

Install the stable Rust toolchain with [rustup](https://rustup.rs/). After installation, verify it is available:

```sh
rustc --version
cargo --version
```

## Run

```sh
cargo run
```

The reference game creates a leader and two followers. Typed behaviour links make each follower track the previous entity's committed position. It advances at 60 simulation ticks per second, reports all three positions, and exits after 120 ticks.

## Check

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Run the transform propagation baseline with:

```sh
cargo run --release --example transform_bench
```

Graphics, asset projects, and project mode remain later layers described in `GOALS.MD`. Coordinate and transform conventions are recorded in `docs/adr/0001-coordinate-transform-conventions.md`.

The completed transform-graph increment is described in `ITERATION_2.md`.

The development-only ECS comparison harness can be run with:

```sh
cargo run --release --example ecs_compare -- all
```

The resulting standalone `bevy_ecs` direction, including its costs and SlotMap integration rules, is recorded in `ITERATION_3.md` and `docs/adr/0002-ecs-direction.md`.

The production component mapping, lifecycle queue, and `PreFixed` / `Fixed` / `PostFixed` schedule contract are described in `ITERATION_4.md` and `docs/adr/0003-ecs-lifecycle-and-schedules.md`.

Run the complete 100,000-entity fixed-tick baseline with:

```sh
cargo run --release --example runtime_bench -- all
```

The recorded workload and results are in `docs/benchmarks/iteration-4-runtime-integration.md`.

The typed behaviour graph, snapshot semantics, unavailable-input rules, and lifecycle ownership are described in `ITERATION_5.md` and `docs/adr/0004-behavior-graph-snapshots.md`. Run its release baseline with:

```sh
cargo run --release --example behavior_bench -- all
```

Recorded results are in `docs/benchmarks/iteration-5-behavior-graph.md`.
