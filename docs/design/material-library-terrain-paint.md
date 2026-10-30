# Material library and terrain painting

Reusable schema-1 `.material.toml` assets hold the existing surface controls. Objects select a project-relative material asset; existing inline definitions remain supported. The Material Generator owns editing, with shared controls and conflict-checked publication. No existing user assets are migrated automatically.

Terrain owns a schema-1 `.paint.toml` sidecar: four library references, an RGBA byte blend map at heightfield resolution, and three ordered height transitions plus a transition width in local metres. Height painting replaces the blend map explicitly; manual strokes blend toward the chosen slot using the existing soft circular brush. Undo is per stroke/process. Saving/reopening preserves exact weights. Changing the terrain resolution requires a new painting map.

Scene preparation resolves files once. The Material component owns resolved inputs; EntityId and renderer-owned GeometryId remain canonical SlotMap identities. Material references are durable paths, never runtime keys. Nested material references and terrain paint on non-terrain geometry are rejected.

The GPU blends up to four complete materials, using one packed noise volume, a diffuse texture array, a weight texture and bounded uniforms. Texture preparation occurs during resource upload, never in the render loop. Terrain diffuse layers are resampled to a common size capped at 1024; terrain noise volumes are capped at 64 cubed per layer. Ordinary material resolution is unchanged. Four slots match river bed, ground, mountain and summit without an unbounded shader loop. These are limits, not measured performance claims.

Validation covers reference containment/cycles, weight dimensions and normalization, height transitions, brush interpolation, asset revision conflicts, legacy inline loading, native shader/renderer smoke and IDE save/reopen. The browser preview is an editing aid; any differences from native rendering must be stated in the UI.
