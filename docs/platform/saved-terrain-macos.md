# Saved scene terrain — native macOS evidence

25 October 2026, Apple M3 Pro, macOS 27.0.1, stable Rust 1.99.1.

Implemented: versioned terrain entity assets and named scene instances; optional
static Rapier terrain collider; live physics type/enablement controls; normal
native mesh inspection/rendering, smooth default material and terrain framing.
No new dependencies. Recipes regenerate on load; terrain is not a frozen binary.

Passed:

- `cargo test --workspace` and `cargo test --workspace --release --all-features`.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features`.
- `cargo fmt -- --check`.
- IDE 51 tests, lint, type check and production build.
- `pnpm --filter emergent-ide test:terrain-save`: its own temporary project,
  unique Cargo package and production server on port 3002; save to a non-entry
  scene, seed edited after generation while saving displayed seed 42, native
  staged validation, selected saved instance after navigation, native terrain
  frames at tick zero, static Terrain collider and live disable/restart.
- Native unit checks compare all terrain collider cell triangles to mesh vertices
  including nonuniform scale. A falling box rests on non-flat translated/scaled
  terrain; deleting terrain removes contacts. Existing box tests remain passing.

The browser screenshot was visually inspected after resumed frames and showed
terrain with smooth lighting and its selected transform gizmo. User game assets
were left unchanged. The temporary worker/server/project were removed.

The save check exposed a pre-existing shared-output issue: a staged game binary
could overwrite the live named executable and retain a deleted manifest path.
Candidate code still receives Cargo checks; scene assets now validate through the
headless `scene_validate` engine example, which takes an explicit project root.
It does not overwrite the live game executable. Scene/instance navigation also
keys cached project-page state so View in scene selects the new terrain.

Limitations: static independent heightfields, positive non-sheared physics TRS,
existing scene lights, bounded camera range; no seamless chunks, LOD, frozen
heightfield export or dynamic terrain. Windows/Linux native execution was not
verified. No production-scale frame-time acceptance claim.
