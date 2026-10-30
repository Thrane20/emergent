# Noise filter assets and field inspection

Filters are reusable project data, not scene instances. Start with a Noise function
asset at assets/filters/<name>.noise.toml. Schema 1 stores durable asset identity
and a [noise] table: algorithm/version, seed, dimensions, resolution, frequency,
octaves, lacunarity, persistence, domain extent and XYZ origin. Perlin, OpenSimplex
and distance-returning Worley use pinned noise-rs 0.9.0, without default features.
No custom engine or browser implementation of these algorithms is introduced.

The engine owns validated immutable settings and a prepared sampler. They are
ordinary owned values; the caller owns generated dense f32 field buffers. They do
not create another entity identity/hierarchy or persist SlotMap keys. Future scene
consumers resolve durable filter references through their owning subsystem. Noise
sampling/generation is explicit construction work, outside frame-critical ticks.
Prepared sampling allocates no per-sample collections; dense fields are allocated
once. This slice does not integrate filters with terrain, shaders or filter graphs.

Normalize octave-weighted signed noise to [0,1] with clamping, without per-image
contrast stretching. Define x-fastest, then y, then z indexing. Generate complete
2D squares or 3D cubes, not just a plane from a 3D function. Bound 2D to 512 square,
3D to 128 cubed, 1..8 octaves, and total octave evaluations to 2,097,152. Validate
finite coordinates and parameter bounds before allocating/sampling. Pin the
algorithm version so seeded output changes require an explicit new contract.

A headless engine example exposes validation/generation to any linked project.
It emits ENF1 + little-endian width/height/depth u32 + x-fastest f32 samples, bounded
to 8 MiB plus header. The server runs this tool with no shell, bounded output,
timeout and one generation in flight. Candidate source is temporary and scoped to
an existing declared asset; source revisions are checked before publication using
the existing guarded TOML editor. Previewing never writes authored files.

The IDE Add modal gains Filters > Noise function. File selection shows parameters
in the right inspector and a larger preview in Overview. Generate preview uses the
draft; Save settings validates and persists it. Seed randomization only changes
the explicit seed input. 3D preview displays selected XY slices of the generated
volume. Black=0, white=1; intermediate values map linearly to gray. Show dimensions,
range and sample count. Keep invalid drafts and the last valid preview on errors,
protect unsaved edits during navigation, and reject stale saves. Raw TOML remains
available and uses the same Rust validator.

Validate deterministic seeds, algorithm differences, 2D/3D sampling, volume indexing,
finite/range/budget checks, invalid-source preservation, path/revision guards and
browser asset creation, seed changes, save/reload, 3D slices and grayscale pixels.
No performance or platform claims beyond observed local evidence.


Version-1 details: octave i uses wrapping-u32 seed + i*0x9E3779B9, frequency *
lacunarity^i and amplitude persistence^i. Normalize as clamp(0.5 + 0.5 * weighted
sum / amplitude sum, 0, 1). Coordinates are offset + extent * index/(size-1), with
both endpoints included. Worley returns Euclidean distance. Pinning is checked by
golden samples as well as repeatability tests. Upstream API reference:
https://docs.rs/noise/0.9.0/noise/ . Cross-platform bit-identical output has not been
established by the local tests.
