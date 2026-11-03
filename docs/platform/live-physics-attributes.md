# Live physics attributes

2026-11-02, macOS / Apple M3 Pro / Metal.

Physics inspector writes remain revision guarded. A new inspection `physics` command revalidates the saved scene with the project registry, resolves the authored ID against the retained world's existing handles, replaces/removes only that PhysicsBody and refreshes its collider outline. Geometry, camera, selection and simulation tick are retained. Clearing an override resolves the asset default; disabling removes physics. Save and live-application errors are reported separately without an automatic restart.

## Evidence

- `cargo check --all-features`: passed.
- Rust live-attribute regression: passed; checks unchanged entity identity/transform and unrelated body, removal, missing IDs and stale handles.
- IDE inspection protocol tests: 6 passed, including valid physics commands and malformed IDs.
- IDE `tsc --noEmit`: passed.
- `cargo clippy --workspace --all-features --all-targets -- -D warnings`: passed.
- Direct standard-template native inspection smoke: passed. Added a collider to a plane, resized it, rejected negative mass without changing the live result, disabled it and reset the override. PNG output changed for additions/resizes and returned exactly to the original after disabling/reset. All frames retained the same worker PID and tick zero. Test process and temporary scene were cleaned up.
- Existing Playwright physics smoke now requires successful live acknowledgement and unchanged session/worker instead of expecting a restart; the browser smoke itself has not been rerun.

The unfiltered workspace suite hits an unrelated existing contract mismatch: `file_instance_light_and_subdivision_limits_are_enforced` expects the old 1,024-instance cap while the loader now allows 4,096. That test was left untouched.

Validation re-reads the whole saved scene on an edit; parsing latency is unmeasured. This does not update an already running separate native game process.

- Optimized Rust live-attribute regression: passed.
- ESLint for changed IDE files and rustfmt for changed Rust files: passed. Whole-workspace fmt check reports an unrelated concurrently edited `examples/map_bench.rs`; left untouched.

The guarded save still invokes the existing Cargo-based Rust validator. Cached builds normally reuse its executable; pending Rust source/config changes may still require compilation. This change removes the inspection restart and scene reconstruction, not the project's validation/build contract.

- Workspace all-feature suite with the single unrelated instance-cap test excluded: passed.
- Rustdoc generation: completed with one unrelated private intra-doc-link warning in map documentation.
