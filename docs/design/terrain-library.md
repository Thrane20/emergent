# Terrain Builder library

Terrain Builder discovers existing project `kind="terrain"` entity assets and saves/loads them independently of scene placement. The existing entity, frozen-height and material-paint sidecars remain canonical authored data. No runtime ownership changes: scene loading still resolves durable asset paths to the engine's SlotMap handles.

The top carousel uses lightweight shaded isometric SVG swatches derived from saved heightfields and material albedos. Recipe-only terrains use the existing Rust generator. Authoring generation is serialized with a bounded queue; thumbnails are cached by asset/dependency revision, with bounded memory. No loading, mesh work or new allocations are added to engine frame paths.

Library saves stage an isolated probe scene through the existing candidate validator and atomic publication path. Updates preserve shared material, physics, transform and asset identity; stale source/height/paint/material revisions reject saves. Placement appends a fresh scene instance referencing the saved asset, with revision checks and no asset rewrites. A dirty-work dialog guards library loads and New terrain; existing scene-instance editing remains available.

Validation covers library-only saves, exact heights/paint round trips, updates and stale revisions, repeat placement with shared asset/unique instance IDs, thumbnail geometry/invalidation, and desktop/mobile carousel/load/save/placement behavior in a disposable project. User port3000 remains untouched; test services use IPv6 port3001 and stop afterward.
