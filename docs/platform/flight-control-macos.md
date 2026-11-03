# Flight-control foundation validation

2026-11-02, macOS arm64, Apple M3 Pro, rustc 1.99.1. Headless engine work only;
no new editor option or native-window visual validation. Public runtime API and
test/example code live in the engine; no game project code was changed.

## Delivered boundary

- FLIGHT-0: [reference baseline](../benchmarks/flight-baseline.md) and
  [frozen contract](../design/flight-control.md).
- FLIGHT-1: `src/control.rs`, exported as `emergent::control`. `tests/control.rs`
  has nine feature-independent tests and one physical integration test. The
  latter uses a small test-only vertical pilot: measured position to bounded
  velocity, PID to 0–25 N thrust with 9.81 N feedforward and 0.08 s motor response,
  then `World::add_force`. It never writes the body's pose/velocity. The body rises
  from y=0.5 toward y=5, peaks below 5.5 and stays within 0.1 m with vertical speed
  ≤0.1 m/s for over ten continuous seconds by the end of a twenty-second run.
  This does not validate attitude control, wind, collision landing or a full pilot.
- FLIGHT-2 partial: transient torque commands and committed angular-rate reads.
  Unit-cube torque test agrees with 1/6 kg·m² inertia: 1 N·m for one 1/60 s tick
  yields 0.1 rad/s. Torque is accumulated for that tick, then absent on coasting
  ticks. Rebuilding the body clears angular motion. Static/nonfinite/overflowing,
  stale and foreign-world commands are rejected; deleted commands do not reach a
  replacement key. Physics-off force/torque commands return `FeatureDisabled`.

## Checks

- `cargo test --release --features physics --test control --test physics --test physics_disabled`:
  10 controller and 11 physics tests passed; the feature-off test is excluded in
  this build.
- `cargo test --release --no-default-features --test physics_disabled`: passed.
- `cargo test --release --test control`: nine feature-independent tests passed.
- The control test binary's test-only allocator counter delegates to the system
  allocator and counts allocations per test thread. Ten thousand stepping,
  invalid-input and reset iterations allocate zero times. The only new unsafe
  code is this small allocator adapter; production control/torque code is safe Rust.
- `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`, and
  `cargo doc --all-features --no-deps` passed.
- `cargo test --workspace --all-features --release --no-fail-fast`: engine and
  standard template checks passed; only the two existing `first_project` authored
  spin/bounce tests fail, as recorded in `plan.md`. No full green workspace claim.
- `cargo test --workspace --no-fail-fast`: same two existing first-project
  failures; engine and standard template checks passed.

There is an existing feature-limited dead-code warning for `TerrainMaterial.layers`
when building without graphics. It does not occur in the warnings-denied all-feature
Clippy run. Existing IDE tests/build were not rerun for this Rust-only foundation.
No port 3000 service was touched or test service left running. Powered physics flight
profiles, their authoring paths and independent-game smoke remain future gates.
The later [FLIGHT-2 validation](physics-locomotion-boundary-macos.md) records the
now-delivered unpowered profile and body/scene integration.
