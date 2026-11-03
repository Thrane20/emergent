# Drone takeoff and landing

2026-11-02, macOS arm64, Apple M3 Pro. Optimized Rust build, fixed timestep
1/60 s, all engine features enabled. This is a correctness run, not a performance
benchmark or a native-window visual check.

The BehaviourTest project at `/Users/iansorbello/dev/emergent-projects/behaviourtest`
has Cube with a DroneLevel1 locomotion profile and the “Drone Takeoff and Land”
brain. The original Sequence could never leave Altitude(5) → Hover: Hover remains
active and Altitude had no arrival completion rule. The existing spatial Arrive
node already uses a stop radius; this fault was specific to altitude completion.

The brain now enables `finish_on_arrival=true`, `arrival_radius=0.1` on both
altitude steps. Takeoff targets 5 m; landing targets centre height 0.5 m, matching
the cube's 0.5 m half extent. The drone profile sets `ground_height=0.5`.

A disposable integration probe loaded the actual project and
`assets/scenes/main.scene.toml`, instantiated it through `SceneDefinition`, then
ran 1,800 engine ticks. It was removed after verification; the portable regression
is `altitude_can_finish_in_a_sequence_without_changing_continuous_hold` in
`tests/brains.rs`.

Observed:

- Landing intent began at 2.600 s, centre height 4.9273 m.
- Peak centre height was 5.0344 m (residual upward velocity while reversing).
- After 30 s, position was `[0, 0.5, 0]`, speed `0.000000 m/s` and altitude intent
  was `None`, confirming the one-shot sequence completed.
- `cargo test --release --test brains --test locomotion`: 16 passed, one ignored
  timing benchmark. Both opt-in completion and unchanged default hold passed.
- `cargo clippy --all-features --lib --test brains --test locomotion -- -D warnings`
  passed.
- Focused IDE brain tests: five passed.
- With `--all-features`, the optimized brain/locomotion tests passed 17 tests
  (one timing benchmark ignored). The standard template compiled and its test
  target passed (zero tests). Rustfmt and `cargo doc --all-features --no-deps`
  passed. The rebuilt BehaviourTest binary validated the updated five-node brain.
- `cargo test --workspace --all-features --release` reached the existing
  `first_project` failures documented in `plan.md`: the authored cube spin test
  cannot find its entity, and offender bounce measures 0.2738215 m against its
  expected 4 m. The workspace suite is therefore not green.

Arrival is a position tolerance; it does not promise zero velocity at the instant
the sequence advances. Ground height is a flat kinematic floor; Cube has physics
disabled, so this run does not establish terrain collision or contact detection.
Restart the game to use the changed engine/assets. The IDE caches its Rust node
catalogue for its process lifetime; restart it to expose the new inspector fields.
