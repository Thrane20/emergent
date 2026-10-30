# M-1 material editor — macOS evidence

2026-10-24, Apple M3 Pro, native Metal rendering. Rust/Cargo 1.99.1,
stable-aarch64-apple-darwin. This is functional evidence, not a performance benchmark.

Passed:

- Workspace debug tests and release/all-feature tests.
- `cargo test -p emergent --no-default-features`: materials remain headless-capable.
- Formatting, Clippy all-target/all-feature checks with warnings denied and rustdoc.
- IDE lint, TypeScript/production build and 37 unit tests.
- `cargo run --release --features graphics --example material_smoke`:
  native 256-square image comparisons independently check albedo, flat/smooth/unlit,
  specular, emission and all four noise channels. Seeded maps render identically on
  repeated uploads. Every changed channel affects more than 100 image bytes.
- `cargo run --release --features graphics --example geometry_gallery -- --smoke`:
  existing lit geometry and directional shadow rendering complete 120 ticks,
  240 presented frames and a resize on Metal without validation failures.
- `pnpm --filter emergent-ide test:materials` on localhost:3000:
  a temporary scene references the real material sphere twice. The inspector edits
  albedo/specular/emission/bump and binds the grain filter, saves only one instance,
  reconstructs inspection and changes real Rust-rendered pixels at tick zero. The
  second instance and shared entity asset stay unchanged. Its temporary scene and
  inspection worker are removed. Screenshot /tmp/emergent-material-editor.png was
  visually inspected; user-authored main-scene placements were preserved.

Regressions cover parameter bounds/unknown fields, serde round trips, seed changes,
2D XZ projection and offset mapping, noise dependency revisions, safe instance edits,
complete overrides/reset, non-renderable targets and missing noise assets.

Current limits: fixed 32-cubed RGBA8 material-noise resolution, basic Blinn-Phong
specular, no image/PBR textures, no bloom or emissive lighting, no displacement or
standalone shared material registry. Windows/Linux runtime rendering is unverified.
