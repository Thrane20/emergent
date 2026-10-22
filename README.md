# Emergent

Emergent is an experimental Rust game engine built around SlotMap-owned entity hierarchies. The current foundation is deliberately headless and small: it provides world-scoped entity handles, parent/child composition, transforms, and a fixed-step game loop.

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

The reference game creates a scene and child entity, advances at 60 simulation ticks per second, prints progress, and exits after 120 ticks.

## Check

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Graphics, ECS selection, behaviours, asset projects, and project mode remain later layers described in `GOALS.MD`.
