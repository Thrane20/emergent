# ADR 0009: bounded lit geometry and first directional shadows

## Status

Accepted for the first renderer slice.

## Decision

`MeshVertex` may provide a validated normal. The renderer generates missing normals once while geometry is uploaded, then owns the resulting vertex/index buffers under renderer-scoped SlotMap handles.

`LightSource` remains an ECS component on a canonical Emergent entity. Rendering consumes copied `LightInstance` values after transform propagation; it does not create a second entity or light identity store. The first renderer accepts at most eight lights per frame, evaluates Lambert ambient, directional, point, and spot lighting, and selects the first enabled directional request in supplied order for a single 2048-square comparison shadow map.

The renderer preserves `render` for unlit colour output. `render_lit` is the explicit opt-in path. Per-object uniforms carry an inverse-transpose normal matrix when the model's linear transform is invertible; singular transforms use identity to avoid non-finite GPU input.

## Consequences

The light payload is bounded and allocation-free in steady-state rendering. The single map is intentionally insufficient for large scenes: it has no cascades, filtering, atlas, culling, point/spot shadows, material system, or scene extraction. Those remain separate decisions after representative workload measurements.
