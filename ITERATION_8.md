# Iteration 8 — First-class geometry

Status: complete.

## Outcome

Model and render the first four geometry families: a point, a line, a polygon, and an indexed mesh. CPU-side geometry remains available to headless games and tools. The optional `graphics` feature supplies a native `wgpu` renderer and a visible reference scene containing all four forms.

This iteration establishes geometry ownership and validation. It does not add asset import, materials, textures, lighting, arbitrary concave polygon triangulation, editing, visibility culling, or render batching across materials.

## Ownership and SlotMap integration

- `Point`, `Line`, `Polygon`, `MeshVertex`, `Mesh`, `Color`, and `Geometry` are owned CPU values. They contain no GPU objects or entity identity.
- `GeometryRenderer` owns uploaded vertex and index buffers in a `SlotMap<GeometryId, GpuMesh>`. `GeometryId` is a typed, generational, renderer-scoped runtime handle.
- Removing uploaded geometry invalidates its handle. Rendering a stale or foreign handle returns an explicit error and never aliases a replacement resource.
- Entities remain canonically identified by `EntityId`. A game extracts copied `GeometryInstance` values containing a renderer geometry handle and model matrix; neither storage owns the other.
- Geometry handles are runtime-only. Project files will use durable asset IDs when authored mesh loading arrives.

## Data and validation contracts

- A point has a finite position, positive finite radius, and finite colour. It tessellates to an octahedral marker so it remains visible across native backends.
- A line has distinct finite endpoints, positive finite radius, and finite colour. It tessellates to a square prism rather than relying on implementation-defined line width.
- A polygon has at least three ordered vertices and one colour. The first implementation accepts only finite, planar, non-degenerate, convex polygons and triangulates them as a fan. Unsupported concave or self-intersecting inputs fail instead of rendering incorrectly.
- A mesh contains finite coloured vertices and `u32` triangle indices. Index count must be a non-zero multiple of three; every index must be in range; every triangle must have non-zero area.

## Rendering and hot paths

1. Upload validates and tessellates geometry once, then creates immutable GPU vertex and index buffers.
2. Each frame receives a borrowed slice of copied `GeometryInstance` values.
3. The renderer validates every geometry handle before acquiring the surface frame.
4. Model-view-projection values are written into one reusable CPU staging vector with device-aligned dynamic-uniform strides, then copied with one queue write.
5. One depth-tested colour pipeline draws every instance. GPU geometry buffers stay resident until removed.

The uniform buffer begins with bounded spare capacity and grows geometrically. Steady-state rendering at or below the current high-water mark does not allocate CPU collections or create per-object buffers. Draw-call batching, packed geometry arenas, frustum culling, and dirty-only uniform uploads require representative workloads and measurement before adoption.

## Expected scale

This is a correctness and boundary milestone for small development scenes. The reference workload is four geometry resources and four instances. No production object-count, draw-call, frame-time, or memory target is claimed yet. Later extraction benchmarks must establish those budgets before the public renderer is described as scalable.

## Validation

- Unit-test invalid point/line values, polygon degeneracy/planarity/convexity, mesh indices, degenerate triangles, and successful constructors.
- Unit-test tessellation counts and ensure generated mesh indices remain valid.
- Test SlotMap removal and stale-handle rejection without requiring a GPU where practical.
- Keep formatting, default and all-feature tests, Clippy, and rustdoc clean.
- Run the visible reference scene in smoke mode on the available native platform, recording adapter, backend, resize, presentation, and shutdown.

## Acceptance gate

Iteration 8 is complete when the public CPU types model all four requested geometry forms, invalid input fails explicitly, the renderer displays one instance of each from renderer-owned SlotMap storage, stale geometry handles fail safely, the native smoke run succeeds, and headless builds remain independent of `wgpu` and `winit`.
