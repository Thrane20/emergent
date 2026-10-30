# SH-1 object shadows — macOS evidence

2026-10-25, Apple M3 Pro / Metal, Rust/Cargo 1.99.1 stable-aarch64-apple-darwin.
Functional results only; no frame-time/performance budget claim.

Passed:

- Workspace debug and release/all-feature suites, formatting, all-target/all-feature
  Clippy with warnings denied, and rustdoc.
- IDE TypeScript/production build, lint and 38 unit tests, including boolean defaults,
  validation and all four independent cast/receive persisted combinations.
- `cargo run --release --features graphics --example shadow_smoke`: a two-caster,
  two-receiver native image comparison verifies removal of the selected caster's
  shadow (1170 changed bytes), exact agreement between disabling casting and
  disabling its receiver, and an unaffected second shadow (1770 changed bytes).
  Casters have receiving disabled; floors have casting disabled, proving the
  options are independent. Resources are removed after every captured combination.
- `pnpm --filter emergent-ide test:shadows` on the existing localhost:3000 server:
  a temporary copy of the authored shadow demo edits both checkboxes through the
  inspector, saves complete per-instance material overrides, reconstructs inspection
  and changes actual Rust-rendered pixels. Gameplay stays at tick zero; shared
  caster/floor assets remain byte-identical. Temporary scene and worker are removed.
  Screenshot: /tmp/emergent-object-shadows.png (visually inspected).

The new reference demo is assets/scenes/shadows.scene.toml. The user's main scene,
its point light and placements are preserved. Current backend remains a single,
2048-square origin-centred map for the first eligible directional light. Point/spot
shadows, cascades and other-platform runtime execution remain unverified/deferred.
