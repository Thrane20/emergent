# Physical locomotion boundary (FLIGHT-2)

2026-11-02, macOS arm64, Apple M3 Pro, Rust 1.99.1. **3/8 flight stage gates
complete**. This is an unpowered physical body boundary, not the powered pilot.
BehaviourTest assets and the user's port 3000 service were not changed.

## Delivered behaviour

The engine accepts a `*.locomotion.toml` profile with `type="drone_physics"` and no
type-specific fields. It requires the physics feature, an enabled dynamic body,
positive uniform local scale and the existing dynamic-body hierarchy contract.
Mass/collider/gravity settings belong to the physics body. Scene and runtime
validation prevent kinematic/physical pose ownership conflicts, competing motion
components and raw character-drive calls. Physical brain look is rejected until
the pilot can execute heading goals.

The catalogue status is `boundary`, with no supported flight manoeuvres. The IDE
labels/disables it; powered simulation preview rejects it explicitly. Loading a
scene is permitted for raw external force/torque testing. Six existing locomotion
types remain available and retain their previous behaviour.

`PhysicsBodyState` returns committed transform, measured linear/angular velocity,
actual mass, effective gravity and a reconstruction revision for the same
world-scoped EntityId. Before initialization or after settings/pose edits make
the committed body obsolete, `World::physics_body_state` returns None. A rebuild
changes the revision; the locomotion agent resets history and observes committed
motion after physics, before post-fixed observers. Ordinary observations retain
history. Brain self/neighbour feedback uses measured velocity, not its smoothed
command. Backend handles do not escape and revisions are not serialized identity.

## Verification

- Seven `tests/physics_locomotion.rs` integration tests pass: profile/preview
  status; authored mode/body/scale/writer validation; unpowered falling despite
  altitude/hover intent; first-step lift/torque and actual attitude telemetry;
  body mass/scale/gravity reconstruction and removal; stale/foreign-world reads;
  runtime writer and character-drive rejection; measured self-velocity in a brain
  view condition. The last case sees a target below only when falling feedback,
  rather than zero commanded motion, is used.
- `locomotion::tests::physical_reconstruction_resets_history_but_ordinary_observation_preserves_it`
  verifies history reset on initialization/missing/new revisions and preservation
  for an unchanged body revision.
- Feature-off build: nine scalar controller tests and two physics-disabled tests
  pass. Both authored and programmatic physical boundaries fail clearly instead
  of falling back to kinematic movement.
- Existing physics, locomotion, brain, control and component checks pass in the
  final optimized all-feature targeted run, together with all engine unit tests.
- Both `cargo test --workspace --all-features --release --no-fail-fast` and
  `cargo test --workspace --no-fail-fast` reach only the two existing authored
  `first_project` spin/bounce failures documented in `plan.md`. Engine and standard
  template targets pass; the workspace is not fully green.
- Changed Rust files pass rustfmt with child-module traversal disabled. The initial
  full `cargo fmt --all -- --check` passed, but the final check flags unrelated
  edits in `src/audio/asset.rs`, `src/audio/mod.rs` and
  `src/project_host/audio_tools.rs`; those files were left untouched.
  `cargo clippy --all-targets --all-features -- -D warnings` and
  `cargo doc --all-features --no-deps` pass.
- Three focused IDE locomotion tests and focused ESLint pass. An isolated
  `EMERGENT_NEXT_DIST_DIR=.next-flight2-build npm run build` passes production
  compilation, TypeScript and route generation.
- Headless Chrome on a separate port 3001 IDE, pinned to the standard template,
  verifies the boundary button is visible, says “boundary only”, is disabled,
  the API reports `status="boundary"` with six available types, and no page errors
  occur. The test browser/server were closed and task-specific build directories
  and generated TypeScript includes removed. The shared host validates the same
  profile through `emergent-standard-template --validate-locomotion ... --candidate`.

The native window was not visually exercised. No powered 3D flight, pilot goal
execution, wind, contact landing or full flight-budget claim is made.

## Optimized workload check

The public `flight_baseline` example now runs raw bodies and physical-boundary
bodies in the same process with equal force, geometry and timing boundaries.
Command: `cargo run --release --features physics --example flight_baseline`.
At 60 Hz, each count warms 120 ticks and measures 1,200 ticks; 1 kg cubes are
separated by 3 m, with 9.81 N lift and no contacts, brains or rendering. Both modes
print zero height drift, and kinematic/free-fall reference trajectories remain
unchanged. Each tick includes force submission, physics, transforms and engine
overhead; boundary mode additionally includes ownership checks and observations.

| Bodies | Raw median / P99 µs | Boundary median / P99 µs | Raw / boundary max µs |
| --- | ---: | ---: | ---: |
| 1 | 6.209 / 8.833 | 2.209 / 3.042 | 23.375 / 19.083 |
| 16 | 15.958 / 29.791 | 11.750 / 18.750 | 138.292 / 93.750 |
| 64 | 32.625 / 47.875 | 40.875 / 50.041 | 102.875 / 57.417 |
| 128 | 54.084 / 73.750 | 80.792 / 102.125 | 126.208 / 276.125 |
| 256 | 106.792 / 130.792 | 162.250 / 187.000 | 162.291 / 245.375 |

This is one sequential local sample with uncontrolled workstation activity.
The lower small-count boundary timings illustrate run variance; they are not a
speedup claim. Larger-count observations show added boundary work, which provides
a reference for the next pilot. Snapshot storage allocates on initialization or
reconstruction and updates in place; no whole-engine allocation measurement is
claimed. Isolated repeated profiling, contact/churn workloads and a complete
controlled drone remain FLIGHT-7 work. See the [original baseline](../benchmarks/flight-baseline.md).
