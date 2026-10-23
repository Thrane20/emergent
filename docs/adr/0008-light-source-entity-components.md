# ADR 0008: Light sources are entity components before renderer inputs

Status: accepted for Iteration 10.

## Decision

A light source is a canonical Emergent entity carrying a validated `LightSource` Bevy component. SlotMap-owned `EntityId` remains its identity and hierarchy handle; no separate light ID or ownership store is introduced for scene-level light declarations.

The first component supports ambient, directional, point, and spot kinds. It stores validated colour, a finite non-negative engine intensity scalar, optional positive range, optional ordered spot half-angles, and disabled/enabled shadow-casting intent. Ambient lights cannot request shadows. Private fields and named constructors keep invalid states out of component storage.

Entity transforms provide spatial data. Point lights use global position, directional and spot lights use the transformed local negative Z axis, and ambient lights ignore transform. Authored durable IDs resolve to fresh entities when a scene is constructed; runtime entity handles are not serialized.

## Consequences

Game projects can define and inspect light entities through the public headless API before a lit renderer exists. The component is ordinary dense ECS data and follows canonical entity lifecycle automatically.

The current vertex-colour renderer does not consume lights. Surface normals, materials, exposure, render extraction, visible-light selection, GPU light buffers, shader evaluation, shadow maps, quality settings, and measured light/shadow budgets remain future work. Shadow casting in this iteration is a request that future render extraction may honor, not evidence that a shadow is rendered.
