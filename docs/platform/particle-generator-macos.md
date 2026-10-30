# Particle engine and generator validation — 29 October 2026

Implemented in the reusable Rust engine, native first-project integration and IDE.
No third-party particle framework or new dependency was added. The approved
[design](../design/particle-generator.md) records the library assessment and scope.

## Verified behavior

- World-owned particle store with owner-scoped typed SlotMap handles, bounded
  dense pools, seeded emission, lifetime/force updates, restart, capacity handling,
  local/world transforms and stale/foreign-handle rejection.
- Versioned recipes and scene entity references; shared texture decoding, sprite
  grid validation, scene capacity checks and guarded save/update/copy/placement.
- Native instanced billboards with linear-light premultiplied sprites, texture
  alpha, flipbook frame selection, sorted alpha/additive composition, opaque
  depth occlusion, soft intersections and resize handling. Scene preparation
  preloads shared sprites; staging buffers and camera bindings are reused.
- Generator presets, layered emitter controls, orbit preview, pause/restart/step,
  original-image selection, native capture, save/place/update/reopen, and direct
  entry through both the main rail and Add Asset → Effects → Particle effect.
- Main-editor effects use an independent frozen two-second sample without
  advancing gameplay or physics. Actual Run uses the fixed-step simulation.
- Imported alpha video bakes into a 32-frame PNG and can be rendered natively.
  Source-alpha validation precedes padding, so letterboxing cannot qualify an
  opaque movie as smoke. Opaque smoke baking is rejected; additive-only baking
  remains available. Media probing/decoding is bounded and network protocols are
  disabled for local ffmpeg/ffprobe inputs.
- Durable fal jobs, restart/idempotency checks, uncertain-submission protection,
  transparent-matting schema, warning handling and download-host restrictions
  passed with a mock transport. No paid fal requests were made.

## Commands and outcomes

| Check | Outcome |
| --- | --- |
| `cargo test -p emergent --all-features` | Pass: 87 library tests plus integration suites, including 24 authored-scene tests |
| `cargo test -p emergent --release --all-features` | Pass |
| `cargo test -p emergent --no-default-features` | Pass; simulation remains usable headlessly |
| `cargo test --workspace --all-features` | Engine/integration suites pass; existing first-project bounce test fails |
| `cargo test --workspace --release --all-features` | Same existing bounce-height failure, measured peak 3.340048 m |
| `cargo clippy --workspace --all-features --all-targets -- -D warnings` | Pass |
| `cargo fmt --all --check` | Pass |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps` | Pass |
| `cargo run --release --all-features --example particle_render_smoke` | Pass: native texture alpha, frames, depth/sorting, soft fade, resize and capture workloads |
| IDE `pnpm test` | Pass: 93 tests |
| Targeted ESLint and TypeScript | Pass |
| `EMERGENT_NEXT_DIST_DIR=.next-particle-build pnpm build` | Pass: production app, including all particle routes |
| `pnpm test:particles` against that build | Real Chrome workflow and native Run/resize/120-tick teardown; temporary project only |

The browser test checks that valid recipe edits preserve the WebGL canvas,
invalid values disable saving, and updates retain the original asset identity.
It imports a synthetic soft-alpha clip, bakes and assigns the atlas, saves,
reopens through the main editor, renders native captures, and verifies that an
unrelated main scene remains byte-for-byte unchanged. It rejects cross-origin
writes and checks that no uncaught browser errors occur. Rapid navigation may
reject an in-flight PUT to an already closed inspection session with the exact
"Inspection is not running" response; the smoke permits only that teardown
response and fails on all other unexpected HTTP errors. Its service uses port 3001,
with an IPv6 listener because the existing VS Code helper occupies IPv4 3001;
the browser uses localhost. The owned service/browser and native Run shut down.

## Initial optimized measurements

Machine: Apple M3 Pro / Metal, 36 GiB RAM, macOS 27.0.1, rustc 1.99.1.
These are initial bounded workloads, not a frame-rate or scene-size guarantee.

Simulation: `cargo run --release --all-features --example particle_benchmark`,
three seeded emitters with fixed populations, 600 steps at 1/60 s. Reports CPU
simulation only; it excludes extraction, sorting, GPU work and overdraw.

| Live particles | CPU p50 | CPU p95 |
| --- | ---: | ---: |
| 1,000 | 0.0032 ms | 0.0035 ms |
| 10,000 | 0.0344 ms | 0.0433 ms |

Native capture: `particle_render_smoke`, 512×512, 20 warmed captures per workload,
three emitters sharing the procedural sprite, box-distributed particles, size
0.15–0.35 m. Timings include one simulation step, extraction/sort/upload, draw,
GPU wait and synchronous RGBA readback. Zero particles is the same renderer's
empty-scene baseline. Small timing differences around this baseline are noise,
not a claim that adding particles improves performance.

| Live particles | Capture wall p50 | Capture wall p95 |
| --- | ---: | ---: |
| 0 | 0.700 ms | 1.004 ms |
| 1,000 | 0.899 ms | 1.312 ms |
| 10,000 | 1.955 ms | 2.334 ms |

Percentiles use nearest rank. Dedicated GPU timestamp profiling, an allocation
profiler, larger transparent-overdraw scenes and many distinct atlases were not
measured. Sorted texture changes can split draw batches; these workloads share
one sprite and should not be generalized to arbitrary texture counts.

## Evidence and limits

Screenshots in [the particle evidence directory](../benchmarks/particles-2026-10-29/)
show the generator, a native flipbook capture and the selected effect in the main
editor. The synthetic clip exercises media/alpha mechanics, not generated art
quality. The normal scene remains at tick zero during authoring.

Live fal access, pricing/credit behavior, generated smoke/fire quality, temporal
matting and seamless loops remain unverified. Model versions and job settings are
recorded, but provider model revisions are unavailable. ffmpeg and ffprobe must
be installed on the IDE server. Source clips remain in the private IDE job
journal; atlas assets alone are sufficient for runtime playback.

The browser preview approximates blend ordering and lacks native depth fading;
use native capture for those checks. Base-level atlas sampling avoids cross-frame
mip bleed; distant-sprite aliasing is not yet addressed with per-cell mipmaps.
Authoring's fixed two-second sample can miss short one-shot bursts. GPU simulation,
collision, trails, mesh particles, volumetric fluids and timeline scrubbing are
deferred. Native validation covers macOS/Metal; Windows and Linux were not run.

`EngineError` now carries detailed particle errors and remains Clone/Eq; it is no
longer Copy. Existing workspace callers compile, but external callers relying on
Copy must clone or borrow errors. The existing bounce-height failure is outside
this change and remains unresolved. Full-tree whitespace checking also reports
an existing blank line at EOF in the user's main scene; focused changed-code
whitespace checks passed and that scene was left untouched.
