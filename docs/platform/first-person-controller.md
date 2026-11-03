# First-person controller validation

2026-11-02, macOS / Apple Silicon. FPS-1 uses the existing SlotMap world and shared project host; no performance or cross-platform claim.

Targeted automated coverage in `tests/components.rs` and `tests/physics.rs` checks raw motion consumption across multiple fixed ticks, pitch limits, finite-input validation, input clearing, stale active camera deletion, authored camera ownership, both project fixtures/prefab loading, box-floor support, wall blocking, jumping, mid-air jump refusal and heightfield walking.

Validation results are recorded below. Interactive cursor capture/focus transitions and sensitivity feel still need a hands-on hardware pass. The optional test scene does not change either project's entry scene. Upright box geometry deliberately leaves capsules, automatic stairs and moving-platform attachment open.

- `cargo check --workspace --all-features`: passed.
- `cargo test --workspace --all-features`: passed.
- `cargo test --release --all-features --test components --test physics`: 21 passed (8 component, 13 physics), including final validation tests.
- `cargo clippy --workspace --all-features --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo doc --no-deps --all-features`: passed.
- Standard-template `--preview-scene assets/scenes/first-person.scene.toml --preview-output /tmp/emergent-fps-preview.png`: passed, Apple M3 Pro / Metal, 1280x720. Image inspected: floor, wall, platform and adjoining heightfield visible from player camera. This is an offscreen tick-zero rendering check, not an interactive input check.
- Whole-worktree `git diff --check` finds pre-existing trailing blank lines in `docs/design/object-components.md` and IDE `globals.css`; left unrelated edits intact.
- `cargo test --workspace`: passed.
