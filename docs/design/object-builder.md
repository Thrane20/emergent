# Object Builder: editable structure assets

The builder owns a local authoring hierarchy of durable part IDs, polygon vertices/faces and local TRS transforms. It reuses sceneHierarchy/validParents. This graph describes a mesh asset; it does not introduce a runtime object store. Saving compiles its parts into one indexed mesh on the existing scene entity. World EntityId and renderer GeometryId remain the canonical SlotMap identities. Part transforms are baked during preparation. Per-part runtime behaviours are outside this contract.

A schema-1 `.structure.toml` sidecar retains editable topology and hierarchy; a normal `kind="structure"` entity references it, and scenes reference that entity normally. Shared material-library selection applies to the assembled object; a part may override it with its own library `material` path. Assembly orders triangles so default-material parts come first and each override material owns one contiguous index range; the scene attaches resolved `MaterialSections` to the same canonical entity (no hidden child entities) and the renderer draws each range with its own bind group, pipeline and uniform slot. Shading (flat/smooth normals) follows the object material for the shared vertex buffer, and a mesh with any transparent section is depth-sorted as a whole. Existing inline and primitive assets are untouched. Save/reopen uses revision guards, staged native scene validation and journal publication. Loading rejects missing parents, duplicate IDs, cycles, invalid faces/indices, nonfinite transforms and degenerate triangles before the live world changes.

The editor has a shared asset carousel with geometry previews and a New object tile across the top. A primitive palette and a taller Three.js viewport occupy the main column. The right column scrolls independent Object graph, Transform and Material cards, with Make it with AI pinned at the bottom. Chat starts collapsed and expands into a full conversation view using the home screen message/composer styles; collapse preserves its history and draft text. It uses the already-installed Three.js version used by model-viewer, with its OrbitControls and TransformControls. These browser objects are disposable presentation objects, not engine identity. Object, face and vertex modes support direct dragging and axis handles. Object mode also exposes rotate/scale. Face Extrude duplicates the cap vertices and adds side quads once per drag; dragging inward is a negative extrusion. Inset creates an inner cap and a ring, controlled as a percentage toward the face centroid. No boolean subtraction or implicit welding is performed.

Polygon triangulation uses the same bounded dominant-plane ear-clipping algorithm in browser and Rust, preserving face-to-triangle mapping for selection. Concave simple polygons are supported; invalid/self-intersecting or collapsed edits are rejected. Nonplanar faces are triangulated using their dominant projection. UI operations are transactional, with bounded undo/redo and one history entry per drag. Reparenting preserves the local transform, matching the current scene editor; drop targets distinguish before/child/after and keyboard controls are available.

Initial budget: 128 parts, 4096 vertices per part, 65536 total vertices, 131072 triangles and a 2 MiB source sidecar. Mesh assembly, triangulation and material loading occur outside frame traversal. During dragging only the affected part preview is rebuilt; orbit frames reuse GPU buffers. No measured frame-time claim is made.

Validation: primitive winding and scale, concave triangulation, face movement, shared vertex movement, extrusion/inset caps and side faces, repeated extrusions, negative depth, invalid/collapsed edits, hierarchy cycles/order, exact save/reopen/conflict checks, native structure loading/rendering, browser selection/drag/tools and main-editor placement/reopening. Cross-platform native runtime remains unverified unless exercised.

## Draft AI and library actions

The configured shared AI provider proposes a complete bounded part list through
`replace_object`; new parts select an existing primitive, while retained durable
part IDs can preserve edited meshes. The server validates the resulting structure
before returning it. Empty unsaved drafts and a bounded 12-message conversation
context are supported. Draft geometry and undo/redo stay browser-owned until Save;
the AI route neither writes project files nor creates runtime objects. Material
selection remains the existing whole-object surface contract.

Save updates the entity/sidecar through existing revision checks and native
candidate validation. It does not place an instance. Right-click a saved carousel
tile (or Shift+F10) to open a native modal dialog with destination scene selection,
Cancel and Add to scene. Placement fetches the latest scene revision and uses the
shared scene placement API, independently of the current unsaved draft. Static
SVG geometry previews avoid a WebGL context per tile; they show shape rather than
the assigned material. No runtime storage or SlotMap identity changes.

[macOS validation](../platform/object-builder-ui.md).

## Transform spaces and shape dimensions

Local/Global controls in the Transform card show and edit position, rotation and
scale relative to the parent or assembled object. Global edits convert through
the parent matrix and reject any result requiring shear. The viewport transform
gizmo uses the selected axis space. These are authoring controls; source files
still store local transforms.

AI primitive proposals now accept `size` for width, height and depth. The proposal
bakes dimensions (including legacy provider scale proposals) into each part's
polygon vertices and leaves its hierarchy transform at unit scale. Parent
translation and rotation remain inherited. Existing authored meshes and scale
values are not migrated automatically. The undoable **Bake part sizes** action
moves existing local scales into their meshes and sets hierarchy scales to one,
letting authors repair inherited squashing. Recheck placement after baking:
child offsets change deliberately when parent scale is removed. This does not
create independent runtime joints; those still require scene entities/prefabs.

## Axis views

The Object Builder viewport has X, Y and Z buttons. Each click switches to an
orthographic camera looking along that world axis, centres the full composed
object bounds and fits both axes of the viewport with margin. Clicking the same
button again flips to the opposite side; choosing a different axis starts on its
positive side. The existing Frame object action returns to perspective framing.
Orbit and transform controls use the active camera, and resize reframes the
selected axis. These are view-only controls and do not change the saved asset.

## Edge and group editing

Object mode can hold several part IDs. Shift-click toggles membership; direct viewport dragging and the shared midpoint gizmo transform all selected roots together so children move only once. Translate, rotate and scale use a shared world-space pivot at the selected roots' centre. A new parent is inserted under the selected siblings' existing parent; mixed-parent selections are rejected. It is an
empty authoring part (zero vertices/faces), still identified by a durable part ID
and compiled into the existing mesh hierarchy without adding geometry. Parent
conversion preserves world placement. Shared rotation and scale reject transforms that require shear. Edge mode stores the two mesh vertex indices of a polygon boundary and
moves those vertices together. These editor operations remain draft-only until
Save; native structure validation owns the final asset check.

The compact group action dialog opens by right-clicking a selected part or empty space in the viewport, or a selected part in the graph when multiple parts are selected.
