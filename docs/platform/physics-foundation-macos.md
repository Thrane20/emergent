# PH-1 physics foundation — macOS evidence

2026-10-24, Apple M3 Pro, Rust/Cargo 1.99.1 stable-aarch64-apple-darwin,
native Metal renderer. Physics is CPU Rapier 3D 0.36.1;
Metal describes rendering, not a GPU physics backend.

Passed checks:

- `cargo test --workspace --features physics` (debug integration).
- `cargo test -p emergent --no-default-features` (headless feature-off loading diagnostics).
- `cargo test --workspace --release --all-features` (physics and existing regressions).
- `cargo fmt --all -- --check`.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps`.
- IDE lint, TypeScript, 33 unit tests and production build.
- `EMERGENT_IDE_BASE_URL=http://localhost:3001 pnpm --filter emergent-ide test:physics`:
  an isolated scene uses real reference assets, streams native tick-zero frames,
  exposes inherited settings, changes gravity to 0.5, saves an instance override,
  reconstructs inspection and keeps the shared asset unchanged. Its temporary
  scene/session are removed. Screenshot inspected at `/tmp/emergent-physics-inspector.png`.
- `cargo run --release -p emergent-first-project -- --run-scene assets/scenes/physics.scene.toml --smoke`:
  completed 120 ticks and 240 presented frames, with the resize event observed.

Physics regressions cover falling, rest on static ground, scaled ground and collider
offset, zero gravity, stable scale for a rotated body, deletion/slot reuse, fresh
construction/reset, paused construction, invalid settings, competing pose writes,
unsupported parents and shear/negative scale. Project tests load the actual physics
demo, settle its cube and reconstruct its authored position without saving results.

No frame-time budget or physics benchmark established. Windows/Linux native execution
remains unverified. Kinematic bodies, public force/query/event APIs and other shapes
are outside PH-1. The top IDE Run button still launches the project entry scene;
use --run-scene to run the separate physics demo.

### Empty model-part root fitting (2026-11-03)

The BehaviourTest DroneUAV prefab has an empty root and one GLB child. Native scene loading now computes the root box from descendant geometry in root-local space when auto-fit is requested. A focused native test covers a nested pivot, dynamic root, no-geometry refusal, and a Static collider with kinematic drone locomotion. All 33 `authored_scene` tests passed with `--features physics`; Clippy passed with an existing unused `TerrainMaterial.layers` warning. The IDE scene-placement suite (8 tests), targeted ESLint, and a clean scoped TypeScript check passed. A read-only `--candidate` validation of the actual BehaviourTest scene with Static physics on DroneUAV passed without changing the project. The same candidate with Dynamic physics is correctly rejected because its `DroneLevel1` profile is kinematic (`type = "drone"`). The inspector now defaults to Static for that profile.
