# Locomotion: how objects move

Status: **first slice implemented** (2026-10-31): runtime, six types, brain intent, editor. Builds on [brains](brain-editor.md) and
[object components](object-components.md).

## Goal

A brain decides **what** an object wants ("go there", "hold", "flee", "orbit that
point"). Locomotion decides **how** this particular object does it, within what it is
physically able to do: a car steers its front wheels and brakes; a character walks;
a plane banks into a turn and cannot stop in mid-air; a drone tilts, hovers and yaws;
a rocket burns along its axis; a shell follows a ballistic arc. The Locomotion page
builds and tunes these movement profiles, previews them, and saves them for the whole
project.

## Model

**Locomotion profile** — `assets/locomotion/<id>.locomotion.toml`: id, name, description,
a **type**, the type's parameters (limits and tuning), and an optional **rig** listing
actuators (wheels, rotors, engines, control surfaces, legs) used for visuals and
per-actuator tuning. Saved for the whole project, like brains.

**Intent** — what every brain (or the character controller, or game code) produces each
tick: a desired velocity, plus optional extras the brain node can set: a target point,
a facing, `brake`, `hover`, `boost`. Today brains output only a velocity; this adds a
small `Intent` struct alongside it so nodes like Hold can say "brake" rather than "zero".

**Locomotion controller** — per object, `emergent.locomotion` component (profile + per-
object overrides). Each fixed tick it turns intent into motion under the type's limits
(top speed, acceleration, turn rate or radius, climb rate, fuel, …) and exposes the
**activity** behind it (steer angle, throttle, brake, rotor speeds, bank, pitch, gait
phase, thrust) so visuals and sound can follow. Without the component, objects keep
today's direct movement.

**Manoeuvres** — the named activities a type supports (steer, brake, move to, hover,
spin, orbit point, take off, land, climb, burn, intercept, ballistic launch). Brains
request them through new Brain nodes in a "Locomotion" heading (e.g. *Orbit point*,
*Land*, *Burn*); the profile decides how they are carried out.

## Types (library headings → types). ✓ = first slice, ○ = later

1. **Ground** — Slide (force/acceleration on a point mass; today's behaviour, made
   explicit) ✓, Wheeled — car/bicycle model with steering angle, wheelbase, brakes ✓,
   Tracked / skid-steer (tanks) ○, Hover-craft ○.
2. **Legged** — Biped IK walk ○ (needs skeletons and an animation system, which the
   engine does not have yet), Quadruped ○, Crawler ○. Until then a legged profile moves
   like Slide with gait timing exposed (step phase, stride) for animation later.
3. **Rotorcraft / VTOL** — Multirotor drone (tilt to move, hover, yaw, altitude hold) ✓,
   Helicopter (collective, cyclic, tail-rotor yaw) ○, Tilt-rotor ○.
4. **Fixed wing** — Aircraft with stall speed, bank-to-turn, climb/sink rates, take-off
   and landing runs ○ (first after the ✓ set).
5. **Projectiles & propulsion** — Ballistic (gravity, drag, launch to hit a point) ✓,
   Rocket (thrust along axis, fuel/burn time) ✓, Guided missile (pursuit /
   proportional navigation) ○, Bomb drop (release point solving) ○.
6. **Water** — Boat ○, Submarine ○.

## Runtime

Rust `src/locomotion.rs`, same pattern as brains: type catalogue exported for the IDE
(`--export-locomotion`), validation, per-object state in a component, one pass per
fixed tick after brains and before physics. The implemented first slice uses
**kinematic models** (including locally integrated point-mass projectile dynamics)
that write the transform directly. They do not drive Rapier dynamic bodies;
the scene loader rejects that competing pose ownership. An opt-in force-driven
drone, reusable PID control and physical contact landing are planned in the
[physics locomotion fit/gap](../plans/physics-locomotion.md), with runtime progress
3/8 (baseline/contract, PID and the unpowered physical boundary). The explicit
`drone_physics` profile requires a dynamic body and has catalogue status `boundary`;
the editor disables it and powered preview rejects it until the pilot is delivered. Wheel ray-casts, tyre friction and
aerodynamic surfaces remain later work.

## Locomotion page (`/locomotion`)

Laid out exactly like the Brain page: heading **Locomotion** with a carousel of saved
profiles; left **Type library** (headings above, planned types disabled); centre
**Rig** view (the body with its actuators: wheels, rotors, engines, legs, as a tree)
and **Preview** (Rust simulation of the profile following a scripted course — waypoints,
a figure-eight, a hover-and-orbit, a launch at a target — top-down and side views, with
activity traces such as steer angle and throttle); right **Inspector** (type
parameters, selected actuator, manoeuvres it supports); **Save profile**; AI chat scoped
to the profile.

## Validation

Per type: limits respected (a car never turns tighter than its minimum radius; a plane
never flies below stall speed except landing; a drone holds altitude within tolerance),
arrival at targets, ballistic launch hits its target within tolerance, deterministic
replay, and browser checks of the page as for brains.

## Decisions (2026-10-31)

- Kinematic models first; full physics vehicles later behind the same profile.
- First slice: page, profile format and runtime for Slide, Wheeled, Drone, **Fixed wing**,
  Ballistic and Rocket. Legged/IK waits for skeletons and animation.
- Extend brain intent: brake, hover, boost and a desired altitude, plus Brain nodes for
  them; Hold also means brake.

## As built (2026-10-31)

- **Engine** — `src/locomotion.rs`: type catalogue (6 available, 11 planned, with
  manoeuvres and rig kind) exported by `--export-locomotion`; `LocomotionProfile` load and
  validation (`--validate-locomotion`); `LocomotionAgent` component with the model, the
  latest `Intent` and the published `Activity` (speed, steer, throttle, brake, bank, pitch,
  yaw, rotor, thrust, fuel, landed). Models: Slide, Wheeled (pure-pursuit steering,
  bicycle model), Drone (tilt-limited acceleration, altitude hold, hover), Fixed wing
  (stall floor, bank-to-turn, climb limits, loiter on hover), Ballistic (low/high arc
  solve to the target), Rocket (burn, turn towards the target, coast). Stepped after
  brains in the motion phase; the heading starts from the authored rotation.
- **Brain intent** — `brain::Intent` (velocity, target, brake, hover, boost, altitude).
  Composites pass it up and Blend merges it; seek/arrive/pursue/attack/patrol/return home
  set the target; Hold brakes. New Brain heading **Locomotion**: Brake, Hover, Boost
  (decorator) and Altitude (decorator). Priority now treats a brake or hover request as
  "wants something", so Hold under Priority stops the object instead of falling through.
  Objects without locomotion follow the velocity as before and climb to a requested
  altitude.
- **Scenes** — `emergent.locomotion` component (profile). Rejected with a dynamic physics
  body (kinematic models own the transform) and with character controller, follow or spin.
- **Host** — `--export-locomotion`, `--validate-locomotion`, `--simulate-locomotion`
  (courses: waypoints, orbit, stop, launch) in `src/project_host`.
- **IDE** — `/locomotion` (rail: Locomotion), laid out like the Brain page: profile
  carousel with rig thumbnails, type library (all headings open, planned disabled), Rig
  view (actuators driven by activity) and Preview (top-down track, height over time,
  live rig and readouts), settings inspector with **Save profile** (Rust-validated, project
  asset), AI chat scoped to the profile. Starter profiles: Runner, Hatchback, Quadcopter,
  Light aircraft, Artillery shell, Rocket. The scene inspector picks profiles for the
  Locomotion component; AI-edited profiles are validated before publishing.

Validation: `tests/locomotion.rs` (catalogue and validation errors; a car never turns
tighter than wheelbase ÷ tan(lock) and brakes to a stop; a drone holds 6 m, hovers at the
stop point and never tilts past its limit; an aircraft never stalls or over-banks, holds
25 m and loiters on hover; shells land within 0.6 m of the target on low and high arcs; a
rocket homes within 3 m and never burns past its burn time; identical replays; a brain
drives a car in a scene and Hold brakes it), launch-angle and heading unit tests,
`tests/locomotion.test.ts`, and a browser check of the page (library, carousel, previews
of the hatchback and rocket, creating and saving a drone profile).

### Completing an altitude step

Altitude normally holds its world-space height while its child is active. Hover
never finishes, so an Altitude → Hover step previously blocked a Sequence forever.
Altitude now exposes `finish_on_arrival` (default `false`) and `arrival_radius`
(default `0.1 m`, minimum `0.01 m`). When enabled, the step returns inactive once
the object's centre is within that vertical distance of `height`, allowing its
Sequence to advance. This is arrival within a positional tolerance, not a test
for zero velocity or ground contact; locomotion still limits and brakes motion.
Existing altitude-holding brains keep their behaviour. The catalogue supplies the
fields to the editor inspector and AI without a second editor-side schema.

For a 1 m cube on a flat ground plane at y=0, landing centre height and the drone
profile's `ground_height` are both `0.5 m`. Drone ground height is a kinematic
floor, not terrain collision. `tests/brains.rs` covers an opt-in takeoff/landing
sequence, completed one-shot state, unchanged continuous hold and invalid zero
tolerance. [BehaviourTest run evidence](../platform/drone-flight-macos.md).

Not yet: tracked, hovercraft, helicopter, tilt-rotor, guided missile (proportional
navigation), bomb drop, boats, legged/IK (needs skeletons and animation), reversing for
wheeled vehicles, take-off and landing runs, rendering the rest of the activity on scene objects (wheels are done: see the wheel rig below; rotors, control surfaces and brake lights are not).


## Wheel rig (LOCO-WHEELS) — brief

**Goal.** A car built from a body and wheel child objects shows its locomotion: front wheels turn with the steering angle and every wheel rolls with the ground speed. Brake lights wait for runtime material overrides, which the renderer does not have yet.

**Authoring.** Three optional fields on the existing `emergent.locomotion` component, so the inspector, scene files and MCP `set_component` handle them with no new tool:
`steering_wheels` (instance IDs: steer and roll), `rolling_wheels` (instance IDs: roll only), `wheel_radius` (metres, default 0.35). Wheels are picked per object in the scene, not in the profile, because a profile is shared by many cars and does not know their children.

**Validation (scene loader).** Each wheel must be an instance in the same scene, a descendant of the locomotion object (not the object itself), listed once across both lists, and must not carry its own mover (locomotion, spin, look at, follow, follow camera, character controller). Errors name the instance and the wheel.

**Ownership and identity.** The scene file stores instance IDs. On instantiation they map to `EntityId`s in a `WheelRig` component on the car: per wheel its id, whether it steers, and its authored local rotation (its rest pose). No new identity or hierarchy model. A deleted wheel is a stale handle and is skipped, never re-resolved.

**Runtime.** In the locomotion fixed tick, right after the car moves: the rig's roll angle advances by `speed × dt ÷ radius` (wrapped to ±π), and each wheel's local rotation becomes `yaw(steer) × pitch(roll) × rest`, applied in the car's frame. This works whatever way the wheel mesh was modelled, as long as the car faces −Z (the locomotion convention). Only rotation is written; translation and scale stay authored. Work is O(wheels) with no allocation per tick.

**Out of scope.** Suspension travel, per-wheel ground contact, skid marks, reversing (the wheeled model never reverses yet), brake lights, and auto-detecting wheels by name.

**Validation.** Loader tests for each rule; a scene test where a brain drives a car and asserts the front wheels hold the agent's steer angle, the rear wheels don't steer, roll matches distance ÷ radius, and a deleted wheel is skipped. The IDE inspector picks wheels with the existing multi-object picker.

**Delivered.** As briefed. `WheelRig` lives in `src/locomotion.rs`; the loader rules in `src/scene.rs`. `tests/locomotion.rs` covers every loader rule and a driven car: front wheels match `yaw(steer) × roll × rest`, rear wheels roll only, authored translation is kept, roll equals path length ÷ radius, and a deleted wheel is skipped. IDE `tests/mcp-wheels.test.ts` marks wheels through MCP `set_component` with instance refs and checks a non-child wheel is refused with the scene untouched.
