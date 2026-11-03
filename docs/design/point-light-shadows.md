# Point-light shadow maps

2026-10-25. Extend the existing renderer, preserving canonical SlotMap EntityId /
renderer-scoped GeometryId ownership and authored LightSource ShadowCasting flags.
No new durable or public GPU identity. Two point-shadow slots coexist with the
existing one directional shadow map and eight-light total limit. Reject additional
shadow-enabled point lights explicitly rather than silently dropping their shadows.

Each slot has six 512-square Depth32Float layers, rendered with 90-degree perspective
views along +/-X/Y/Z. A retained 12-layer depth texture and layer views/bind groups
are allocated once. Fragment shading selects a face from light-to-surface direction,
projects through the matching matrix, and compares depth. Projecting the receiver
slightly toward the light provides bounded world-space bias. Cast/receive material
options remain independent and debug geometry remains excluded.

Main frame uniforms carry the twelve point view-projection matrices. A separate,
aligned dynamic matrix buffer supplies each depth pass (one directional + twelve
point faces). Reuse CPU staging and GPU resources; no new steady-state allocations.
Expected first workload: tens/hundreds of meshes, one/two point lights. Shadow depth
storage is 12 MiB for point slots plus 16 MiB directional per renderer. This cost and
up to twelve point depth passes are explicit; no frame budget/performance claim.

Point shadows use the light's world position/range, independent of orientation.
Range must be 0.01..1000000 and world position within +/-1000000 per axis when
shadow casting is enabled. Near distance is range*0.001. Light range bounds each
frustum. Texture faces clamp at their edges; seam-aware filtering, quality controls,
caching/culling, more shadow slots, spot shadows and cascades remain deferred.

Validate all six directions, moved lights, multiple lights, out-of-range surfaces,
cast/receive independence, invalid bounds and slot limits, and existing directional
shadows through real native image comparisons plus projection/selection unit tests.


## Use

Enable `casts_shadows = true` under a point asset's `[light]` table. Reload or Restart
inspection after changing the asset. The existing reference key-light already opts
in, so its shadows work after reloading with the updated renderer. Other newly added
point lights retain the existing opt-in default. The object's Material Cast shadows
and Receive shadows controls apply to directional and point shadows alike.

Select **Point shadows — lamp and cube** for a contained demo, or run:

```sh
# Prepare the engine-owned disposable fixture from src/emergent-ide:
# npx tsx scripts/mcp-smoke-project.mts
cargo run --release --manifest-path target/mcp-smoke/game/Cargo.toml -- --run-scene assets/scenes/point-shadows.scene.toml
```

Moving the light updates all six frusta each rendered frame. Rotating a point light
does not affect its shadows. Inspection remains at gameplay tick zero. Existing
main-scene assets and lighting were not modified.

Validation: [SH-2 macOS evidence](../platform/point-light-shadows-macos.md).
