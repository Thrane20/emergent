# Inspection at 30 Hz, picking and transform gizmos

Keep the existing tick-zero native worker, SlotMap entity ownership and latest-only
960x540 PNG channel. Pace to 30 Hz without catching up missed frames. Record a
release baseline before/after; target cadence is not a universal achieved rate.

Reuse the renderer's actual view/projection and tessellated geometry. Build an
optional inspection-only CPU picking cache once, indexed by canonical EntityId;
raycast nearest triangles with a bounds rejection and clip-depth checks on clicks,
not every frame. Non-rendered authored nodes/light markers can be selected from
the scene graph and by projected origin markers. No runtime keys leave Rust.

Rust owns selected entity, gizmo coordinate math and an optional drag snapshot.
The browser draws projected axes/rings, maps its letterboxed viewport to normalized
coordinates and sends bounded pick/select/begin/drag/finish/cancel commands. Local
translation uses parent axes; local rotation/scale use object axes. Global uses
world axes and the existing strict TRS conversion. Global Scale left-multiplies
the world linear transform by a world-axis scale while retaining its pivot; it
does not modify object-axis scale components under a world-aligned handle.
Unsupported shear conversions reject the edit. Disabled near-parallel handles report a useful error.

Pointer drag previews mutate only the retained inspection world. Coalesce movement
commands (one in flight), freeze other edits while dragging, then persist the final
local transform once on release through revision guards and Rust candidate
validation. Cancel/Escape/pointer cancellation or failed publication restores the
original retained transform. Selection and inspector state remain unified. Camera
orbit remains drag on the canvas, Shift-drag pan, wheel zoom; a click picks.

Acknowledgements can carry bounded TOML tool results (authored selection ID,
projected handles/markers, local final transform). Match overlays to acknowledged
frame command sequences to avoid placing controls on an older camera frame.
Begin binds the authored ID, tool and space; a per-gesture token must match on
every drag/finish/cancel, preventing another client from mutating that gesture.
One drag snapshot per session, no unbounded command or frame backlog. Picking and
gizmo work runs only on commands. No simulation ticks or whole-project file writes
occur during pointer moves.

Validate ray/triangle occlusion and misses, clip/projection math, parented local
and global TRS, rejected shear, cancellation/stale sessions, real browser picking
and three drag tools, source persistence/conflicts and retained worker identity.
Picking acceleration beyond bounds checks, snapping, multi-select, free/plane
handles, undo/redo and hardware video are outside this first slice.
