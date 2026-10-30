# Saved terrain and terrain colliders

Save the last successfully generated Terrain Builder recipe as a UUID-backed
`.entity.toml` asset (kind terrain, [terrain] versioned recipe), plus a new root
instance in the chosen scene. Carry the current scene through navigation and
show the destination explicitly. Edited draft inputs do not replace the displayed
recipe. Validate the staged scene with the existing native project checks before
journalled conflict-checked publication of both files. No partial orphan asset
when inclusion fails. The current engine regenerates the recipe at scene load;
this is not an exported frozen heightfield or streaming format.

Scene loading generates an owned TerrainField and converts it to Geometry::Mesh.
Existing World SlotMap EntityId and renderer GeometryId retain ownership. Picking,
transforms, materials, Run and paused inspection use existing mesh paths.

Add a typed Collider enum (Box / Terrain) with TerrainCollider holding a copied,
versioned recipe. PhysicsBody remains Copy for allocation-free steady-state input
collection; no new identity graph or reference-counted world owner. A static
Rapier heightfield is prepared on body insertion/settings/scale changes. Recipe
regeneration uses the same grid, world height units, diagonal and orientation as
the rendered mesh. Body pose and positive non-sheared scale use existing TRS
validation. Dynamic terrain colliders are rejected, not silently approximated by
boxes. Shape selection is constrained to terrain assets; inspector persists
physics enablement, friction and bounce. Existing box constructors stay usable.

Expected first scale: independent 32–256 grids, scene maximum 1024 instances
(existing limit). Mesh generation and collider preparation allocate on loading or
rebuild, not every tick. No chunk streaming, terrain editing, height queries,
hydraulic erosion or dynamic terrain bodies in this slice.

Validate asset/save bounds, correct scene and displayed recipe, duplicate names
with unique IDs, conflict/failed publication, native load and inspection, terrain
heightfield orientation/triangle agreement, falling body contacts on non-flat
translated/scaled terrain, stale body removal and box regressions. Run normal Rust
checks, IDE checks and targeted temporary-project browser smoke; leave the user's
project assets unchanged during testing.

Scene inspection now frames terrain using world origins and conservative scaled
bounds (including parent transforms). Camera controls allow 2–30,000 m orbit
radius, with existing ±10,000 m target limits. The renderer derives a bounded far
plane from view distance so wider terrain is visible. The reference game also
frames loaded mesh bounds on initial upload for Run and native snapshots. These
are first-pass framing limits, not large-world precision guarantees. Terrain
colliders are not drawn as misleading yellow boxes; their render mesh is the
surface. Existing box outline overlays remain.

Staged scene checks run the staged game's own `--validate-scenes` mode after project
Cargo checks, so project component types resolve. The staged manifest renames the
binary to `<package>-candidate`, so the live game executable is never overwritten
with a binary containing a temporary CARGO_MANIFEST_DIR. Navigation keys scene/instance
props so returning from Terrain Builder selects the newly saved object even when
Next.js retains the earlier project page in its route cache.

Validation evidence: [native macOS checks](../platform/saved-terrain-macos.md).
Saved assets default to smooth lit shading and a neutral gray albedo. The preview
sun is not added to the game scene; authored lights remain in control.


Sculpted terrain extends this path with an optional `terrain_heights` reference
to a bounded schema-1 TOML sidecar containing row-major f32 metre samples. The
sidecar, entity asset and scene instance share the existing save transaction.
The loader validates count, finite bounds and asset containment, retains one
immutable field on the owning world entity and uses it for mesh construction and
static collider preparation. Recipe-only assets retain regeneration. See
[terrain sculpting](terrain-sculpting.md) for editor behavior and invalidation.
