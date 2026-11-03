# Prefabs

A prefab is a named project asset (`assets/prefabs/<id>.prefab.toml`) containing
one root and its complete descendant graph. Schema 1 reuses scene entity entries,
component validation and project-relative asset references, with an explicit
`root`. Local durable IDs are resolved to fresh World-owned SlotMap handles on
construction. The root is the control/lifecycle handle; named members remain
individually addressable. There is no separate runtime ownership graph.

## First slice

- Save a selected scene subtree as a named prefab, without removing the original.
- Place an independent snapshot with fresh durable IDs for every member. Preserve
  local transforms, components and internal entity links. The selected root's
  local transform becomes the prefab origin (translation/rotation reset; scale
  retained). Reject external entity links instead of binding another object.
- Shared brain assets remain shared; per-instance `brain_bindings` map their
  authored target names to the instance's members. Validate every binding and
  compile brains by asset plus bindings, preventing cross-instance targeting.
- Rust `PrefabDefinition` validates with the same scene loader, then instantiates
  a graph returning its actual root and a local-ID-to-EntityId map. Loading may
  allocate and perform I/O; instantiation performs setup allocations and no I/O.
  Callers retain renderables/components for existing renderer/project integration.
- Editor uses existing candidate validation, revision checks, journals, project
  events and unsaved-work guards. Prefabs are project-scoped library assets.

Limits: 256 KiB and 1,024 entities, inherited from authored scenes. Prefabs contain
entity assets, not recursive prefab references. Placement copies the graph; edits
to the prefab definition do not propagate into existing scene instances. Audio
is not implemented by this slice. Asset defaults/media remain shared. No new
frame-time or scale claim is made; intended setup cost is proportional to graph
size plus the existing validation costs.

## Validation

Test independent repeated construction and deletion, root/child transforms,
component links and brain bindings, dangling/external references, bad root/cycles,
round-trip capture/placement, revision conflicts and capacity bounds. Run Rust
format/tests/Clippy/rustdoc, IDE tests/typecheck/build and browser smoke. Record
unavailable or failing repository checks separately from prefab regressions.

## Use

Drag a parent from the Scene graph into **Prefabs** to capture it immediately,
using its current name. The section contains a scrollable table of prefab names
and part counts. Drag a row to the scene root or empty graph space to instantiate;
drop on an existing scene node to make the new root its child. Use the existing
scene-tree rename action to name an instance independently. The viewport's
**Add an asset here** picker also accepts prefabs. Or choose **+ Add asset → Prefabs**,
search/select an existing project prefab, and click **Add prefab to scene** to
place an independent instance at the current scene root.

Keyboard: select a scene object, focus the capture region and press Enter to save
it. Focus a prefab row and press Enter to instantiate at the scene root. Stale or
cross-project drag payloads are rejected; failed captures leave the source intact.

```toml
schema_version = 1
id = "drone"
name = "Drone"
root = "body"

[[entities]]
id = "body"
asset = "assets/entities/body.entity.toml"

[[entities]]
id = "rotor-front-left"
parent = "body"
asset = "assets/entities/rotor.entity.toml"
[entities.transform]
translation = [-1, 0.2, 1]
```

```rust,ignore
let drone = PrefabDefinition::load_named(&project, "Drone")?;
// Use load(project, asset_path) instead when labels are ambiguous.
let scout = drone.instantiate_as(&mut world, "Scout 1")?;
let second = drone.instantiate_as(&mut world, "Scout 2")?;
// Root controls the complete hierarchy; local member IDs resolve within each copy.
let body = scout.root;
let rotor = scout.entities["rotor-front-left"];
```

The loaded definition is reusable; spawning clones its prepared construction data
and performs no file reads. Attach returned project components and renderables
through the same game/renderer integration used for `LoadedScene`. Spawn time
and mesh-copy memory have not been benchmarked; this is a construction API, not
a promise of allocation-free spawning during a frame.

## Drag authoring and label lookup

The Prefabs section becomes a drop target and a bounded scrollable table. Dropping
an existing scene node captures its subtree immediately, using its current display
name. The original stays in the scene. Dragging a table row onto the scene root
adds a fresh instance; dropping onto a scene node parents the new root there,
keeping its authored local transform. Keyboard Enter on the capture region saves
the selected subtree, and Enter on a prefab row instantiates at the scene root.
Both directions keep project/scene/revision checks and existing native validation.
The UI has no separate save button or prefab selector. Long operations disable
competing edits, and errors remain visible in the originating section.

`PrefabDefinition::load_named` / `load_named_with`: scan declared asset
roots for an exact prefab name, reject missing or ambiguous labels, then use the
existing validator. Discovery is loading-time I/O, skips symlinks, and never runs
in the fixed update. Callers load once and use the existing instantiate methods.
No new runtime identity/store or per-frame lookup is introduced.

## Sidebar cards and origin icons

The main sidebar has independent Scene graph, Prefabs and Project files cards. Project files starts collapsed and is selection-only (no delete buttons). Scene graph scrolling does not scroll the prefab header away. Prefab capture has a compact drop/keyboard target alongside the collapse button; it stays present when the library is collapsed. Expanded rows remain draggable and keyboard-placeable.

Editor placement records `prefab_label` on the expanded root. This optional, bounded printable label is authoring provenance only: it selects the prefab icon after reload, carries no runtime key, introduces no live source dependency and does not change ownership or linked-update semantics. Ordinary objects use a cube icon; prefab assets/roots use a primary-coloured package icon. Duplicates retain provenance. Legacy expanded roots without provenance are left unchanged rather than inferred from names or matching geometry.

Optional entity-list defaults are valid when written explicitly during capture/remapping: a field whose schema declares an empty-array default accepts 0–64 references. Required lists retain their 1–64 rule. This keeps locomotion steering/rolling wheel defaults consistent between ordinary scene loading, inspector clearing and prefab snapshots; malformed IDs and oversized lists remain rejected.
