# First Project — Agent Guidelines

This directory is a game project built on the public `emergent` crate. Do not edit engine source under `../..` to implement game features. If a required public capability is missing, document the boundary before proposing an engine change.

## Ownership

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

- Scene and entity files are authored data, even though the engine does not instantiate them yet.
- Use `textures/` for images such as PNG or JPEG, `models/` for future mesh/object files, and `audio/` for sound assets.
- Refer to assets with project-relative forward-slash paths under a declared asset root.
- Do not claim a media type is loadable until an engine importer or project loader exists for it.
