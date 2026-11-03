# First Project — Agent Guidelines

This directory is a game project built on the public `emergent` crate. Do not edit engine source under `../..` to implement game features. If a required public capability is missing, document the boundary before proposing an engine change.

This is an ordinary game, not an engine/IDE reference implementation or source of
truth. Shared contracts belong in engine source and `templates/standard`; no IDE
feature, engine test or project generator may depend on this project's code/assets.
Build this game explicitly using its Cargo.toml; it is outside the engine workspace.

## Ownership

- `src/main.rs` hands control to `emergent::project_host::main`; the window, inspection, previews and every `--` tool command come from the engine. Implement IDE-facing features there, not here.

- Project-authored Rust belongs under `src/`.
- Generator-owned, reviewable Rust belongs under `src/generated/`. Keep its module exports explicit and deterministic.
- Authored content belongs under `assets/`. Do not put generated caches, compiled artifacts, or absolute machine paths there.
- `emergent.toml` is the project contract. Keep paths relative and portable, and retain stable project/output IDs.

## Engine usage

- Use `EntityId` and `BehaviorId` for live runtime references. Never invent a second entity identity or persist SlotMap keys in assets.
- Construct and mutate entities through public `World` APIs. Use the lifecycle queue from Bevy systems.
- Treat linked behaviour inputs as previous-tick snapshots. Do not depend on insertion order for same-tick results.
- Keep fixed-update work bounded and allocation-free in steady state where practical.

## Assets

- Scene instances reference entity assets through the public `SceneDefinition` loader. Consult current engine capability documentation for supported assets and schemas; this game's asset inventory does not define engine capabilities.
- Use `textures/` for images such as PNG or JPEG, `models/` for future mesh/object files, and `audio/` for sound assets.
- Refer to assets with project-relative forward-slash paths under a declared asset root.
- Do not claim a media type is loadable until an engine importer or project loader exists for it.
- `--inspect-scene` retains an authoring world at tick zero. Reserve stdout for the framed inspection protocol; log construction diagnostics to stderr. Tool commands do not advance the normal game loop.

- Scene authoring tools use durable instance IDs and public World conversion/reparent APIs. Propagate before reading global transforms; reject lossy shear conversions and retain local TRS on reparent. `--scene-transforms` and `--convert-global` reserve stdout for TOML reports; `--validate-scenes` checks all requested scenes without rendering/gameplay.

- Inspection gizmo drags own an ephemeral gesture token and a starting transform snapshot. Use public TransformGizmo/PickingMesh APIs, retain tick zero, preview only in memory and publish through the IDE authoring guard once on release. Never let an unmatched token finish or cancel another gesture.

- Terrain assets regenerate through public TerrainSettings; TerrainCollider is a static heightfield matching that recipe. Box colliders remain available. PhysicsBody.collider is typed Collider; use as_box() before reading box dimensions.
