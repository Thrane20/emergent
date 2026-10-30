# Terrain resolutions through 1024

Terrain grids now support 32, 64, 128, 256, 512 and 1024 samples per side. Existing defaults and saved assets stay unchanged. The terrain subsystem still owns one dense field and mesh per asset, with existing SlotMap handles for runtime geometry. Existing terrains change only after explicit generation and saving; this does not add streaming or LOD.

The 1024 grid holds 1,048,576 heights (4 MiB) and 2,093,058 triangles. Preparation happens outside the game loop. Terrain uses prepared point noise sampling under its own bounded resolution, octave and erosion contract; general noise-field quotas remain unchanged. Height and paint sidecars are individually bounded to 32 MiB, save JSON to 64 MiB, and binary generator output to the requested field count. Three-way comparison stays capped at 256 to retain its existing response and memory envelope.

Preview GLBs pack directly into their final buffer. Heightmap painting redraws the affected rectangle, without per-pixel temporary arrays; 3D rebuilding still happens at stroke release. Single-terrain generation remains serialized with a bounded queue. Render and static Rapier collision loading consume the same saved field.

## Measurements

Apple M3 Pro, macOS, optimized Rust builds, three trials per resolution. Workload: island, seed 190, size 270 m, height 56 m, frequency 0.8, five octaves, roughness 0.5, default domain warp/ridges and 16 erosion passes. These measure preparation, not frame rate.

| Resolution | Generation | Native mesh preparation | Triangles | Preview GLB |
| --- | --- | --- | --- | --- |
| 256 | 14.1–14.4 ms | 0.6–0.8 ms | 130,050 | 3.7 MiB |
| 512 | 56.1–56.8 ms | 2.5–3.4 ms | 522,242 | 15.0 MiB |
| 1024 | 215.5–218.4 ms | 9.6–14.0 ms | 2,093,058 | 60.0 MiB |

The pre-change 256 baseline was 16.8–35.1 ms generation and 0.6–1.1 ms mesh preparation. Short-run variation prevents a general speedup claim. `/usr/bin/time -l` reported maximum resident memory of approximately 78 MiB for the isolated 1024 Rust benchmark; this excludes the editor, saved-file parsing, renderer and physics.

A separate Node/tsx ramp-heightfield harness measured direct GLB packing at 43–44 ms for 1024. Its process reached approximately 320 MiB RSS across repeated allocations before garbage collection. Compared with the previous 256 harness, eliminating a buffer copy lowered observed RSS but did not consistently lower elapsed time. Neither harness establishes a scene frame-time budget.

At 270 m width, sample spacing is 1.059 m at 256, 0.528 m at 512 and 0.264 m at 1024. Increasing resolution regenerates the procedural recipe; it does not automatically upsample existing sculpted terrain. A 1024 terrain has 16 times as many vertices as a 256 terrain, so use it where the additional detail is useful.

Validation includes maximum-grid mesh/index regression tests, general-noise quota regression tests, the IDE suite and production build, native four-material rendering at 1024, and a temporary-project browser smoke for generation, sculpting, painted saving, reopening and scene placement.
