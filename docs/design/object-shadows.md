# Object shadow controls

2026-10-25. Extend the existing directional shadow map with independent boolean
MaterialDefinition.cast_shadows and receive_shadows settings (both default true
for compatibility). Entity defaults and complete scene-instance material overrides
reuse existing validation, revision guards and paused inspection reconstruction.
No additional identity/store: canonical EntityId and renderer-scoped SlotMap
GeometryId retain ownership, with shadow flags uploaded alongside material data.

Cast=false excludes the object from the depth shadow pass while retaining its
normal colour pass. Receive=false returns full visibility for that surface; direct
lighting/specular and emission continue normally. Casting and receiving are
independent, including for unlit casters (unlit receivers ignore lighting). Debug
collider outlines neither cast nor receive. Steady-state work adds a bounded flag
check per shadow draw and per shaded fragment, with no new allocations/resources.
No performance gain/target claimed.

Scope: first eligible shadow-enabled directional light, existing fixed 2048-square
map and bounded origin-centred projection. Point/spot shadows, cascades and quality
controls remain deferred. Add a supported directional light for visible shadows;
object toggles cannot enable shadows from an unsupported light type.

Validate native image changes for all cast/receive combinations, verify disabled
receivers leave the shadow available to other receivers, authored round trips and
boolean validation, inspector save/restart, and existing material/shadow rendering.


## SH-2 update

Point lights now also cast through six-face maps (two point slots plus the existing
one directional slot). All SH-1 object flags still apply. See
[point-light shadow design](point-light-shadows.md) for bounds and validation.
