# Object facing and editor focus/alignment

Forward is transformed local -Z, normalized in world space. It is derived from
the existing rotation/scale hierarchy, not stored as a competing orientation.
The inspector shows the world-space look-at direction for every scene instance.

F queries bounds from the retained native picking mesh, frames them using the
actual editor view/projection, and changes only the inspection camera. Shift-F
queries a proposed local transform matching the editor's world position and
orientation, preserving world scale. The native World global-to-local conversion
rejects lossy shear rather than approximating parented transforms. Proposals do
not mutate the world; publish through the existing revision-guarded placement
save, then update inspection. Failed validation leaves authored placement intact.

Ownership remains World/EntityId and renderer-scoped geometry. Bounds use the
existing picking cache with eight transformed corners; do not retessellate or
scan every vertex on keypress. Inspection stays at tick zero. Keyboard handling
ignores typing, dialogs, modifiers, repeat and in-flight edits, and blocks selection
while alignment commits. Test hierarchical facing, native bounds, camera-derived
alignment, stale saves, both projections and actual browser shortcuts.
