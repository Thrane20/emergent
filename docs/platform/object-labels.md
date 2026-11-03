# Object layers and tags validation

2026-11-02, macOS / Apple Silicon / Metal and Chrome.

Delivered: optional first-asset-root `object-labels.toml`, schema 1 with two name arrays of at most 32 slots; reserved blank slots retain indices. Default layer 0 is Default. Per-asset and per-instance `[classification]` values become ObjectClassification on existing SlotMap-owned entities. A single layer index has a corresponding bit mask; tags are u32. No hierarchy inheritance or render/physics filtering is implied.

Authoring uses revision guarded scene writes and project config publication after native scene/prefab validation. Catalog revision participates in scene revision hashing. Configuration refreshes scene metadata before allowing further membership edits; classification edits use an explicit live inspection command and do not restart the worker. Modal dialogs use native dialog focus containment and Escape/cancel handling. Undefined slots and invalid masks are rejected. Clear/reuse slots deliberately; clearing slots used in saved scenes/prefabs is refused.

Validation:
- Two Rust focused tests pass: bit 31, mask matching, invalid/undefined slots, scene loading/component insertion and undefined tag rejection.
- Ten focused IDE tests pass (classification plus existing placement tests).
- TypeScript and focused ESLint pass; Clippy with warnings denied passes.
- Chrome smoke passes on a disposable standard-template project at port 3001: configure layer/tag slot 31, choose Actors, select Hostile through the modal and verify persisted layer 31 / unsigned tags 2147483648. UI screenshot inspected.
- Workspace suite stopped at unrelated `profiles_validate_types_fields_and_limits`: expected profile list lacks the newly added fixed_wing_physics. Separate locomotion work was preserved.

No frame-time claim. Runtime matching is bounded bitwise work; catalog I/O/validation occurs during loading or explicit authoring edits. Collision group integration, camera masks and brain query controls remain follow-ups.

Follow-up validation: remaining workspace all-feature tests pass with the single unrelated locomotion expectation excluded. The two focused tests also pass without optional engine features. Production IDE build passes. Catalog defaults/invalid input and toggle-boundary regressions are covered in the IDE tests.

Formatting passes. Rustdoc completes with the existing unrelated private map/refresh link warning. Test worker/server were stopped and the temporary project removed.
