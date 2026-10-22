# Emergent

Emergent is an experimental Rust game engine built around SlotMap-owned entity hierarchies. The current foundation is deliberately headless and small: it provides world-scoped entity handles, parent/child composition, local and global 3D transforms, iterative dirty-subtree propagation, and a fixed-step game loop.

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

The reference game creates a scene, carrier, and scout hierarchy. It advances at 60 simulation ticks per second, reports the scout's inherited world position, and exits after 120 ticks.

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

Graphics, ECS selection, behaviours, asset projects, and project mode remain later layers described in `GOALS.MD`. Coordinate and transform conventions are recorded in `docs/adr/0001-coordinate-transform-conventions.md`.

The completed transform-graph increment is described in `ITERATION_2.md`.

The development-only ECS comparison harness can be run with:

```sh
cargo run --release --example ecs_compare -- all
```

The resulting standalone `bevy_ecs` direction, including its costs and SlotMap integration rules, is recorded in `ITERATION_3.md` and `docs/adr/0002-ecs-direction.md`.
