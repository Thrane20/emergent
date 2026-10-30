# Bounded meshed-density comparison

2026-10-30. Compare cached triangle geometry from the authoritative
`RaymarchedTerrain::distance()` field against direct ray marching before choosing
a runtime density/collision system. This does not add a scene collision toggle or
replace the existing renderer. No authored recipes or user scenes change.

## Research and candidate

Primary sources inspected: published crate metadata/source and upstream docs.

| Candidate | Contract / licence | Decision for this spike |
| --- | --- | --- |
| [fast-surface-nets 0.2.1](https://docs.rs/fast-surface-nets/0.2.1/fast_surface_nets/) / [source](https://github.com/bonsairobo/fast-surface-nets-rs) | Naive Surface Nets, sampled regular grid, indexed buffers, reusable storage. Published Cargo licence MIT OR Apache-2.0; depends on glam 0.29 and ndshape 0.3. | Selected behind optional `density-mesh`. Small focused boundary; inspect output topology and accuracy rather than inheriting upstream performance claims. Duplicate glam stays internal to dependency. |
| [mcubes 0.1.7](https://docs.rs/mcubes/0.1.7/mcubes/) | Marching Cubes, volume input, downstream-convertible mesh; published MIT. | Viable comparison candidate; ordinary MC ambiguity/topology needs explicit assessment. Not added. |
| [isosurface](https://github.com/swiftcoder/isosurface) | Multiple MC/dual techniques, dependency-free, Apache-2.0, u32 indices. | Broader algorithm surface than needed for the first bounded comparison. Not added. |
| [isomesh 0.0.11](https://docs.rs/crate/isomesh/0.0.11) | Multiple methods, engine-independent buffers, libm dependency; published 2026-10-14. README explicitly says it has not had human code review. | Interesting ambiguity/subgrid research; do not adopt its broad guarantees without independent review. Not added. |
| Custom marching tetrahedra | Small case table, consistent tetrahedral decomposition possible | Would require new interpolation, sharing, normal/topology validation and maintenance; reuse the focused crate first. |

Version metadata and source establish the chosen dependency boundary, not an ongoing
maintenance guarantee. Pin the published version. No Unity code is copied.

## Ownership and data flow

Engine pure preparation owns a bounded scalar array and extraction scratch, then
returns an ordinary `Mesh` plus preparation statistics. The recipe remains the
authority. Optional `density-mesh` works without graphics/physics; no new persistent
identity graph is introduced. Existing renderer `GeometryId` SlotMap handles own
uploaded resources. Any collision smoke uses the existing Rapier dependency in an
isolated example, without changing World/physics attachment semantics.

`RaymarchedTerrain::extract_mesh(cell_size)` samples the full finite field bounds
with one-cell exterior padding. Use isotropic metre spacing; reject non-finite,
non-positive/tiny spacing and requests above four million scalar samples before
allocating. Quantize dimensions by ceil, retain an exterior halo, and use the
same deterministic distance query for sampling and reference measurements.
No smoothing, automatic filling of missing features or post-hoc topology repair.

Convert library positions to field-local metres and normals into existing mesh
vertices. Reject invalid/degenerate output through existing Mesh validation;
empty uniform grids are valid empty extraction. Return counted sampling/output
bytes and sampling/extraction/conversion timings, without calling these peak or
driver memory. Preparation is blocking and belongs off the frame thread. Sampling
is bounded but cubic; no kilometre terrain at one-metre 3D resolution is promised.

## Workload and validation

Use closed bounded legacy arch/tunnel, v2 overhang and cavern fixtures at multiple
cell sizes, plus a coarse kilometre fixture. Measure surface residuals and triangle
interior errors against the same distance field, topology (open/nonmanifold edges
and winding), reference-versus-triangle ray hits/misses and hit-distance errors.
Grid methods can miss sub-cell features and Surface Nets may be nonmanifold around
ambiguous cells; failures must be recorded rather than silently repaired.

Benchmark warmed GPU camera routes for both representations using existing pass
timing. Use identical field/cameras/viewport but disclose different shading: mesh
rendering does not yet reproduce per-fragment procedural biome materials. Record
triangle/vertex/index counts and uploaded bytes, sampling/preparation/collider
build times. Capture both representations for visual inspection.

Collision suitability is a separate gate: build a static Rapier triangle collider,
check floor/roof/wall and multiple-surface queries where represented, and compare
against the authoritative field. Construction/query success alone is not dynamic
contact, character-controller, transformed-region, seam or replacement acceptance.

Headless tests: bounds/caps, deterministic output, empty/full grids, sphere winding
and closed edges, finite normals. Run native geometry/physics checks and existing
engine tests, formatting, Clippy and rustdoc. Leave world sampling/LOD transitions,
streaming, editing, async revision checks and scene integration for later stages.
