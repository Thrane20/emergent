# Authored scene loading — local validation

Date: 2026-10-24. Rust 1.99.1 / Cargo 1.99.1, stable aarch64 Apple Darwin.

## Observed results

- `cargo test --workspace` and `cargo test --workspace --release --all-features`
  pass. Loader regression coverage includes shared asset instances, fresh runtime
  handles, inherited transforms, subtree/stale-handle safety, file/value/schema
  errors, duplicate IDs, missing parents/assets, cycles and path escapes.
- Additional focused loader tests pass for every primitive/light type, the
  eight-light bound, subdivision limits and overflowing composed transforms.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all -- --check`, and `cargo doc --no-deps --all-features` pass.
- `cargo run -p emergent-first-project -- --headless` completes the bounded
  reference scenario. Cube rotation remains covered by project tests.
- `cargo run --release -p emergent-first-project -- --smoke` passes on Apple M3
  Pro / Metal / Bgra8UnormSrgb: six uploaded geometry resources, 120 ticks, 241
  presented frames, resize observed and clean shutdown. Native and offscreen
  extraction now consume the same loaded primitive/light content.
- IDE service tests (19), TypeScript, lint and production build pass. The existing
  production browser smoke passes for Statements, settings, fixture GLB preview,
  saving, server restart and selection.
- Against the user's existing `localhost:3000`, `pnpm test:authored` creates a
  uniquely named temporary scene, uses Add Asset to include a cube, loads a real
  Rust capture, changes translation/size/colour in its asset, and observes a changed
  rendered image. A negative size reports the asset path and keeps the previous
  frame. Only smoke-owned assets are removed afterward.
- `pnpm test:scene` also passes against the existing IDE: native PNG display and
  failed-reload retention.

## Hosted Object Creator acceptance

The existing real saved `assets/models/a-robot-arm/model.glb` loaded successfully
in Chrome's Model Viewer. After reloading the IDE, selecting `object.asset.json`
reopened the same textured red robot arm. Its descriptor records fal request
`01a168ac-3ac2-7cd0-b232-990d3a39e4c1` and the earlier save. This check submitted
no paid generation and used the saved project artifact, not a provider URL. The
container is GLB 2 with a declared length matching its 1,537,512-byte file.

This closes OC-1's real-result compatibility/reopen gate. It does not validate
native Rust GLB importing or guarantee future provider output quality.

## Limits

These are correctness/integration checks, not a frame-time benchmark or shadow
quality assessment. Windows/Linux runtime validation remains pending. Schema-1
loading is limited to explicit primitive/light/empty-node instances, without
nested compositions, instance overrides, authored behaviour ports or a general
content-type registry. Run uses the manifest entry scene; Load selects a scene.
