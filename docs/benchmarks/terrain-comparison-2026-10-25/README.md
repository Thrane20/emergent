# Terrain preparation comparison — 25 October 2026

Measured on Apple M3 Pro, macOS 27.0.1, Rust 1.99.1 stable aarch64-apple-darwin,
Cargo `release`. Native CPU preparation only; no frame-time or GPU claim.

Recipe: [terrain.toml](terrain.toml), seed 42, mountainous, three main peaks,
100 m square, 38 m relief, Perlin frequency 0.65 / four octaves, roughness 0.35.
Original noise and diamond-square disable warp/ridges/erosion. Enhanced uses
Perlin, warp 0.25, ridge strength 0.5 and 16 thermal erosion passes.

For each resolution, one warm-up then 20 samples; each sample launches the built
CLI once and generates the three recipes sequentially. Timings come from the
Rust generation stage, excluding process launch, output encoding, Cargo, GLB
conversion, HTTP and rendering. Order is fixed, not randomized. P95 is nearest
rank, sample 19 of 20. Raw samples and height/slope diagnostics are in
[results.json](results.json).

| Samples per side | Recipe | Median ms | P95 ms | Height-field bytes |
| --- | --- | ---: | ---: | ---: |
| 128 | Original noise | 1.363 | 1.526 | 65,536 |
| 128 | Diamond-square | 0.554 | 0.589 | 65,536 |
| 128 | Enhanced pipeline | 3.949 | 4.260 | 65,536 |
| 256 | Original noise | 5.277 | 5.792 | 262,144 |
| 256 | Diamond-square | 2.127 | 2.263 | 262,144 |
| 256 | Enhanced pipeline | 15.624 | 16.278 | 262,144 |

Diamond-square was cheaper for these recipes; this does not establish a general
quality/performance winner. Enhanced does more work and was correspondingly more
expensive. Generation stays outside the game loop. Both outputs have identical
sample count; the IDE also reports exact GLB output bytes and its separately
measured preparation time. These sizes exclude working buffers and peak process
or GPU memory. Diamond-square adds a `(resolution+1)^2` float grid; erosion adds
one resolution-squared float working array. Browser comparisons retain three GLBs.

Slope diagnostics are absolute neighbour height differences divided by world
sample spacing, with P95 across horizontal and vertical edges. They help identify
sharp transitions but are not a visual quality score: enhanced changes ridge
coverage and cannot be compared as if it were only a smoothing pass.

Visual inspection in native Chrome confirmed the separated baseline mountains,
diamond-square variation and the enhanced connecting ridge with relaxed peak
tips. The comparison preserves sun and actual absolute initial camera radius and
target. Cards orbit independently and stack below 1100 px. Generation is still
height-field based: straight connecting ridges and grid-aligned diamond-square
features can remain visible; hydraulic drainage, erosion channels and seamless
chunk edges are not implemented. Perlin remains the default, with diamond-square
as an alternative rather than an automatic replacement.

Reproduce a single native sample from repository root:

```sh
cargo run --release --example terrain_builder -- --settings docs/benchmarks/terrain-comparison-2026-10-25/terrain.toml --compare > /tmp/terrain-comparison.etf
```

ETF1 is a little-endian binary tool protocol: ASCII `ETF1`, u32 field count,
then for each recipe u64 elapsed nanoseconds, u32 resolution and resolution² f32
world-space heights. Decode with `decodeTerrainFields` in the IDE terrain helper.
For 256 samples, copy the recipe to a temporary file and change `resolution`.

Validation: default/workspace and release/all-feature Rust tests, strict Clippy,
rustdoc and formatting; IDE tests, lint, type checking and production build;
`EMERGENT_IDE_BASE_URL=http://localhost:3001 pnpm --filter emergent-ide test:terrain`
against a temporary production server. Browser smoke covers all presets, both
base generators, deterministic output, three-way comparison/metrics, matched
camera framing, visible sunlight/ambient fill, orbit/zoom/reset, failure recovery
and narrow layout. No project files or paid provider calls. Native evidence is
macOS only; Windows/Linux execution remains unverified.


## Version 2 mountain massifs

Default mountain shaping now uses irregular multi-summit groups, broad shoulders
and saddles. Explicit version 1 and the original comparison baselines are retained.
The earlier table above measures version 1; it is not a version-2 timing claim.

A matched release run with the same 128×128 recipe, three groups, seed 42 and
16 erosion passes measured version-1 enhanced median 3.887 ms
(P95 4.073) and version-2 enhanced median 4.518 ms
(P95 4.575). Twenty samples each after one warm-up, alternating order,
without concurrent test/build jobs. Raw samples: [massifs-v2.json](massifs-v2.json).
Additional shaping costs preparation time; no frame-time benefit is claimed.

Native Chrome comparison visually confirmed several uneven summits and saddles
within each group, broader footprints and retained inter-group ridges. Regression
checks cover multiple substantial maxima for seeds 42/7/99, isolated group counts,
ridge contributions, deterministic seeds, legacy version-1 heights, bounds and
mesh normals/winding. The real browser smoke, Rust workspace/default and release
all-feature suites, strict Clippy/rustdoc/formatting, 48 IDE tests/lint/types/build
passed. No project files were changed.
