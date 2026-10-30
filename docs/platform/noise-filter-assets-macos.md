# Noise filter assets — reference validation

Date: 2026-10-24. Apple M3 Pro, macOS, Rust/Cargo 1.99.1. Sampling is CPU/headless;
this slice does not require a GPU or alter the inspection/game frame loop.

- Rust workspace debug and release/all-feature suites pass: 56 engine unit tests,
  nine authored-scene tests, two ECS tests, three project tests and one project
  contract test. New tests cover all algorithms, deterministic repeats, changed
  seeds, version-1 golden samples, 0..1 bounds, complete 3D volume indexing and
  slice differences, nonfinite inputs and evaluation/resolution/version limits.
- Rust formatting, Clippy with warnings denied, rustdoc with warnings denied and
  the no-default-features check pass. Added dependency: noise 0.9.0 pinned without
  default features; its four package additions include rand/rand_core/rand_xorshift.
  The prepared Worley callback stays local to the generator; no competing entity
  ownership store or unsafe code was introduced.
- IDE service tests (31), TypeScript, lint and production build pass. Tests cover
  template category/scene exclusion, bounded settings, metadata/comment preservation,
  malformed binary dimensions/samples and dense 3D transport.
- `pnpm test:noise` uses installed headless Chrome against the running IDE, creates
  a uniquely named filter via Filters / Noise function, and invokes the real Rust
  generator. Identical inputs reproduce identical grayscale pixels; seed and
  algorithm changes alter the image. Every RGB channel matches, alpha is opaque,
  and pixels vary. A 32-cubed field contains 32,768 samples; changing its Z slice
  changes the preview.
- Generation leaves authored source untouched. Save settings persists seed,
  algorithm and 3D mode, which survive a page reload. Invalid inputs preserve the
  last valid preview; an external source edit rejects the stale save without
  overwriting it. Raw noise TOML rejects an invalid frequency without publication,
  then saves a valid candidate through the Rust validator. Out-of-scope preview
  requests are rejected. Smoke assets are
  removed afterward, and no paid AI/provider calls are made.
- Existing isolated production `test:ui` passes for statements, AI settings, mocked
  generation, GLB preview/save, server restart and project selection. Browser
  screenshot inspection verifies the filter inspector, grayscale field, legend,
  sample/range labels and volume slice slider.

The development server initially retained the old compiled TOML validator; it was
restarted on port 3000 to load the new filter contract. The reference cube test
also assumed zero placement despite authored scene edits: it now checks rotation
relative to the initial authored transform and preserves its translation/scale.
The scene's placements/hierarchy were left intact.

Limits: definition schema 1 and algorithm version 1; 512² 2D / 128³ 3D, 1..8 octaves,
2,097,152 octave evaluations, 256 KiB source and at most 8 MiB sample payload.
Generation/validation has one admitted request at a time and a 120-second child
process timeout. Field buffers are transient and not exported/baked by the IDE.
Terrain/material consumers, filter graphs, value noise/domain warp, Windows/Linux
runtime testing and cross-platform bit-identical reproducibility remain open.
Correctness checks do not establish a real-time sampling throughput budget.
