# Iteration 9 — Procedural primitive library

Status: complete.

## Outcome

Add a validated procedural primitive layer that produces the existing indexed `Mesh` contract for planes, cuboids/cubes, pyramids, UV spheres, cylinders, cones, capsules, and toruses. The same generated meshes upload through `GeometryRenderer`; no second rendering or identity path is introduced.

## Ownership and SlotMap integration

- `Primitive` is an owned, headless CPU value with validated construction parameters.
- `Primitive::to_mesh` deterministically produces an owned `Mesh`. Generation allocates only for the result and occurs before GPU upload, never in the render loop.
- `Geometry::Primitive` lets the renderer tessellate and upload a primitive once. The resulting GPU buffers remain owned by `GeometryRenderer` under renderer-scoped `GeometryId` handles.
- Entities continue to own only canonical `EntityId` identity and copied transforms. Primitive parameters and renderer handles do not become alternate entity identities.

## Data and generation contracts

- All sizes and radii must be finite and positive. Half-extents are explicit where a shape is centred on the origin.
- Segment and ring counts have documented lower bounds and a bounded upper limit. Invalid or excessive tessellation requests fail before allocating output buffers.
- Plane and cuboid dimensions, pyramid base/height, sphere radius/stacks/sectors, cylinder and cone dimensions/segments, capsule radius/half-segment/rings/segments, and torus major/minor radii/segments are explicit.
- A torus requires its major radius to exceed its minor radius. A capsule requires a positive straight half-segment; a sphere represents the zero-segment case.
- Generated triangles pass the same finite-position, index-range, and non-degeneracy validation as authored meshes.

## Hot paths and expected scale

Primitive generation is setup/loading work. It does not execute during fixed ticks or frame rendering unless a game explicitly requests regeneration. The initial gallery creates eight resources once and reuses their GPU buffers and the renderer's existing uniform staging allocation.

The segment limit prevents accidental unbounded setup work from authored or generated parameters. No claim is made that maximum-detail shapes are suitable for real-time regeneration. Caching equivalent primitive parameter sets is deferred until workload evidence justifies the lookup and memory costs.

## Validation

- Verify every primitive generates a valid non-empty indexed mesh with the expected broad topology.
- Reject non-finite/zero dimensions, insufficient or excessive subdivisions, invalid capsule lengths, and invalid torus radii.
- Keep default headless builds independent of graphics dependencies.
- Run formatting, default and all-feature tests, Clippy, and rustdoc.
- Run an eight-shape native gallery through the available GPU backend, including resize and clean shutdown.

## Acceptance gate

Iteration 9 is complete when all eight primitive families can be created and converted to valid meshes through public headless APIs, rendered through existing SlotMap geometry storage, exercised together in the native gallery, and documented with planned advanced mesh attributes still clearly deferred.
