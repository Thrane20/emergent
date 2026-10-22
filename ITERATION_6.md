# Iteration 6 — First visible 3D object

Status: complete.

## Outcome

Open a native `winit` window and render one depth-tested cube through `wgpu`. The cube's model matrix is copied from an authoritative Emergent entity global transform after fixed simulation updates. Headless builds and tests remain available without compiling or initializing graphics.

## Ownership and boundaries

- `Engine` continues to own simulation time, schedules, the SlotMap world, and game callbacks.
- A graphics-gated renderer owns the `wgpu` instance, surface, adapter-derived device and queue, surface configuration, render pipeline, depth texture, and fixed cube buffers.
- `winit` owns the platform event loop and window. The visible example coordinates events, fixed simulation advancement, extraction, and rendering.
- Render data contains copied matrices and viewport state. It never holds entity references, Bevy entities, or long-lived borrows into `World`.
- SlotMap `EntityId` remains the object's canonical runtime identity. GPU buffer offsets are transient renderer details.

## Data flow

1. `winit` creates the window after `Resumed`.
2. The renderer selects a surface-compatible adapter and configures the surface.
3. The example advances Emergent's fixed simulation from elapsed wall time with bounded catch-up.
4. The example reads the cube entity's propagated global transform and copies its model matrix into a frame uniform.
5. The renderer writes that small uniform, encodes one indexed draw with depth testing, and presents.
6. Resize recreates the depth target and reconfigures only non-zero surface dimensions. Close requests stop the game and exit cleanly.

## Performance shape

- Cube vertex and index buffers and pipeline state are created once.
- Each frame writes one fixed-size uniform and creates only the command encoder required by `wgpu`.
- Simulation retains its fixed timestep and bounded catch-up. Rendering requests redraws independently and does not change simulation tick semantics.
- This is a smoke workload. No performance target is claimed from a single cube.

## Validation

- Keep existing headless tests passing with default features.
- Compile and lint all targets with the graphics feature.
- Test projection/model matrix creation without a GPU where useful.
- Run the example on macOS, observe the cube, resize the window, and close it cleanly.
- Record the adapter, backend, surface format, and local runtime result. Windows and Linux remain documented but unverified until run on those systems.

## Deferred

- General mesh/material assets, texture loading, lighting, cameras as entities, visibility, culling, batching, and render graphs.
- Multiple objects and incremental extraction buffers.
- Input mapping and project assets.
- Project mode, diagnostics overlays, and renderer profiling.

## Acceptance gate

Iteration 6 is complete when the opt-in example presents a depth-tested cube whose transform comes from the Emergent world, handles resize and surface recovery, exits cleanly, and leaves the default headless validation path intact.
