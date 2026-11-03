# Emergent

Emergent is an experimental Rust game engine built around SlotMap-owned entity and behaviour graphs. The current foundation is deliberately headless and small: it provides world-scoped handles, parent/child composition, local and global 3D transforms, iterative dirty-subtree propagation, Bevy ECS component storage and fixed schedules, typed behaviour links, previous-snapshot evaluation, and a fixed-step game loop.

## Prerequisite

Install the stable Rust toolchain with [rustup](https://rustup.rs/). After installation, verify it is available:

```sh
rustc --version
cargo --version
```

## Build everything

```sh
pnpm build             # Rust workspace (engine + standard template, release, all features), then the IDE
pnpm run build:rust    # cargo build --workspace --release --all-features
pnpm run build:ide     # next build in src/emergent-ide
```

The engine is a library crate; it is compiled as part of the game crate that uses it. The IDE's
tools (`--validate-scene`, `--inspect-scene`, `--simulate-brain` …) run that game binary.

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

Lit geometry is available through `GeometryRenderer::render_lit`. It accepts copied `LightInstance` values derived from canonical `LightSource` entity components and propagated transforms. The initial renderer supports eight lights per frame, Lambert ambient/directional/point/spot illumination, and one selected shadow-casting directional light with a fixed 2048-square depth map. Missing mesh normals are generated at upload; use `MeshVertex::with_normal` to author one explicitly. Use `insert_with_shading(..., ShadingMode::Flat)` when every triangle should retain a distinct face normal.

Create/open a game through the IDE project switcher. Shared native tooling lives
in `src/project_host`; new games use `templates/standard`. With no saved selection,
the IDE shows New/Open rather than choosing a sample.

The optional sample game is outside the engine workspace. Run it explicitly with:

```sh
cargo run --manifest-path projects/first_project/Cargo.toml
```

It validates `projects/first_project/emergent.toml`, reads a project-owned data asset, constructs its linked scene and point-light entity through public Emergent APIs, and opens a native lit preview with a ground plane, moving cubes, and a pyramid. Use `-- --headless` for the bounded 120-tick non-windowed run or `-- --smoke` for native resize, presentation, and shutdown validation. It is ordinary game content, not the IDE contract or a source of new-project defaults.

## Check

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Default builds remain headless. The optional graphics boundary and first platform result are recorded in `docs/history/foundation-iterations.md` (iteration 6), `docs/adr/0005-graphics-boundary.md`, and `docs/platform/iteration-6-macos-smoke.md`. Geometry ownership is recorded in `docs/history/foundation-iterations.md` (iteration 8); procedural primitive contracts are recorded in `docs/history/foundation-iterations.md` (iteration 9); light-source entity data is recorded in `docs/history/foundation-iterations.md` (iteration 10).

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

The completed transform-graph increment is described in `docs/history/foundation-iterations.md` (iteration 2).

The development-only ECS comparison harness can be run with:

```sh
cargo run --release --example ecs_compare -- all
```

The resulting standalone `bevy_ecs` direction, including its costs and SlotMap integration rules, is recorded in `docs/history/foundation-iterations.md` (iteration 3) and `docs/adr/0002-ecs-direction.md`.

The production component mapping, lifecycle queue, and `PreFixed` / `Fixed` / `PostFixed` schedule contract are described in `docs/history/foundation-iterations.md` (iteration 4) and `docs/adr/0003-ecs-lifecycle-and-schedules.md`.

Run the complete 100,000-entity fixed-tick baseline with:

```sh
cargo run --release --example runtime_bench -- all
```

The recorded workload and results are in `docs/benchmarks/iteration-4-runtime-integration.md`.

The typed behaviour graph, snapshot semantics, unavailable-input rules, and lifecycle ownership are described in `docs/history/foundation-iterations.md` (iteration 5) and `docs/adr/0004-behavior-graph-snapshots.md`. Run its release baseline with:

```sh
cargo run --release --example behavior_bench -- all
```

Recorded results are in `docs/benchmarks/iteration-5-behavior-graph.md`.

First-person walking: select `assets/scenes/first-person.scene.toml` in the IDE and Run. Click the native window to capture the mouse; WASD moves, Space jumps, Escape releases. Losing focus clears input. The optional scene and `assets/prefabs/first-person.prefab.toml` are included in the standard template and reference project. For custom rigs, put `emergent.character_controller` on a dynamic root body and `emergent.mouse_look` on its child camera; make that camera active. Sensitivity is degrees per raw mouse pixel; pitch limit is degrees. Movement follows horizontal camera facing. This uses an upright box: capsules, automatic stair climbing and moving-platform attachment remain open. Only meshed heightfield terrain has collision; ray-marched terrain remains visual-only.

The **Shader Editor** (`/shaders`) creates project surface shader graphs from Force field, Water and Dissolve templates. Edit node inputs and exposed parameters, orbit the preview, pause/scrub time, or ask the AI to revise the unsaved draft. Save the shader, choose **Create material**, then choose that material in an object's Material inspector. Material Generator exposes the shader parameters. Native Run animates saved shader materials; the optional `assets/scenes/shader-showcase.scene.toml` demonstrates all three. Displacement needs subdivided geometry and does not change collision. See [shader design](docs/design/shader-editor.md) for current limits.

Select a scene object and open **Layers & tags** above Components. Use **Configure layers & tags…** to name up to 32 fixed slots per list. Pick one layer, toggle tags from the dropdown or use **Select tags…** for combinations. Configuration lives in the first asset root's `object-labels.toml`; objects use `[classification] layer = 0` and `tags = 0` (a u32 mask). Bit positions remain stable when names change. Runtime code reads `ObjectClassification` from the existing World entity; `matches(layer_mask, any_tag_mask)` matches a layer and any requested tag (zero tag filter ignores tags). This is membership/filtering data; physics collision and camera visibility rules remain separate.
