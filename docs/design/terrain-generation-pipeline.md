# Terrain generation pipeline — versions 1 and 2

Implement a reusable, headless Rust preparation path: seeded Perlin or diamond-square base → bounded coordinate warp → preset shaping and controlled mountain/ridge layout → optional thermal erosion → dense height field → smooth mesh. The IDE consumes the same Rust height field and retains its existing grayscale GLB preview and directional sun.

Settings are serializable, deny unknown fields and carry an explicit algorithm version (default 2, with version 1 retained). Independent seed streams keep mountain placement stable when changing the base generator. Diamond-square uses an internal 2^k+1 grid and bilinear resampling to the existing 32/64/128/256 sample grids. Frequency/octaves apply to Perlin; roughness also controls diamond-square displacement decay. Thermal erosion is a bounded, double-buffered, conservative four-neighbour transfer with a world-space slope threshold and closed boundaries. It is not hydraulic erosion.

Ownership: generation returns owned dense values and a mesh, without creating entities or another identity store. Instantiation still uses the existing World and renderer SlotMap owners. Generation, allocation and mesh preparation happen outside gameplay/render traversal. Resolution is at most 256 squared; at most 64 erosion passes; buffers are allocated once and reused across passes. No new dependencies, background worker framework or frame-loop work.

Comparison is an explicit tooling request with three sequential generations: original Perlin shaping (warp/ridges/erosion off), diamond-square shaping (same), and enhanced settings. Seed, preset, extent, relief, resolution, sun and initial absolute camera framing match. Separate cameras may be orbited independently. Timing measures Rust generation and JavaScript GLB preparation; field and GLB bytes are exact output sizes, not peak process/GPU memory. A bounded three-model JSON/base64 response is acceptable for this preview tool; it is not a runtime transport.

Validation: repeatability, finite/bounded outputs, coastline and flat-terrain invariants, diamond-square/resampling, seed variation, erosion conservation and slope reduction, smooth upward mesh normals/winding; API validation and browser comparison/load/lighting smoke. Record optimized native generation samples with hardware and workload. No frame-rate claim. Preserve the last good preview on failure and release blob URLs on replacement/unmount.

Deferred: ray marching, hydraulic erosion, world/chunk stitching, runtime LOD, frozen heightfield export. Saved scene assets and static terrain colliders are now described in [saved-scene-terrain.md](saved-scene-terrain.md). Version 1 describes independent patches; adjacent patches are not guaranteed seamless. Reproduction is version/recipe stable on the tested platform, not a promise of bit-identical floating point across all platforms.

## Version 2 — mountain massifs

Replace the enhanced single radial tip with a bounded four-summit crest per major
mountain group: jittered summit heights/widths, saddles along its spine, broad
shoulders and asymmetric local contour warping. Independent morphology seed
streams retain the existing major-centre positions and inter-group ridge links.
Mountain count is the number of groups, not an exact summit count. Morphology is
prepared once and sampled without per-vertex allocations; height bounds and
thermal erosion remain unchanged. No new ownership, dependencies or UI sliders.

Default recipes use version 2. Explicit version 1 remains reproducible; comparison
baselines explicitly use version 1, while Enhanced retains the selected recipe.
Validation covers multiple substantial summits within a single group, retained
ridge connectivity, stable legacy samples, seeds/count/bounds and release timings.
