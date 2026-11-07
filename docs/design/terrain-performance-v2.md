# Terrain Performance v2 — distance-aware tracing, mesh culling and heightfield LOD

Agreed 2026-11-04: improve large-terrain cost for **both** terrain kinds before the
meshed-density/collision comparison. Ray-marched fields stop doing sub-pixel work
far from the camera; heightfields stop drawing triangles the camera cannot see.
Continuous-world chunking, streaming and gameplay budgets remain the later stages
in the [terrain plan](../plans/terrain.md).

## Ownership and data flow

No identity or storage model changes. Ray regions remain renderer-owned
`RaymarchedTerrainId` SlotMap entries; meshes remain `GeometryId` entries. Saved
recipes, terrain samples, materials and collision are untouched. Everything here is
render-time behaviour with a diagnostic bypass.

## Ray marching (`src/shaders/raymarched_terrain.wgsl`)

1. **Pixel-footprint hit tolerance (v2 fields).** The ray spread per pixel comes from
   screen-space derivatives of the ray direction (computed before any `discard`). A
   ray stops when the *unscaled* field value is within half a pixel's footprint at
   that distance. The stepping radius is divided by the field's Lipschitz bound
   (about 9 on default hills); the tolerance undoes that division so the hit really
   is within half a pixel. Legacy v1 fixtures keep their fixed tolerance.
2. **Over-relaxed sphere tracing** (Keinert et al. 2014): step 1.6× the safe radius;
   when consecutive spheres stop overlapping, step back and continue with plain
   steps. The box exit is only accepted from a verified (non-failed) position, so
   an overshoot through a thin surface past the box is stepped back first.
3. **Tetrahedral normals:** four field evaluations instead of six.
4. **Long steps with a safe finish (2026-11-05).** Near rays crept because every step
   is the field value divided by the global Lipschitz bound (≈9): even a steep ray
   closes only about a tenth of the gap per step. Rays that start outside the solid
   now step at least one pixel footprint, or half the *unscaled* height gap. The
   unscaled gap comes from `biome()` and applies only where the height term is the
   active surface; cave-carving and box terms are already distances and are used as
   they are (multiplying them by the bound distorted cave mouths in testing). When a
   step lands inside the solid, the ray returns to its last outside point and finishes
   with plain safe sphere tracing. Bisection was tried first and rejected: a long step
   across a rim can contain several crossings, and bisection then picks a surface
   behind the rim. Inside-solid cameras keep plain tracing.

   Limit: a long step that passes entirely through a solid feature thinner than the
   step, without landing inside it, misses that feature. The cavern route and biome
   overhang smoke are the regression checks.

`RaymarchedRenderOptions::distance_adaptive` (default on) bypasses 1, 2 and 4 for exact
comparisons. The CPU field stays authoritative and full-detail; rendering may differ
from it by at most about half a pixel at the hit.

Rejected after measurement: a one-pixel tolerance (≈15% faster but 10% of pixels
changed at 4km) and distance-scaled normal sampling (visible shading softening).
A rock-detail fade was not built: at 960×540 the 3D rock features stay above pixel
size until roughly 2.5km, so it would not help the 1km default.

## Mesh culling (`src/graphics.rs`)

- **Whole-mesh frustum culling** for every non-overlay instance in the main pass and
  in each shadow pass (directional and point faces, against that pass's light
  matrix). Local bounds are computed once at upload; the eight-corner clip-plane test
  is shared with the ray-region culling (`box_outside_clip`) and keeps non-finite
  input visible. It rejects only geometry the rasterizer would clip anyway, so output
  is pixel-identical.
- **Spatial clusters for large meshes:** at upload, base-material triangles of meshes
  with at least 16,384 triangles are counting-sorted into a grid over the two widest
  centroid axes (≈8,192 triangles per cell, at most 64×64 cells), so each cell is a
  contiguous index range with its own bounds. Draws skip invisible clusters and merge
  adjacent visible ones. Vertices, material bounds and section ranges are unchanged;
  only base triangle order changes. Per frame: eight corner transforms per cluster
  per pass, with no allocation.
- `set_mesh_culling(false)` bypasses both for comparisons; `submitted_triangles()`
  reports main-pass triangles after culling.
- Metal writes no timestamps for a pass without draws, so the GPU profiler now
  treats a shadow pass whose casters were all culled as zero time.

## Heightfield distance LOD (`src/graphics.rs`)

- **Grid recognition:** at upload, a mesh whose indices are exactly the row-major n×n
  grid that `TerrainField::mesh` emits (checked index by index), with no material
  sections and at least 16,384 triangles, is treated as a terrain grid. No API, asset
  or scene change; anything else falls back to the generic clusters above.
- **Tiles and levels:** the grid is split into 64×64-cell tiles (these are its culling
  clusters). Each tile gets coarse index sets at steps 2, 4, 8 and 16 over the same
  vertices, appended after the base indices. Every level keeps the tile border at full
  resolution: coarse interior quads plus four zipper strips join the interior to the
  border. Neighbouring tiles at any mix of levels therefore share identical edges and
  cannot crack, without neighbour masks. Edge tiles too small for a step reuse their
  finest available level. Normals stay the full-resolution vertex normals.
- **Selection:** per tile and frame, the coarsest level whose cell edge spans at most
  four pixels at the tile's nearest distance (bounding-sphere estimate, model scale
  included). Shadow passes use the camera's choice so shadows match the visible
  surface. Orthographic views draw full detail. Stateless: no hysteresis yet, so a
  tile sitting on a level boundary can switch back and forth as the camera moves.
- Cost: +14% index memory at 1024 (coarse sets); per-frame work is one distance and
  `log2` per tile. `set_mesh_lod(false)` bypasses it for comparisons.

## Validation

Unit tests: cluster partition (every triangle exactly once, winding intact, bounds
contain vertices, contiguous ranges), small meshes untouched, box rejection (behind,
beside, beyond far plane, straddling near plane, NaN), grid detection, and for every
LOD level of full and partial tiles: exact area coverage, no overlaps, consistent
winding and a boundary identical to the full-resolution tile border. The GPU benchmark
adds player-height routes, asserts culled and unculled mesh captures are identical,
and reports adaptive-versus-exact and LOD-versus-full (lit, shadowed) pixel differences
with saved images. Measurements:
[v2 benchmark](../benchmarks/terrain-performance/v2/README.md).

## Next

- **LOD hysteresis and popping:** measure visible popping on a moving route before
  adding hysteresis or geomorphing.
- **Shadow-pass timing** with culling/LOD on a lit route.
- **Opaque-depth termination** for rays behind meshes (unchanged open item).
- Then the continuous-world stages: chunked worlds and streaming beyond one 1024 patch.
