# Experimental bounded ray-marched terrain

Date: 2026-10-30. First milestone: engine prototype, not a production terrain replacement.

## Scene/editor integration increment

Additional Terrain Builder option, not a replacement for heightfields. Persist a
schema-1 `kind="raymarched-terrain"` entity with a `raymarched_terrain` recipe.
World owns its ordinary EntityId/component and hierarchy. Renderer owns up to eight
typed SlotMap field resources; renderer-scoped handles never enter files. Project
extraction updates model transforms and retires resources when their entities die.
Trace in local space and project hits back to world depth; inverse-transpose normals
support finite invertible affine transforms. Render passes retain buffers/pipelines;
only uniforms change per frame. Eight fields is a conservative cap, not measured
performance acceptance. Mesh and implicit fields share depth, not a shadow contract.

Terrain Builder offers a Ray marching tab with bounded seed/width, native orbit
preview, library save/load/update and Add to scene. Reuse staged scene validation,
revision/conflict guards and project transactions. Reject physics/material/heightmap
attachments on these assets; do not silently synthesize a heightfield collider.
Scene tree selection/transforms/framing and Run must render the authored field.
Collision research follows separately (triangle extraction from the same field is
the first candidate); this increment adds no gameplay collision.

Research starting points: Sebastian Lague's MIT-licensed
[Ray-Marching](https://github.com/SebLague/Ray-Marching) Unity test project and
[Marching-Cubes](https://github.com/SebLague/Marching-Cubes). His former project is
explicitly incomplete/unoptimized. Use camera/field composition concepts as
references, not Unity ownership or pipeline code; no source copied in this increment.

## Original standalone prototype rationale

The following records the original singleton prototype. The scene increment above
extends ownership to typed SlotMap resources; it supersedes the one-field limit.

The engine needs a new renderer capability: existing terrain produces meshes and
cannot display implicit overhangs. Keep the first prototype independent of scene
serialization and expose a validated value recipe plus an optional renderer pass.
The renderer owns one experimental field, like its scene environment; it has no
independent entity identity or ownership graph. Multiple authored regions will need
World EntityId and renderer-owned typed SlotMap resources before scene integration.

Use analytical signed distance primitives: bounded ground box, seed-positioned
mountain sphere, subtractive tunnel and torus arch. Union/min and subtraction/max
preserve the distance bound. No unbounded noise displacement or dense voxel volume.
Sphere tracing uses conservative steps, fixed iteration limit and finite ray length;
inside cameras step by absolute distance to reach the exit. Near/far unprojection
supports both perspective and orthographic cameras. Surface hits write projected
WebGPU depth into the existing opaque pass; meshes and particles share that depth.
Lighting is initially fixed directional Lambert plus ambient. Field shadowing,
scene-light parity and mesh shadow-map participation remain separate acceptance items.

Data flow: validated recipe -> renderer uniform/pipeline prepared once -> per-frame
camera matrices -> fullscreen bounded fragment traversal -> colour and depth.
Steady-state CPU work is a uniform upload, without field allocation or mesh rebuild.
Initial workload: one 32m region; 256 maximum steps, 0.002m hit tolerance, 100m ray
limit. These are bounded prototype inputs, not a performance claim.

Collision approach: evaluate the same field headlessly for ray queries and tests.
Do not attach a heightfield collider: caves/overhangs need an extracted triangle
surface or an implicit collision integration. Gameplay collision is unavailable for
this first milestone and must be explicit before scene placement is enabled.

Validation: headless recipe/bounds/inside/outside/cave/arch tests; native offscreen
colour/depth occlusion against ordinary meshes; camera-inside and orthographic views;
optimized wall-time capture timings at several resolutions. Capture timings include
GPU completion/readback and are not GPU timestamp measurements. Follow with editor
authoring, saved recipes, collision extraction and representative camera routes.

Reference: [Hart, Sphere tracing](https://graphics.stanford.edu/courses/cs348b-20-spring-content/uploads/hart.pdf).
Conservative distance bounds are required for safe steps; enhanced tracing and
acceleration are deferred until this baseline is measured.
