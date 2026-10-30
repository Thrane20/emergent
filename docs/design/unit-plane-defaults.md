# Unit planes and TOML revision hashing

Planes created through the IDE use a fixed built-in 1 × 1 metre XZ surface.
Their entity definition needs only `primitive = "plane"` and colour in [geometry];
width/depth are controlled through transform scale. The scene loader defaults
omitted half_width / half_depth to 0.5 m. Explicit dimensions remain accepted for
older/custom authored assets, and the procedural Rust Primitive::plane API still
accepts half-extents. No schema-version bump or runtime identity change is needed.

A plane at root scale X=12, Y=1, Z=12 is 12 × 12 m. Parent transforms compose as
before. Ground's existing 12 m geometry was normalized to the unit plane with
compensating X/Z scale 12 in its asset defaults and sole main-scene override.
These vectors retain normal override semantics: the instance vector replaces the
asset default; they do not multiply to 144. It had no children or explicit physics
settings. Native validation checked all project scenes before journal publication.

Diffuse image repeat uses local-space geometry coordinates. The main-scene
Ground repeat changed from 1 to 12 to retain the same world-space texture tiling
when the local mesh shrank from 12 m to 1 m. Its world geometry, position, colour,
material settings and authored IDs were otherwise preserved. This was a targeted
migration of Ground, not a rewrite of every existing plane or imported mesh.

The TOML editor's old revision function reused the selected-document reader for
all project TOML/Rust dependencies, imposing its 256 KiB preview limit on unrelated
terrain heightfields. Ground's 210-byte source therefore failed to open when a
318490-byte or 312309-byte terrain sidecar was encountered. Revision checks now
stream each contained regular file into a content hash, preserving dependency
conflict detection without buffering it through the preview reader. Only the
selected editable document retains the 256 KiB content limit. Same-size dependency
edits still change the revision; containment/symlink checks remain in force.

Validation: 75 IDE tests; TypeScript, focused ESLint and production build; engine
all-feature optimized tests, Clippy and rustdoc. Native authored-scene tests cover
implicit 1 m planes, explicit legacy 12 m planes, and a default plane scaled to
12 m. All project scenes passed staged native validation for the Ground migration.
A live port-3001 inspection measured Ground's expected world bounding radius
8.485281 m at scale [12,1,12], confirming the original 12 m square. Ground's TOML
was reloaded successfully in the user's port-3000 editor, with no source edits
made through that UI. Full workspace debug/release runs retain the separate
first-project bounce-height test failure at maximum world Y 3.1047554.
