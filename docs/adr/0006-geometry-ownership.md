# ADR 0006: CPU geometry and renderer-owned resources

Status: accepted for Iteration 8.

## Decision

Emergent models `Point`, `Line`, `Polygon`, `MeshVertex`, `Mesh`, `Color`, and `Geometry` as validated CPU values outside the optional graphics module. Headless games, content tools, and tests can therefore construct and validate geometry without a window or GPU.

`GeometryRenderer` owns uploaded GPU buffers in a SlotMap. Public `GeometryId` values combine a private generational key with a renderer identity, so stale handles and handles from another renderer fail validation instead of resolving to an unrelated resource. Geometry resources do not own entities. Games extract copied `GeometryInstance` values from canonical entity transforms each frame.

Points tessellate into octahedral markers and lines into square prisms. This makes their radius explicit and avoids depending on backend-specific point and line rasterization widths. The first polygon contract accepts only ordered, planar, strictly convex vertices and uses fan triangulation. The first mesh contract accepts finite coloured vertices and non-degenerate indexed triangles.

## Performance consequences

Tessellation and immutable GPU upload occur when geometry is inserted. Per-frame model-view-projection values use one reusable, alignment-aware dynamic uniform buffer that grows geometrically. Rendering performs no per-object buffer creation and allocates no collections in steady state below the recorded high-water mark.

The initial renderer issues one draw per instance. Materials, packed mesh arenas, visibility, batching, dirty extraction, and GPU-driven rendering remain later measured work. The four-object gallery establishes correctness and ownership, not a production draw-count target.
