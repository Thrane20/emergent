# Native terrain GPU pass profiling

2026-10-30; macOS 27.0.1 (26A434), arm64 Apple M3 Pro / Metal, wgpu 30.0.1,
Rust release profile. [Raw samples](routes.csv), [design](../../../design/terrain-performance-v1.md#gpu-profiling-increment),
[terrain plan](../../../plans/terrain.md#current-priority--terrain-performance-v1).

## What was implemented

- Optional device TIMESTAMP_QUERY support, with no profiling target/query/buffer
  allocation until `profile_lit_frame()` is called. Unsupported offscreen devices
  return `None`; windowed renderers reject this diagnostic call.
- Per-main-pass GPU timing and summed directional/point shadow-pass timing.
  Main includes sky/ray/mesh, and excludes the separate particle pass. Shadow
  counters cover the renderer's full 13-pass limit: one directional + two × six
  point faces. No queries are issued by normal presentation or colour capture.
- Separate CPU preparation, render-command encoding and completion durations.
  Completion includes sampling submission/wait, subsequent resolve scheduling,
  query readback and a second bounded wait; it overlaps GPU execution. It is not
  additive to GPU time. Diagnostic target/query buffers are reused until resize.
- `geometry_buffer_bytes(id)` validates the existing renderer-scoped handle and
  returns exact uploaded vertex/index buffer lengths. Stale/foreign handles return
  `None`. It excludes material textures/uniforms and driver allocation overhead.

This is an explicit blocking diagnostic API, not an asynchronous in-game overlay.
World/renderer SlotMap ownership, saved assets and the density function are unchanged.

## Metal counter correctness

Resolving in the same submission initially produced reversed counters on this
machine (example 869097856288750 → 869097855044625). These were rejected rather
than interpreted as a duration. The pinned Metal backend uses render-pass counter
attachments. [wgpu's timestamp example](https://wgpu.rs/doc/wgpu_examples/timestamp_queries/index.html)
documents query/resolve buffers and adapter timestamp periods;
[upstream issue 9414](https://github.com/gfx-rs/wgpu/issues/9414) reports incomplete
Metal resolves when committed before host-observed sampling completion.

The diagnostic path now waits for sampling completion before encoding/submitting
resolve. Each wait has a 10s timeout; mapping errors unmap the reusable buffer.
No backend fork or unsafe code was added. The final run produced all 896 expected
finite, ordered camera-route samples, plus warmups and shadow/resize checks. Zero,
reversed or invalid-period samples fail explicitly; none are discarded or replaced
with invented durations. This is local evidence, not proof for other drivers.

## Workload

960×540, perspective 45°, near 0.1m/far 30km. Fixed seed 42. Each workload first
captures an image, warms 32 samples around the route, then records 128 evenly
spaced camera angles for one orbit. The camera target is fixed and its height
varies along the orbit. These are camera inspection routes, not walking, chunk
crossing, teleport or simulation acceptance tests.

- `ray-1km`: default Hilly 1024m field, 650m camera radius.
- `ray-4km`: 4096m Hilly field, 1950m radius.
- `cavern`: 1024m CaveCavern field, cave radius 16m; camera orbits at radius 20m
  inside the chamber at y=30m.
- `ray-eight-regions`: eight translated 1024m fields in a 4×2 arrangement, 1300m
  camera radius; six to eight ray regions submitted. Repeated patches do not
  establish seamless global generation or streaming.
- Heightfields: default v2 Hilly recipes at width 1024m/relief 120m, resolutions
  128/512/1024, one ordinary white mesh without lights or shadows.

Ray and heightfield surfaces, material algorithms and topology differ. The table
compares their current rendering costs, not equivalent output or quality. Ray
routes also have no lights/shadows; their shading remains the procedural ray shader.
One region draws one full-screen triangle within its scissor; the heightfield
triangle counts below describe actual uploaded surface geometry.

The final measured run was sequential after owned validation/native presentation
processes exited. Other local applications were not controlled. It contains no
colour-pixel readback, PNG, browser transport, window presentation or simulation.
GPU timestamps measure pass execution; the serial wall time includes query waits.

## Results

Milliseconds, nearest-rank p50/p95/p99 over 128 samples (indices 63/121/126):

| Workload | Main GPU p50 / p95 / p99 | Serial wall p50 / p95 / p99 |
| --- | --- | --- |
| Ray 1km | 12.534 / 16.727 / 20.768 | 13.330 / 17.746 / 21.754 |
| Ray 4km | 14.418 / 18.137 / 19.954 | 15.504 / 19.051 / 20.948 |
| Cavern | 1.737 / 2.750 / 3.250 | 2.474 / 3.314 / 3.935 |
| Eight ray regions | 16.059 / 20.253 / 22.626 | 16.896 / 21.222 / 23.401 |
| Heightfield 128 | 0.063 / 0.075 / 0.127 | 0.511 / 0.677 / 1.084 |
| Heightfield 512 | 0.461 / 0.511 / 0.764 | 0.928 / 1.293 / 1.500 |
| Heightfield 1024 | 1.178 / 1.551 / 1.720 | 1.776 / 2.131 / 2.372 |

These distributions describe different camera views, plus timing variability,
not repeated measurements of one fixed frame or statistically established tail
latency. Do not convert them into an accepted gameplay frame-rate guarantee.

| Heightfield | Triangles | Uploaded vertex/index bytes | MiB |
| --- | ---: | ---: | ---: |
| 128 | 32,258 | 1,042,456 | 0.994 |
| 512 | 522,242 | 16,752,664 | 15.977 |
| 1024 | 2,093,058 | 67,059,736 | 63.953 |

The profiler itself retains 2,080,768 counted bytes at this viewport: a 2,073,600
byte RGBA8 target plus two 3,584-byte timestamp buffers. Query sets, existing depth/
shadow targets, materials, CPU samples, upload staging, driver and allocator costs
are excluded. Zero mesh bytes for ray fields means no uploaded surface mesh; it
does not mean zero GPU memory. Complete peak/resident-memory acceptance remains open.

## Interpretation and next work

CPU preparation and encoding medians are tens of microseconds on these fixtures;
ray-route GPU costs are substantially larger. Inspecting fields at grazing angles
and multiple regions is therefore a stronger current concern than triangle count
alone on this machine. Dense heightfields still grow steeply in geometry memory,
so chunk visibility and LOD remain required for the proposed large world.

Next run a bounded **meshed-density comparison using the same authoritative ray
field**. Measure extraction time/bytes, GPU cost, cave/overhang surface error and
collision suitability before choosing a mesh/cache strategy. This also informs
the still-missing collision system. No extraction method or library is selected.
Keep heightfield shared-lattice chunk LOD next in the large-world track, followed
by bounded streaming. Gameplay/simulation/presentation and full memory budgets
must be established before closing any continuous-world gate.

## Reproduce and validation

```sh
cargo run --release --example terrain_gpu_profile --features graphics -- /tmp/terrain-gpu-profile
```

The example checks unchanged before/after captures for every workload, live/stale
mesh handles, timestamp reuse, 13/6/1/0 shadow-pass transitions and 320×180 →
960×540 resize. Unsupported-feature selection, timestamp-period conversion and
invalid counters have unit coverage; a second unsupported physical adapter was
not available. Normal rendering still works without calling the profiler.

- `cargo test -p emergent --all-features --release`: **142 pass** (103 unit,
  25 authored-scene, 2 ECS mapping, 9 physics, 3 units), plus doc tests.
- `cargo test --workspace --all-features`: engine passes; the known first-project
  bounce assertion still fails at 3.340048m (expected 4 ± 0.2m). Not a green workspace.
- `cargo fmt --all --check`, workspace all-target all-feature Clippy with warnings
  denied and all-feature rustdoc pass.
- Native legacy ray and biome smokes pass: mesh depth occlusion, orthographic,
  inside cameras, transforms/handles, all eight biomes, overhangs, sampled CPU/GPU
  coverage and 4km capture.
- First Project native Run smoke passes on Metal: 120 ticks / 238 presented frames,
  then exits. This checks presentation compatibility, not a gameplay frame budget.
- No IDE code changed in this increment; browser/IDE suite not rerun. Previous
  [preview workflow evidence](../README.md) remains separate. No test server was
  started and user port 3000 was not used. Windows/Linux remain untested.
