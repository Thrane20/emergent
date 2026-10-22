# ADR 0001: Coordinate and transform conventions

Status: accepted for the Iteration 2 foundation.

## Decision

Emergent uses a right-handed world coordinate system:

- positive X points right;
- positive Y points up;
- negative Z points forward;
- one world unit represents one metre by convention; and
- angles are expressed in radians.

Local transforms contain translation, a normalized quaternion rotation, and scale. They compose as `translation * rotation * scale` and operate on column vectors. A child's global affine transform is `parent_global * child_local`. Matrices exposed by the engine use column-major storage, matching `glam`.

Zero and negative scale are allowed. They can produce singular or reflected transforms, so future operations that require inversion or decomposition must return an explicit failure. Non-finite values and non-normalized quaternions are rejected when a local transform is assigned.

Reparenting preserves the local transform. The entity's global transform is therefore allowed to change. Preserve-world reparenting and decomposition of transforms containing shear remain deferred.

`wgpu` integration will use a right-handed, Y-up camera and an explicit projection compatible with WebGPU's zero-to-one depth range. World transforms remain graphics-API independent.

## Math representation

The foundation uses `glam` 0.34.1. `Vec3` and `Quat` represent authored transform values, while `Affine3A` is used internally for cached global transforms. `Affine3A` provides compact affine composition without implying that a global transform can always be decomposed back into translation, rotation, and scale.

## Consequences

- Engine, game, asset, physics, and rendering integrations must convert explicitly if their source convention differs.
- Global transforms are derived cache data. Local transforms and hierarchy links remain authoritative.
- Mutating a local transform or hierarchy link must invalidate the affected subtree before the next propagation phase.
- Rendering code may convert a global affine transform to `Mat4` at the extraction boundary.
