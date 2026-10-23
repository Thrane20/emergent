# Iteration 11 — Lit geometry and first directional shadows

Status: complete. The 2026-10-23 Metal smoke created the lit pipeline, submitted the directional shadow path, presented 239 frames, observed resize, and shut down cleanly; see `docs/platform/iteration-11-macos-smoke.md`. Visible image quality and frame timing remain unmeasured.

## Outcome

Extend the optional `wgpu` geometry renderer with normals, copied light inputs, bounded GPU light data, diffuse lighting, and one directional shadow map. Prove the path in a native lighting gallery built entirely from canonical entities, `LightSource` components, and renderer-owned geometry handles.

## Ownership and SlotMap integration

- `MeshVertex` remains an owned CPU mesh value. An authored normal is optional; missing normals are generated during renderer tessellation from indexed triangle topology.
- `GeometryRenderer` continues to own vertex/index buffers, the light uniform buffer, depth targets, and the shadow texture. `GeometryId` remains the only renderer resource handle and stays renderer-scoped and generational.
- Light identity remains the canonical `EntityId` plus its `LightSource` component. `LightInstance` is copied render input derived from a component and a propagated transform; it is neither durable identity nor a second light store.
- The renderer accepts no more than eight light inputs per frame and selects at most one enabled directional light for shadow mapping. The selection is deterministic in supplied-input order.

## Data flow

1. A game constructs entities and attaches `LightSource` components through public `World` APIs.
2. After transform propagation, the game copies required world transforms and light components into `GeometryInstance` and `LightInstance` render inputs.
3. The renderer validates input matrices, writes one bounded light uniform buffer and reusable per-object dynamic uniforms, then runs an optional directional depth pass followed by the lit colour pass.
4. Fragment shading evaluates a simple Lambert response. Ambient lights add colour directly; directional, point, and spot lights use transformed normals. The one selected directional shadow light samples its depth map with comparison sampling.

## Hot paths and limits

- The per-frame light upload uses a fixed-size eight-light uniform payload; it does not allocate in steady state.
- Geometry upload and missing-normal generation occur during `insert`, not per frame.
- Per-instance dynamic uniforms and render loops reuse existing renderer staging memory.
- The first shadow map is a fixed 2048-square depth texture. Its pass draws every submitted shadow caster; no visibility culling, atlas, cascade, point-light cube map, or resolution scaling is claimed.
- Light limits and shadow-map resolution are deliberate first bounds, not measured production targets.

## Contracts

- `MeshVertex::with_normal` accepts a finite, non-zero normal and stores a normalized value. `MeshVertex::new` remains available for meshes whose normals are generated during renderer upload.
- `GeometryRenderer::insert_with_shading` accepts `ShadingMode::Smooth` (the default) or `ShadingMode::Flat`. Flat mode expands indexed triangles during upload and assigns each face one normal; it deliberately trades GPU vertex memory for clear face boundaries and a simpler fragment path.
- `GeometryRenderer::render` preserves the existing unlit-compatible call surface. `render_lit` accepts explicit copied `LightInstance` values.
- A shadow-casting request only becomes a shadow when it belongs to a directional light selected for the bounded shadow pass. Point and spot shadow requests remain data for future work.
- Non-uniform model scaling is supported by an inverse-transpose normal matrix stored in the per-object uniform.

## Validation

- Validate authored normals and missing-normal generation, including finite unit-length GPU normals.
- Validate light-input bounds, transform validation, deterministic directional-shadow selection, and WebGPU depth conventions mathematically.
- Retain geometry and primitive tests, then run default and all-feature release suites, Clippy, rustdoc, docs validation, and the independent first project.
- Run the native lighting gallery with resize and clean shutdown on the available backend. This proves resource creation, light upload, shadow pass submission, presentation, and shutdown; it is not a frame-time, quality, or Windows/Linux result.

## Acceptance gate

Iteration 11 is complete when the renderer uploads usable normals and bounded light data, a lit native gallery visibly submits a directional shadow pass over canonical entity geometry, the public API documents exact light/shadow limits, and validation evidence is recorded without overstating the incomplete rendering stack.
