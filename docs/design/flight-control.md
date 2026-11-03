# Flight-control implementation contract

FLIGHT-0 decisions, 2026-11-02. These settle the initial boundaries in the
[delivery plan](../plans/physics-locomotion.md); a decision is not a delivered pilot.

## Ownership and timing

The opt-in type is `drone_physics` (unpowered boundary). Existing `drone` profiles remain kinematic.
A physical drone requires an enabled dynamic `PhysicsBody` and the `physics`
feature. Physics owns mass, gravity, collider-derived inertia, pose and velocity;
the locomotion profile owns pilot/actuator limits and controller configuration.
Backend handles remain private, with the existing world-scoped SlotMap `EntityId`
as canonical runtime identity. A profile has no live integrator state.

Use one 60 Hz engine fixed step initially. Read the last committed plant state,
evaluate goal/controller, queue world-space forces and torques, then integrate
physics and publish results. Results become visible to the brain next tick.
Newly queued forces/torques must apply on the body's first reconciliation/step.
Before the first step, the pilot uses authored pose and zero initial velocities;
there is no inferred first-frame velocity from a target or command. Existing
velocity APIs are snapshots until the next reconciliation, so the pilot must
explicitly reset when a body is reconstructed instead of treating an old sample
as its new initial state.

Keep dynamic bodies at scene root or under the supported stationary identity
container. Reject unsupported parents/scales and competing writers. In particular,
`drive_body*` is not a flight actuator, and brain look must become a heading
request rather than a physical pose write. Feature-off scenes cannot fall back to
kinematic flight. FLIGHT-2 enforces these ownership rules; physical brain look goals are rejected
until the FLIGHT-3 pilot can turn them into heading requests.

## Delivered FLIGHT-2 boundary

A `drone_physics` profile currently has no actuator/controller fields. It loads
only with the physics feature, requires an enabled dynamic body and positive
uniform local scale, and executes no brain flight intent. Game code can apply
existing force/torque commands; the body falls if unpowered. The type catalogue
uses `status="boundary"`, distinct from `available` and `planned`. The editor
labels and disables it; Rust powered preview rejects it with an explicit message.
This keeps raw physical integration usable without advertising a finished pilot.

`World::physics_body_state(entity)` returns copied committed transform, measured
linear/angular velocity, actual mass, effective gravity and a reconstruction
revision. It returns `None` before initialization or when current settings/pose
no longer match the committed body. `PHYSICS_GRAVITY` is the single fixed backend
gravity constant; effective gravity includes the body's gravity scale. Existing
velocity-only APIs retain their last-committed snapshot semantics.

The revision is world-local change bookkeeping for the same EntityId, not object
identity or a serialization key. Body settings/scale rebuilds allocate a new
revision without reusing old feedback. The agent resets motion history when an
observation disappears or revision changes; ordinary observations preserve it.
Physical observations update before post-fixed observers. The optional snapshot
is boxed so legacy agents do not embed a full physical snapshot; it allocates on
initialization/reconstruction and updates in place during steady stepping.

Physical brain self/neighbour velocity comes from that observation, or zero
before initialization, rather than smoothed commanded velocity. Raw character
drive and competing controller/follow/spin/look pose writers are rejected. The
next pilot must extend the same reset hook to its PID/actuator history; this
boundary does not contain pilot state yet.

## Reference plant and authority

Reference: 1 kg, 1 m cube, world +Y up, gravity `[0,-9.81,0] m/s²`; physics derives
inertia from the box. Nominal level hover requires 9.81 N. Max collective thrust
is 25 N, max tilt 35°, climb/descent setpoint bound 2 m/s, horizontal setpoint bound
5 m/s, body-rate bound 2 rad/s. The initial motor response time constant is 0.08 s.
These are a synthetic calibration fixture, not measurements of a real aircraft.

Freeze a conservative aggregate authority envelope for FLIGHT-3. For commanded
collective T clamped to `[0,25] N`, differential headroom is `min(T,25-T) N`.
Roll/pitch moment bounds are headroom × 0.2 m; yaw moment bound is headroom × 0.02 m.
Normalised absolute torque requests share one summed budget ≤1. At either zero
collective or max collective, differential torque authority is zero. This avoids
granting independent unlimited lift and attitude authority without implementing a
rotor mixer. Apply actual thrust along actual body +Y and convert body moments to
world torque. Use finite motor response; do not rotate directly toward a setpoint.
Actual achieved actuator output feeds controller anti-windup in matching units.

## Goal/result semantics for the upcoming pilot

- One persistent movement goal per vehicle. Reissuing the same command revision
  retains control state; a new revision changes the goal. Heading is a separate
  bounded yaw request. Only one publisher owns movement; do not combine conflicting
  explicit goals through steering-tree velocity sums.
- Explicit flight goals take precedence over the legacy velocity adapter. Inside
  that adapter, altitude requests set vertical control, hover/brake hold measured
  position, and otherwise velocity is the command. Existing advisory target points
  do not implicitly become persistent position goals. No explicit goal plus no
  legacy motion input defaults to current-position hold while armed.
- Takeoff keeps the starting X/Z and targets world altitude. Fly-to then hold uses
  a world-space point. Hold captures the measured pose once on goal entry. An
  entity target resolves through its valid owning-world handle; deletion fails
  that goal and captures a safe current-position hold, never a replacement entity.
- Nominal settled completion: altitude error ≤0.1 m for takeoff, position error
  ≤0.2 m for fly-to, and speed ≤0.1 m/s, continuously for 0.5 s. Leaving the band
  before completion resets the timer. Completing a goal reports a latched result
  for that revision and continues stabilising the hold until superseded.
- Land holds horizontal position and requests a bounded 0.5 m/s descent. Touchdown
  requires an actual support contact with vehicle-facing normal dot +Y ≥cos(45°),
  speed ≤0.1 m/s and angular speed ≤0.1 rad/s for 0.5 s. Side-wall contact, passing
  an altitude and an airborne apex are not touchdown. Collider contact tolerances
  belong to physics; no global centre-height clamp is used by this type.
- Settled/landed results are distinct. Once touchdown is confirmed, reduce thrust
  and disarm; a completed airborne mission does not disarm implicitly. Cancel or
  target loss while powered enters measured-position hold. Explicit power loss
  removes thrust and torque and lets physics act.
- Continuous saturation longer than 1 s reports limited authority. The initial
  explicit-goal timeout is 30 s; expiry fails the goal and attempts powered hold,
  without inventing force to guarantee success. A failed hold in excessive wind
  remains visibly limited. These thresholds are reference fixture choices, not
  universal hardcoded mission policy for future vehicle types.

Reset PID and goal dwell history on scene/body reconstruction, teleport/reset,
disarm/re-enable and incompatible profile changes. Preserve controller history
across ordinary setpoint changes. Paused inspection performs no control step;
resume resets measurement derivative history and uses simulation time only.
Initial gain edits rebuild/reset controllers; bumpless live gain transfer is not
part of this slice. Arrival tolerance on old altitude nodes retains its existing
positional semantics.

## PID choice and numerical contract (FLIGHT-1)

Use the engine-owned `control::{Pid, PidConfig, PidInput, PidOutput}`. The focused
crate assessed in the plan would need timestep scaling, derivative filtering and
downstream actuator feedback adaptation. A small fixed-value implementation
provides that contract directly without another dependency or object registry.

`Pid::new` validates immutable configuration. `step(input, dt)` accepts explicit
simulation duration, setpoint, measurement, caller feedforward, optional measured
derivative and optional total achieved output from the preceding control interval.
All signals must be finite. Gains are nonnegative for positive actuator response;
reverse-response plants must convert signal direction at their boundary. Each axis
owns its own PID instance.

The derivative is on measurement, with a first-order backward-Euler low-pass filter;
an inferred first derivative is zero. I integrates using seconds and is bounded
in output units. Conditional integration prevents further internal saturation;
back-calculation tracks previous achieved minus requested output, with a stable
`gain*dt/(1+gain*dt)` recovery factor. Without external feedback, tracking uses the
previous internally clamped output. A zero I gain disables all integral authority.
Output reports P/I/D, requested and clamped output, internal saturation and tracking
error. The caller retains external actuator telemetry; internal saturation does
not claim to observe downstream allocation.

Failed input/arithmetic leaves the controller unchanged. Reset drops all history;
configuration changes construct a fresh validated controller. To freeze, do not
step; reset derivative history through `reset` on re-enable. Feedforward, gravity,
world identity, motor dynamics and quaternion attitude error are outside scalar
PID. Three scalar PIDs are not an Euler-angle attitude controller: the future pilot
computes quaternion attitude error and body-rate setpoints before rate control.

## Baseline and budgets

[FLIGHT-0 baseline](../benchmarks/flight-baseline.md) records the reproducible
kinematic, free-fall and constant-lift reference before controller integration.
Use the plan's stability gates and provisional 128-body budgets as initial
implementation targets: control/actuation P99 ≤0.5 ms, combined physics/control
P99 ≤2 ms at 60 Hz in the defined simple scene, zero steady controller allocations.
These budgets are frozen for the reference workload, not yet achieved by a full
pilot. Full flight acceptance and benchmark scene/contact coverage remain later
gates; a scalar PID helper is not a physics-drone capability.
