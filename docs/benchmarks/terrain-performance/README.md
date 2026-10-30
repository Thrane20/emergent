# Terrain Performance v1 — first increment

Follow-up: [GPU pass profiling and native camera routes](gpu/README.md) now add
isolated pass timestamps and mesh-buffer counts. The capture measurements below
retain their original scope; the follow-up is a separate serial workload.

Measured and validated 2026-10-30, macOS 27.0.1 (26A434), arm64 Apple M3 Pro,
wgpu 30.0.1 / Metal, Rust release profile. [Design](../../design/terrain-performance-v1.md)
and [remaining gates](../../plans/terrain.md#current-priority--terrain-performance-v1).

## Workload and method

`examples/terrain_performance.rs` uses one 1024m region, seed 42, default Hilly
recipe; the cavern view explicitly uses CaveCavern with radius 16m. Five fixed
cameras cover overview, grazing, cavern interior, small projected region and
offscreen terrain. Each view warms eight captures, then records 40 samples.
Percentiles use nearest rank; p99 is the maximum of these 40 samples. This is a
small local comparison, not a statistically established tail-latency guarantee.

The benchmark includes CPU preparation, target/readback allocation, submission,
GPU completion and pixel copy. PNG writing and reference comparisons are outside
the timed section. It is **not isolated GPU timing**, native Run frame timing,
browser delivery latency or a continuous-world acceptance workload. Other local
applications were not controlled. The final full and half runs were sequential,
after browser/native smoke processes had exited.

Raw microseconds: [before](before.csv), [culling only](culling-only.csv),
[after bounds + shader changes](after.csv), [half resolution](half.csv).
The before file was captured before renderer/shader changes. Culling-only numbers
include the dormant heatmap branch but precede biome simplification. Retain that
distinction when comparing results; these are sequential runs, not randomized trials.

## Results

Capture wall times, milliseconds:

| View | Before p50 / p95 / p99 | After p50 / p95 / p99 | Half p50 / p95 / p99 |
| --- | --- | --- | --- |
| Overview | 8.615 / 10.304 / 10.610 | 6.857 / 8.829 / 9.621 | 3.060 / 3.546 / 4.072 |
| Grazing | 20.824 / 26.192 / 27.378 | 15.927 / 19.745 / 23.870 | 5.904 / 6.766 / 8.611 |
| Cavern | 3.324 / 4.085 / 4.485 | 3.459 / 4.000 / 4.224 | 1.747 / 2.008 / 2.263 |
| Small | 1.582 / 1.999 / 2.520 | 1.313 / 1.566 / 1.899 | 1.011 / 1.143 / 1.303 |
| Offscreen | 0.501 / 0.689 / 1.165 | 0.556 / 0.680 / 1.066 | 0.378 / 0.440 / 0.896 |

Full is 960×540; Half is 480×270. The half benchmark does not include preview
upscaling/PNG costs. The worker includes upscaling in the displayed PNG timing.
The overview/grazing medians decreased about 20%/24% in this comparison. Cavern
median increased about 4%; offscreen median also increased. Do not claim universal
speedup. Culling alone did not show a reliable timing gain on the single-region
fixture; it removes known submitted work but does not replace shader optimization.

| Full-resolution view | Submitted regions | Summed scissor pixels / 518,400 |
| --- | --- | --- |
| Overview | 1 | 398,970 |
| Grazing / cavern | 1 | 518,400 |
| Small | 1 | 29,700 |
| Offscreen | 0 | 0 |

Scissor area counts candidate pixels, not fragments, successful hits or field
evaluations. Regions overlap and the shader already clips rays to local bounds.

## Correctness and workflow validation

- Projected bounds tests cover all six clip planes, camera-inside/near-plane
  fallback, perspective, orthographic and reflected/nonuniform transformed boxes.
  Interior sample projections remain inside the scissor.
- Every benchmark view compares culled versus full-screen captures. Visible
  views are byte-identical. Fully culled Metal sRGB clears can differ by 1/255 in
  a channel. Final shader captures versus the original reference differ by at
  most 1/255 per channel; the cavern is byte-identical. These checks cover these
  fixtures, not every valid recipe.
- `raymarched_terrain_smoke` passes legacy field/mesh depth occlusion, camera
  inside, orthographic, transformed region lifecycle and stale/foreign handles.
- `raymarched_biome_smoke` passes non-heightfield overhangs, 256 sampled CPU/GPU
  coverage queries, three CPU-reference mesh occlusion checks, native 4096m extent
  and all eight biome captures. Its cold capture timings are not benchmark data.
- Terrain worker quality unit test passes. Production step limits remain 256
  (legacy) / 1024 (biomes); neither quality nor heatmap changes a field recipe.
- Production-build Chrome smoke on owned IPv6 port 3001 passes Full → Half → Full
  exact view restoration, heatmap on/off restoration and unchanged worker PID;
  existing orbit/pan/zoom, cave preview, guarded save/load, scene placement,
  transform/reopen, mobile and native Run checks also pass. The disposable project,
  worker and test service are shut down. User port 3000 is not used for testing.

## Reproduce

```sh
cargo run --release --example terrain_performance --features graphics -- /tmp/terrain-perf-full
cargo run --release --example terrain_performance --features graphics -- /tmp/terrain-perf-half --half
cargo run --release --example terrain_performance --features graphics -- /tmp/terrain-perf-uncropped --no-culling
```

Add `--reference=/path/to/earlier-captures` for same-resolution image comparison
(maximum accepted channel difference 1/255). Runtime `RaymarchedRenderOptions`
and `raymarched_frame_stats()` are documented in the public Rust API.

## Checks and remaining limits

Engine all-feature release tests: **141 pass** (102 unit, 25 authored-scene,
2 ECS mapping, 9 physics, 3 units). IDE suite: **128 pass**. Typecheck, affected
ESLint, isolated Next production build, Rust formatting, workspace all-target
all-feature Clippy with warnings denied and rustdoc pass. The full workspace
test command still fails the existing first-project bounce-height assertion:
3.340048m versus 4 ± 0.2m; it does not establish a green workspace.

After the [GPU profiling follow-up](gpu/README.md), still open: native Run traversal distributions,
allocation/memory and browser delivery budgets, extreme/grazing recipe coverage,
multi-region performance, depth-aware stopping, mesh LOD and streaming. The heatmap
visualizes steps/exhaustion but does not aggregate GPU counters. Magenta exhaustion
is a diagnostic overlay at near depth, not a newly found terrain surface. Quality
resizing/upscaling and capture buffers allocate; steady-state game rendering is a
separate path. No Windows/Linux measurement or production frame budget is claimed.
