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

Run the first visible 3D example with:

```sh
cargo run --release --features graphics --example first_window
```

It opens a native `winit` window and renders a rotating, depth-tested cube through `wgpu`. The cube is a canonical Emergent entity; the renderer receives a copied global transform after fixed simulation. Use the self-closing platform smoke mode with:

```sh
cargo run --release --features graphics --example first_window -- --smoke
```

Run the first-class geometry gallery with:

```sh
cargo run --release --features graphics --example geometry_gallery
```

It renders a solid point marker, line segment, convex polygon, and custom indexed mesh as four independently transformed entities. Add `-- --smoke` for the self-closing native validation run.

Run the procedural primitive gallery with:

```sh
cargo run --release --features graphics --example primitive_gallery
```

It generates and renders a plane, cube, pyramid, UV sphere, cylinder, cone, capsule, and torus through the public headless `Primitive` and `Mesh` APIs. Add `-- --smoke` for native resize and shutdown validation.

Lit geometry is available through `GeometryRenderer::render_lit`. It accepts copied `LightInstance` values derived from canonical `LightSource` entity components and propagated transforms. The initial renderer supports eight lights per frame, Lambert ambient/directional/point/spot illumination, and one selected shadow-casting directional light with a fixed 2048-square depth map. Missing mesh normals are generated at upload; use `MeshVertex::with_normal` to author one explicitly.

Run the independent reference game project with:

```sh
cargo run -p emergent-first-project
```

It validates `projects/first_project/emergent.toml`, reads a project-owned data asset, constructs its linked scene and point-light entity through public Emergent APIs, and advances the headless game. The checked-in project demonstrates the boundary between engine code, generated Rust, authored assets, and per-platform output declarations; authored scene expansion, light rendering, and output packaging remain later work.

## Check

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Default builds remain headless. The optional graphics boundary and first platform result are recorded in `ITERATION_6.md`, `docs/adr/0005-graphics-boundary.md`, and `docs/platform/iteration-6-macos-smoke.md`. Geometry ownership is recorded in `ITERATION_8.md`; procedural primitive contracts are recorded in `ITERATION_9.md`; light-source entity data is recorded in `ITERATION_10.md`.

## User guide

The HTML user guide lives under `docs/docs`. Serve it locally from the repository root:

```sh
python3 -m http.server 8000 --directory docs/docs
```

Then open [http://localhost:8000](http://localhost:8000). It includes the current feature overview, setup instructions, a compiled Hello World example, the independent game-project contract, the engine capability catalogue, searchable navigation, and a curated library/API reference.

Run the transform propagation baseline with:

```sh
cargo run --release --example transform_bench
```

The full capability horizon and project mode remain staged work described in `GOALS.md` and `docs/capabilities/engine-capabilities.toml`. Coordinate and transform conventions are recorded in `docs/adr/0001-coordinate-transform-conventions.md`.

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
