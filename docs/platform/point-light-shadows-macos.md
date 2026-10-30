# SH-2 point-light shadows — macOS evidence

2026-10-25, Apple M3 Pro / Metal, Rust/Cargo 1.99.1 stable-aarch64-apple-darwin.
Functional validation only; no performance/scene-size budget claimed.

Passed:

- Workspace debug tests and release/all-feature tests (including point-face
  projection, near/far depth, invalid bounds and CPU/WGSL frame-layout checks).
- Formatting, all-target/all-feature Clippy with warnings denied, and rustdoc.
- IDE TypeScript/production build, lint and 38 unit tests.
- `cargo run --release --features graphics --example point_shadow_smoke`:
  compares actual captured images in +/-X/Y/Z directions, with 14541 changed
  bytes per direction when a caster is disabled. Disabling the receiver produces
  exactly the same unshadowed image. Also verifies moved lights, both point-shadow
  slots contributing independently, surfaces beyond the range, explicit slot-limit
  errors, and directional/point coexistence.
- Existing native `shadow_smoke` still passes (1170/1770 changed bytes), and the
  native `material_smoke` still passes all shading/material/noise comparisons.
- `EMERGENT_SHADOW_SCENE=assets/scenes/point-shadows.scene.toml pnpm --filter emergent-ide test:shadows`:
  the real paused Rust worker changes point shadows when inspector cast/receive
  toggles are saved. Gameplay stays at tick zero, original shared assets remain
  unchanged and temporary scene/session are removed.
- A real 1280x720 tick-zero scene preview succeeds on Metal and was visually
  inspected: /tmp/emergent-point-shadow-preview.png.

The renderer retains twelve 512-square Depth32Float layers (12 MiB), plus the
existing directional map (16 MiB). A reused aligned matrix buffer drives all depth
passes. At most two point lights can cast simultaneously; eight total lights are
still supported. Extra point-shadow requests fail explicitly. Point range/position
bounds are documented in the design, along with near-plane and bias semantics.

Spot shadows, more slots/resolution controls, seam-aware filtering, culling/caching,
cascades and Windows/Linux native validation remain open. Main-scene/key-light
assets and user placements were preserved; the existing key point light already
has shadow casting enabled.
