# Terrain Performance v2 measurements

2026-11-05; Apple M3 Pro / Metal, release profile, 960×540, perspective 45°,
near 0.1m / far 30km, seed 42. Same harness and routes as the
[v1 GPU record](../gpu/README.md), plus three player-height routes: the eye sits 3m
above the surface and turns in place, looking slightly down. Raw samples:
[after](routes.csv), [before](routes-baseline.csv). [Design](../../../design/terrain-performance-v2.md).

Main-pass GPU milliseconds, nearest-rank p50 / p95 over 128 samples. "Before" was
captured with the same benchmark binary immediately before any v2 renderer change.

| Workload | Before p50 / p95 | After p50 / p95 | Change at p50 |
| --- | --- | --- | --- |
| Ray 1km orbit | 11.18 / 14.21 | 2.10 / 2.73 | −81% |
| Ray 4km orbit | 12.12 / 15.11 | 2.06 / 2.57 | −83% |
| Cavern | 1.65 / 2.77 | 1.61 / 2.32 | −2% |
| Eight ray regions | 13.86 / 20.74 | 2.58 / 3.30 | −81% |
| Ray 1km, player height | 14.52 / 22.30 | 3.56 / 5.05 | −75% |
| Ray 4km, player height | 15.32 / 21.27 | 4.03 / 5.59 | −74% |
| Heightfield 512 orbit | 0.49 / 0.53 | 0.40 / 0.45 | −18% |
| Heightfield 1024 orbit | 1.31 / 1.43 | 0.50 / 1.22 | −62% |
| Heightfield 1024, player height | 1.06 / 1.23 | 0.38 / 0.55 | −64% |

Heightfield 128 is unchanged (0.08ms). Triangles submitted at p50 (max) of 2,093,058
for heightfield 1024: 375,834 (424,976) orbiting, 316,784 (405,952) at player height.

### Where the gains come from

Measured as the work landed (p50, 1km orbit / 1km player height):

| Step | Ray 1km orbit | Ray 1km player height |
| --- | --- | --- |
| Before | 11.18 | 14.52 |
| Half-pixel tolerance, over-relaxation, four-tap normals | 5.96 | 10.65 |
| + long steps (1 pixel footprint or half the unscaled height gap), safe finish | 2.10 | 3.56 |

| Step | Heightfield 1024 orbit | Heightfield 1024 player height |
| --- | --- | --- |
| Before | 1.31 | 1.06 |
| Frustum culling + clusters | 1.36 | 0.51 |
| + distance LOD | 0.50 | 0.38 |

## Image differences

Culled and unculled heightfield captures are byte-identical (asserted). LOD is compared
with full detail under a shadow-casting directional light; ray captures with
`distance_adaptive: false`. Pixels differing by more than 8/255 in any channel:

| Route | Pixels differing |
| --- | --- |
| Ray 1km orbit | 0.72% |
| Ray 4km orbit | 2.71% |
| Eight regions | 1.71% |
| Cavern | 0.007% |
| Ray player height (1km and 4km) | 0.11% |
| Heightfield 512 / 1024 orbit, LOD vs full | 0.006% / 0.020% |
| Heightfield 1024 player height, LOD vs full | 0.000% |

Inspected side by side: ray 4km, cavern, ray player height and lit heightfield 1024 —
visually indistinguishable. During tuning, an unscaled gap that also inflated the cave
terms produced a visibly distorted cave mouth (1.7% of cavern pixels); the fix limits
the unscaled gap to the height term. That case is why the cavern route is the
regression check for long steps.

## Memory

Heightfield 1024 uploaded vertex/index bytes rise from 75,448,344 to 86,209,560
(+14%) for the coarse index sets. Vertices are shared; no level adds vertices.

## Limits

Serial offscreen rendering without lights/shadows in the timed loop, colour readback or
presentation; not a gameplay frame budget. One machine. A later full run was discarded
because the IDE, Chrome and a running game were using the GPU (heightfield 128 read
0.70ms instead of 0.08ms); the recorded run was taken on a quiet machine with the same
renderer code. Shadow-pass culling/LOD timing was not benchmarked separately. Long ray
steps can still pass through solid features thinner than the step when the ray never
lands inside them (see design).

## Validation run

- `cargo test -p emergent --all-features`: 317 pass, 0 fail (new: cluster partition, grid
  LOD tiling/border/winding, grid detection, box rejection, small-mesh passthrough).
- `cargo clippy -p emergent --all-targets --all-features -- -D warnings`: clean.
  `cargo fmt --check` reports only another in-progress change (skinning in
  `src/graphics.rs`, `tests/skinned_model.rs`), not these files' new code.
- Native smokes pass: `raymarched_terrain_smoke` (depth/occlusion, orthographic,
  inside-solid), `raymarched_biome_smoke` (overhang, CPU/GPU coverage, all biomes,
  4km), `terrain_performance` (bounds-culling equality), `terrain_gpu_profile`.

## In-app check (2026-11-06)

Production IDE build on port 3001 with disposable projects:

- `scripts/raymarched-terrain-smoke.ts` passes (builder live preview, library, scene
  placement, native orbit/selection/transform, native Run), run from a temporary copy
  without one stale assertion: the "visual only" inspector note it expects was removed
  from the IDE earlier.
- `scripts/terrain-resolution-smoke.ts` passes (512/1024 generation, sculpt, painted save,
  native validation, scene placement).
- Manual pass: a template-derived project with an unpainted 1024 heightfield in the
  shadows scene. Native scene preview orbited and zoomed out: smooth, lit, no holes or
  tile cracks; selection works. Offscreen at 1km/5km/30km, pixels are identical with
  culling and LOD on or off.
- Found and fixed: a heightfield without an authored material was uploaded flat-shaded,
  which at 1024 needs a 301MB vertex buffer (over the 268MB device limit) and panicked
  the preview. This predated v2. Scene loading now gives material-less terrain a smooth
  default material, as it already did for imported models (`src/scene.rs`, regression
  test in `tests/authored_scene.rs`).
- `scripts/terrain-scene-inspection-smoke.ts` is stale (it never presses the scene
  viewer's Load button), so it was replaced by the manual pass above.

```sh
cargo run --release --example terrain_gpu_profile --features graphics -- /tmp/terrain-gpu-profile
```
