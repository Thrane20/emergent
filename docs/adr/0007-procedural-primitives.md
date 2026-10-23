# ADR 0007: Procedural primitives generate validated meshes

Status: accepted for Iteration 9.

## Decision

Procedural shapes are represented by a validated, headless `Primitive` value. Private shape variants prevent callers from constructing unchecked parameter combinations. Named constructors cover planes, cuboids/cubes, pyramids, UV spheres, cylinders, cones, capsules, and toruses; `PrimitiveKind` supports inspection without exposing invalid fields.

`Primitive::to_mesh` deterministically produces the existing coloured indexed `Mesh`. `Geometry::Primitive` uses the same conversion during renderer upload. This preserves one mesh-validation contract, one renderer SlotMap, one `GeometryId` type, and one draw path.

Subdivision axes accept shape-specific minimums and a maximum of 512. The bound rejects accidental extreme work before allocation. Primitive generation remains loading or construction work and is never scheduled automatically during simulation or rendering.

## Consequences

The first generated vertices contain position and colour only. Surface normals, tangents, UV coordinates, material seams, flat-versus-smooth shading controls, mesh optimization, and equivalent-parameter caching remain part of the advanced mesh/material stages. Adding those attributes should extend the shared mesh contract rather than create primitive-specific GPU layouts.
