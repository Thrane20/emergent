# Terrain world — stage 1: seamless sampling and chunk preparation

Agreed 2026-11-07 as the first continuous-world increment
([terrain plan](../plans/terrain.md), stage 1). Headless only: no renderer, physics,
scene or IDE integration, and no performance claim.

## Scope

- A versioned, bounded `TerrainWorld` descriptor: seed, chunk counts per axis,
  cell size and world-coordinate noise shaping (height, frequency per km, octaves,
  roughness, domain warp). It is a new contract beside `TerrainSettings`; the
  existing 32–1024 patch format, saved samples and legacy recipes are untouched.
- `TerrainWorld::prepare_chunk(coord)` returns an owned `TerrainChunk`: 129 × 129
  heights (128 cells per side), matching normals and height bounds. Pure function of
  descriptor and coordinate, so generation order, threads and repeat calls agree.
- `TerrainChunk::mesh(color)` builds a chunk-local mesh (positions relative to the
  chunk origin, so f32 stays precise); `TerrainChunk::origin()` is its world offset.
- Materials (added 2026-11-07): `height_band_weights` applies the Terrain Builder's
  height-paint rule per sample, so shared border samples get identical weights.
  `Material::terrain_blend(layers, resolution, weights, offset)` builds an in-memory
  blend (no paint file); `offset` is the chunk's world x/z. The renderer wraps it by
  each layer's image period in f64 and adds it to mesh-local positions for triplanar
  projection, so the pattern continues across chunks without f32 precision loss far
  out. Noise-map layers are rejected: their masks are relative to each mesh's bounds.
  Each chunk still uploads its own material (texture array per chunk) — the stage-2
  sharing question.

Not in this increment: patch-local presets (islands, canyons, rivers, mountain
placement, erosion, diamond-square) until each has a world-space continuity
contract; chunk storage/lifecycle (stage 2); LOD (stage 3);
streaming (stage 4); collision and height queries (stage 5); saved world assets
and IDE authoring (stage 6).

## Ownership and data flow

Descriptor → requested `ChunkCoord` → `prepare_chunk` → owned `TerrainChunk`.
Preparation creates no entity, `GeometryId` or collider and touches no live store,
so it can later run on worker threads unchanged. Stage 2 adds a typed SlotMap of
chunk records that hold these results plus renderer/physics handles; coordinates
remain a lookup, never identity. Nothing here serializes a SlotMap key.

## Coordinates and continuity

- The world is centred on the origin: chunk x ranges over
  `-(chunks_x / 2) .. chunks_x - chunks_x / 2`, likewise z, so negative coordinates
  are normal. Chunks outside that range are rejected.
- Every sample sits on one integer global lattice, `chunk * 128 + local`, in `i64`.
  World position is `lattice * cell_size`, evaluated in `f64`. Adjacent chunks
  evaluate the same lattice points at shared borders, so heights are bit-identical.
- Normals use central differences over a one-sample halo (131 × 131 evaluations),
  so border normals are also identical. The standalone patch mesh keeps its
  one-sided edge normals.
- Bounds: at most 256 chunks per axis and 0.25–16 m cells (≤ 524 km), well inside
  the noise coordinate limit and exact in `f64`. Same-platform determinism only.

## Cost

Per chunk: 17,161 sample evaluations × octaves (plus 2 × 2-octave warp samples when
warp is on); 129² heights and normals (≈265 KB). Unmeasured; stage 0 measures it.

## Validation

Unit tests: equal heights and normals on shared edges and the four-chunk corner,
including negative coordinates; equal border paint weights; blend validation;
generation-order independence; finite heights
within bounds; descriptor rejection (version, counts, cell size, non-finite
values, out-of-world coordinates); serde round trip; mesh layout and world-space
border coincidence. The existing patch tests remain the legacy compatibility check.

Rendered fixture (`cargo run --release --example terrain_world_seams --features graphics`,
2026-11-07, Apple M3 Pro / Metal, 960×540, shadowed sun, three textured layers):
2 × 2 chunks versus one merged mesh over the same samples differ in 0.000% of pixels
(>8/255), with and without distance LOD. The same chunks with chunk-local projection
differ by 4.6%, showing the comparison detects texture seams.
