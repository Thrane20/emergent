# Scene-instance transform editing

Scene instances may override translation, XYZ Euler rotation in degrees, and scale
through an optional `[entities.transform]` table. Each supplied vector replaces
the asset's corresponding vector; omitted vectors inherit asset defaults. The
result is local to the parent, not an additional transform multiplied onto the
asset. Reset removes the table. This changes authored data only: fresh SlotMap
handles and renderer-owned geometry retain their existing ownership rules.

The IDE lists instances of the selected scene separately from asset files. The
right inspector exposes nine numeric values, a uniform-scale toggle, Apply &
reload, and Reset to asset defaults. It displays the instance ID, asset reference
and parent. Statements/chat retain their existing asset-file scope.

Reads are bounded and restricted to project asset roots. A revision covers the
scene and referenced asset definitions. Saves reject stale revisions, non-finite
values, zero scales and unsupported source syntax, preserve unrelated TOML and
comments, and reuse guarded publication/backups. The linked game's headless
`--validate-scene <path> --candidate <temporary-source>` command validates the
complete candidate using the engine loader before publication. Recheck source
revisions after validation. No temporary candidate replaces the active scene.

The same loader serves validation, headless play, native Run and paused preview.
All parsing and validation stays outside fixed updates. The first UI updates one
instance per save; camera navigation, picking, gizmos and general undo/redo are
separate increments. Validate independent instances sharing an asset, inheritance,
partial overrides, reset, stale files and rejected edits, plus a real browser
edit/save/reload against the Rust renderer.
