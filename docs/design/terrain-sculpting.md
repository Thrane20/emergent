# Terrain sculpting

The Terrain Builder owns a dense editable heightmap in the browser. A top-down
heightmap canvas paints a soft circular grayscale mask (white=1, black=0), scaled
by brush strength. Raise/lower shift metres per second while held; distance-based overlapping stamps
keep quick drags visible. Smooth blends toward a
3x3 neighbourhood from a scratch snapshot; flatten blends toward an explicit
height. Heights stay within 0..relief. Paint updates the canvas while held and
refreshes the existing 3D GLB on stroke completion. Generation replaces edits.
A one-stroke undo protects accidental edits. No image-brush upload in this slice.

Save stores a bounded frozen f32 heightfield as a project TOML sidecar, alongside
the existing recipe asset and scene inclusion in the same journalled transaction.
Old recipe-only assets still regenerate. A private immutable scene component owns
the validated field; physics borrows it only when preparing the static collider.
The component is installed at scene construction and cannot be edited through the
public API. Scene reload uses new EntityIds, invalidating the old collider. Scale changes rebuild from the immutable field; a mismatched runtime recipe is
rejected. Recipe-only colliders retain their existing regeneration behavior. SlotMap EntityIds remain the
canonical identity; no new ownership graph or runtime terrain identity is added.

Workload remains 32..256 samples per side. Brush work is bounded to its sample
rectangle, with reused field/scratch buffers; GLB rebuilds happen at stroke end,
outside simulation. No performance target is claimed. Validate brush mask/strength,
clamping, edges, deterministic smoothing, frozen saves, malformed fields, native
mesh/collider agreement, existing recipe-only loads, and browser drag/undo/save.


The pointer-down event explicitly focuses the canvas before preventing default
browser behavior, so Space paints after clicking. Pointer-up flushes the final
movement segment before committing. A visible stroke status and paint-surface
heading distinguish the heightmap from the orbit-only 3D preview. Compact desktop
layout keeps the complete heightmap visible at a 1920x900 viewport.


The desktop editor places the heightmap at the top of a left sculpting column,
with brush controls stacked underneath and their own vertical scrolling. The
right column contains the full-height 3D viewer with preparation statistics at
its foot. Below 900px the two columns stack. Generation/save remain in their
existing outer sidebar; algorithm comparison keeps its existing card layout.


## Editing saved terrain

The selected terrain instance exposes Edit terrain in ProjectStudio. Each Edit terrain action opens the scene/instance pair with a fresh navigation
token, so Terrain Builder reads current saved sources even if an earlier editor
route is cached. Client navigation releases the main inspection session. GET /api/terrain resolves
the asset through the scene, bounds/validates the recipe and sidecar, or regenerates
recipe-only terrain using the existing Rust tool. The browser builds its normal
preview from those samples. Loading does not write project files.

Update requests carry the scene/instance pair and a revision of scene, asset and
heightfield sources. The server resolves the target again; client asset paths do
not select a write destination. Only the [terrain] recipe and height samples change.
Unrelated asset text/material/physics/default transforms and all scene instances
remain intact. First edits of procedural terrain add a heightfield reference;
existing snapshots retain their reference. Identity remains authored UUIDs and
fresh runtime SlotMap handles on scene reload.

Updates reuse the scene-edit lock, staged native checks and conflict-checked
journal publication. All authored scenes are validated (maximum 127, matching the
current tooling bound) because assets can be shared across scenes. The unchanged
selected scene participates as a publication conflict guard. Saved navigation
includes the new revision so the main editor reconstructs its scene inspection
instead of retaining a cached surface. The UI names the existing target, locks
name/destination and states that updates affect all instances using the asset.
