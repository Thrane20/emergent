# Generated models with movable parts (MODEL-1)

## Goal

Generate an object (for example a helicopter), split it into parts (body, main rotor,
tail rotor), and animate a part independently at runtime, e.g. spin the rotor in flight.

## Current state

- Object Creator saves a TRELLIS `model.glb` plus `object.asset.json`. Native GLB
  loading, UV textures, editable part sets and scene hierarchy placement are delivered.
- Object Builder structures bake parts into one mesh; per-part runtime behaviour is
  explicitly outside that contract, so they are not the vehicle for moving parts.
- Engine model meshes carry positions, normals and UVs; the base-colour image renders
  through the existing UV-mapped material path.
- Scene instances already form a hierarchy with per-instance components
  (`emergent.spin`, brains, physics) and durable instance IDs.

## Design

**1. Split (IDE, off the frame path).** A saved model gets a **Split into parts** action.
It sends the GLB to a fal part splitter and keeps the result as a candidate, like other
generation jobs:
- `tripo3d/tripo/segment` (GLB in, GLB with named parts out) is the default, because it
  reports part names.
- `hitem3d/hi3d/split` (general mode, low/medium/high granularity) is the alternative.
In the IDE the parts are listed and shown in the 3D viewer. The user can merge parts
(splitters often cut a rotor into blades), **cut a part in two with a plane** when the AI
misses one, rename parts, choose each part's parent, and set its pivot
(default: the part's bounding-box centre; snap to the centre of another part or a picked
point). Saving writes a **model set**:
- `assets/models/<id>/parts/<part-id>.glb`, each re-centred on its pivot;
- one `kind = "model"` entity asset per part referencing its GLB;
- `object.asset.json` gains `parts: [{ id, name, parent, pivot }]` with durable part IDs.

**2. Engine model loading (new capability).** `kind = "model"` entity assets load a
GLB's meshes into a normal engine `Mesh`, with positions, normals, indices and **texture
coordinates**; the base-colour texture renders through UV sampling, so the model looks as
it does in the IDE preview. This adds optional UVs to `MeshVertex` and a UV-mapped
diffuse path beside the existing triplanar one. Loading is bounded (size, vertex
and triangle budgets, like structures), validated and done at scene load, never per
frame. The `gltf` crate is the focused dependency for parsing; no other engine is imported.

**3. Placement.** Placing a split model adds a parent instance and one child instance
per part, preserving the part hierarchy and pivots as local transforms. Each part is then
an ordinary scene instance: select the rotor, add **Spin** (axis Y), and it rotates about
its pivot while the parent flies. Identity stays with SlotMap `EntityId`s at runtime and
durable instance IDs in files.

## Hot paths and scale

Nothing new runs per frame: parts are ordinary entities and transforms already propagate.
Splitting and GLB parsing happen in the IDE or at scene load. Budgets per model (initial):
64 parts, 100 MiB source GLB and 200 MiB exported part set. Native model loading
limits each part to one million vertices and 500k triangles; box cuts enforce the
same per-part geometry limits before publishing an edit.

## Validation

- Engine: GLB load (positions, normals, UVs, indices, base-colour texture), budgets and malformed
  files rejected, part pivot re-centring, a spinning child about its pivot under a moving
  parent.
- IDE: split job lifecycle with a mocked provider, merge/rename/reparent/pivot, save and
  reopen, placement creating the instance hierarchy, staged native validation.
- One live fal split of a real generated model before calling it done.

## Decisions and open choices

- Decided: AI split and manual plane cuts both ship in v1; models render with full UV
  textures.

## Order

1. Done: optional mesh UVs and a UV-mapped base-colour texture in the renderer
   (`tests/uv_material.rs`).
2. Done: GLB loader and `kind = "model"` entity assets (`tests/model_import.rs`, including
   a child part spinning about its pivot under a moving parent).
3. Done: part splitting (`src/lib/model-split.ts`, mocked-provider tests), the `/models`
   editor (merge, plane cut, rename, parent, pivot), and save as a part set validated by
   the engine in a staged copy (`src/lib/model-parts.ts`).
4. Done: placement as a group instance with child part instances; verified in a browser
   run (cut, save, place, reopen) and a native render of the placed scene.

## Known limits

- Plane cuts assign whole triangles by centroid, so the cut edge follows the mesh.
- Each part GLB embeds its own copy of the texture (about 2 MB per part for TRELLIS).
- The live fal splitters have not been called yet; their output is handled by name
  (Tripo's GLB variant preferred over its FBX), but the shape is unconfirmed.

## Box cut authoring — delivered

An editor-owned oriented box is drawn as a screen rectangle through the model's
centre. Its initial depth equals its width. Eight corner handles resize it in its
original coordinate frame; dragging happens in the current view plane, so orbiting
lets the user adjust depth in 3D. Numeric centre/size controls provide exact and
keyboard-accessible adjustment. Drawing/resizing is a draft until **Cut** is pressed.
The camera remains orbitable after drawing; **Redraw box** starts a replacement
rectangle. Escape/Cancel discards the box, never edits geometry. Corner buttons also
support arrow keys; Shift uses a smaller step.

Apply to all intersected parts. Clip triangles against the six oriented box planes,
interpolating normals/UVs, retaining outside polygons and removing inside surfaces.
Do not add caps or solid-boolean topology. Compact surviving indexed vertices; a
no-op preserves original geometry. Bound input/output to the existing 500k triangle
loader limit per part. Prepare every changed geometry before publishing any edit;
failure leaves all parts intact. Fully removed parts are dropped and descendants
reattach to their nearest surviving ancestor; reject removal of the entire model.
Preserve surviving durable IDs, names, pivots, materials and existing save/export
validation. Runtime SlotMap ownership is unchanged; all work runs in the IDE on
explicit interaction, outside engine frame paths.

Validation completed: clipped surface area and interpolated attributes, rotated boxes,
no-op/full deletion, invalid/bounded input, hierarchy repair, pointer drawing and
eight handles, 3D resize/orbit, Cancel/Cut separation, save/reopen of a fresh
template-derived project with a synthetic GLB. No paid split or user game fixture.
See [browser and save/reopen evidence](../platform/model-box-cut.md).

## Object Library publication — delivered

Saving parts also writes a stable `parts/model.prefab.toml` through the existing
prefab schema and the same staged validation/publication transaction. The prefab
references the saved group/part assets with their pivot-relative transforms. Repeat
saves retain its durable prefab ID. The Object Builder library lists these model
entries beside structures; opening one routes back to Model Parts, and its existing
scene action instantiates the full prefab through shared placement. Keep structures
and imported mesh editing in their respective editors; no mesh conversion, runtime
ownership change, new library database or automatic scene insertion is required.
Validation: [Object Library save and placement](../platform/model-parts-library.md).

## Open

- Linked prefabs: placements are expanded copies; editing the part set later does not
  update existing placements.
