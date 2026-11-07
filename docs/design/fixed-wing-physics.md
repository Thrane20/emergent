# Physical fixed-wing aircraft (FW) — design brief

A rigid-body aircraft flown by forces and torques through Rapier, alongside the existing kinematic `fixed_wing` (which stays as it is). It reuses the physics boundary from FLIGHT-2 (dynamic body, committed state, reconstruction revisions) and `control::Pid` from FLIGHT-1. The drone pilot (FLIGHT-3+) is separate work: this brief adds a fixed-wing pilot, not a shared flight framework.

## Conventions

- The aircraft faces **−Z** in its own frame: **airspeed over the wing** is the velocity component along body −Z. Velocity is the world-space directional velocity; there is no wind yet, so airspeed is relative to still air.
- **Heading** is a compass bearing in degrees: facing world −Z is 0°, +X (east) 90°, +Z 180°, −X (west) 270°. This matches the scene-map editor, which shows north (−Z) up.
- Body rates use aircraft convention: **roll** p (right wing down positive, ailerons), **pitch** q (nose up positive, elevator), **yaw** r (nose right positive, rudder). Bank is positive right wing down.
- Units: metres, seconds, kilograms, newtons; profile angles in degrees, runtime in radians.

## Ownership and identity

- Profile type `fixed_wing_physics` ("Fixed wing (physics)") holds configuration only. Mass and the box collider live on the physics body; inertia is derived from them.
- Each aircraft's `LocomotionAgent` owns its pilot: flight phase, PID state, spooled throttle and telemetry, boxed so kinematic agents keep their small layout. Pilot history resets on body reconstruction (the committed-state revision changes), scene reload and goal changes that need it.
- The flight goal is a plain value on the agent, set by game code or derived from the brain's intent. A landing zone is named by `EntityId` and resolved each tick; a deleted zone fails the landing explicitly, and the aircraft goes around and holds.
- Only physics writes the aircraft's pose and velocity. The pilot only queues forces and torques; nothing teleports the body.

## Plant (forces and torques each fixed tick)

Weight W = m·g. Lift-off speed V_lo is the profile's **"speed at which wing lift overcomes gravity"**, defined at the rotation angle of attack.

- **Angle of attack** α = atan2(−v_up, v_forward) in the body's symmetric plane; **sideslip** β from the body-right component.
- **Lift** L = W · (V/V_lo)² · c(α), perpendicular to the airflow in the symmetric plane. c(α) is linear from a zero-lift angle of −3° (c = 1 at the rotation angle) up to the **stall angle**. Past stall, lift falls to 60% and drag rises, and the aircraft reports `stalled`. Negative α mirrors this, so inverted flight works.
- **Drag** = parasitic k_p·V² + induced k_i·L²/V². k_i makes induced drag 8% of weight at V_lo (so slow flight costs power). k_p is solved so that level flight at full thrust reaches the profile's **max speed**.
- **Side force** opposes sideslip.
- **Thrust** T = throttle · thrust-to-weight · W along the nose. The throttle spools towards the command at a finite rate, so **climb rate comes from excess power**: (T − D)·V / W. An underpowered aircraft cannot climb, and nothing adds hidden lift.
- **Control moments**: ailerons, elevator and rudder track commanded body rates, with angular acceleration limited by authority ∝ dynamic pressure (sluggish when slow) plus a little propeller wash on elevator and rudder. Torque = I·α_cmd with box-collider inertia. Weathervane stability in yaw and pitch, plus rate damping. A stall reduces elevator authority and adds a nose-down moment.
- **Ground** (supporting contact): the aircraft body must have physics friction 0, so contact is frictionless, and the pilot models the wheels:
  - rolling resistance 2% of W;
  - brakes up to 50% of W, never reversing the aircraft;
  - lateral tyre grip that stops sideways sliding;
  - nosewheel steering that yaws at taxi speed whatever the airspeed.

  A collider with friction 0 now combines as frictionless with any surface (Rapier otherwise averages it with the ground's).

## Autopilot

Cascaded loops, all on measured state:

- **Heading → bank**: the heading error sets a turn rate (0.5 rad/s per radian of error), converted to the bank that gives that rate at the current airspeed (atan(rate·V/g)), limited by **max bank**. Scheduling with speed keeps slow aircraft from wing-rocking.
- **Climb demand** is capped at 80% of what spare thrust sustains at the current speed, so a steep climb never bleeds the aircraft to the stall.
- **Bank → roll rate** command.
- **Altitude → climb rate** target, within max climb and max descent.
- **Climb rate → pitch**: a PID with feed-forward of the flight-path angle plus the angle of attack needed for lift at the current speed and bank. **Raising and lowering the nose** is this loop.
- **Pitch and turn rate → body rates**: Euler-to-body conversion, so a banked turn holds altitude. The rudder adds turn coordination and cancels sideslip.
- **Airspeed → throttle**: a PID with feed-forward from the estimated drag.

**Max bank** limits autopilot turns and manual roll. **Max bank 0 means unlimited**: manual sticks can roll and loop the aircraft (aerobatics). Autopilot turns then use 60°.

## Phases

`Parked` → `Taxi` → `TakeoffRoll` → `Rotate` → `Climb` ("taken off") → `Cruise` → `PatternEntry` → `Downwind` → `Base` → `Final` → `Flare` → `Rollout` → `Parked`. There is also `Manual` (stick control).

- **Takeoff roll**: full power, brakes off, nosewheel holds the runway heading, elevator neutral until the airspeed over the wing reaches V_lo.
- **Rotate**: pitch to the rotation angle until the wheels leave the ground.
- **Climb**: wings level at the climb rate until near the target altitude, then **Cruise** (hold heading, altitude and speed, or fly to a point and circle it).
- **Landing** on a nominated rectangular scene-map **zone**: its long side, along the zone's local −Z, is the landing direction, and its near end is the threshold.
  - Enter the traffic pattern at a 45° entry to downwind at pattern height, flying a left-hand circuit.
  - Fly downwind parallel to the runway, then **base leg**, then **turn to final**.
  - On final: nose slightly up at the approach attitude, airspeed from pitch, and **power PID-controlled to hold a 3° glide path** to the touchdown point.
  - Flare a few metres up (idle power, more nose up), touch down, and brake to a stop inside the zone (**Rollout**).
  - If it can't land in the zone (too high, too fast, or the zone was deleted), it goes around.

## Interfaces

- Rust: `FlightGoal` (Park, TakeOff, Hold heading/altitude/speed, FlyTo, Manual sticks, Land zone, Taxi) set on the agent, plus `FlightTelemetry` (phase, airborne, airspeed, heading, altitude, climb rate, pitch, bank, α, β, throttle, brake, lift, thrust, drag, stalled), readable each tick.
- Brain adapter when no goal is set: a moving intent becomes FlyTo (taking off first if on the ground) at the intent's altitude. Brake or hover on the ground becomes Park.
- Activity (speed, throttle, brake, bank, pitch, yaw) stays filled in for the wheel rig, visuals and sound.

## Stages

| Stage | Scope | Acceptance |
| --- | --- | --- |
| **FW-1 plant and flight** ✓ | Profile type, plant, ground model, autopilot loops, Park/TakeOff/Hold/FlyTo/Manual goals, phases Parked→Cruise, brain adapter, telemetry, frictionless-zero collider rule | Rolls, lifts off at about V_lo and not much before; underpowered aircraft can't take off; climbs at a rate set by power; holds altitude ±3 m and compass heading ±3° in level and banked turns within max bank; manual sticks roll past 90° only when max bank is 0; stalls when too slow; reconstruction resets the pilot |
| **FW-2 circuit and landing** ✓ | Taxi, pattern entry, downwind, base, final with power PID on the glide path, flare, touchdown and rollout inside a zone, go-around | Lands from a cold start and stops inside the zone; glide-path error bounded on final; touchdown pitch ≥ 0 (no nose-first landing) and vertical speed below a limit; deleted zone → go-around and hold |
| **FW-3 authoring** ✓ | Locomotion editor fields and preview (take-off, circuit), MCP preview, brain nodes (Take off, Land on zone), scene-map zone picker | Create, preview and save in the IDE; MCP preview reports phases and touchdown point |

## Hot path and scale

Per aircraft per tick: a few quaternion rotations, about six scalar PIDs and two queued force submissions. No allocation. The pilot is boxed per agent. A reference budget for many aircraft is set in FW-3, with measured numbers.

## Out of scope for now

Gusts (uniform wind arrived with AIR-3a: see the airport design), flaps, propeller torque and P-factor, ground effect, crosswind landings, runway surfaces other than flat ground, and multi-engine asymmetry.

## Delivered: FW-1 and FW-2

`src/fixed_wing.rs` (plant, wheel model and pilot), `Model::FixedWingPhysics` and `fly()` in `src/locomotion.rs`, and the frictionless-zero collider rule in `src/physics.rs`. `tests/fixed_wing_physics.rs` (physics feature, 60 Hz, a 1 t aircraft with a 10 × 8 m box, default profile) has 9 tests:

- **Take-off.** Parked stays put. The take-off roll starts rotation at 0.9 × V_lo, lifts off at about 23–24 m/s (V_lo 22), climbs out at ≥ 1.25 × V_lo and power-limited 5 m/s, then holds 150 m ± 3 m and heading 0° ± 3° at 40 m/s.
- **Turns.** Turning to 270° (west) stays within max bank 30° and loses less than 10 m.
- **Power.** Thrust-to-weight 0.03 never reaches lift-off speed.
- **Manual.** Manual roll passes 120° only when max bank is 0, and stops at about 45° otherwise. Power-off with the stick back stalls.
- **Fly-to.** It circles the point.
- **Taxi.** It reaches a point at ≤ 9 m/s and stops within 6 m.
- **Landing on a 900 × 30 m zone, from a cold start, with no go-around:** take-off, pattern entry, downwind at 300 m, descending and extending downwind, base, final on the centreline with a glide-path error under 1 m below 60 m height and power around 20% from the PID, a flare from 5 m to 4.8° nose up, touchdown at −0.7 m/s about 185 m past the threshold, and a stop on the centreline inside the zone. All phases are asserted.
- **Deleted zone.** It holds rather than lands.
- **Loader rules.** Friction 0 and a box collider are required, and the speed ordering is checked.

**Approximations to know.**
- The box rotates about its tail edge, so on the ground the pilot adds the main-gear pivot moment while the elevator pulls up.
- Inertia comes from the box collider.
- Gyroscopic coupling is ignored.
- The circuit is always left-hand, and the downwind leg is laid out from the approach turn radius (minimum 400 m).
- Go-around triggers: unstable below 60 m (more than 40 m off the centreline or 20 m off the glide path), slower than 1.05 × V_lo, past 40% of the runway on final, or floating past half the runway in the flare.

**Not measured yet.** Per-aircraft tick cost (expected to be small: one pilot step plus two queued forces). FW-3 sets a budget and measures it.

## Delivered: FW-3

- **Editor preview.** `simulate` flies `fixed_wing_physics` profiles with Rapier on a 6 km field with a 900 × 30 m runway zone. Courses:
  - Waypoints: a 2.4 km square at 150 m.
  - Orbit.
  - Launch: take off and climb.
  - Stop: taxi 150 m.
  - Circuit (new): take off, fly the pattern and land.

  Aircraft runs last up to 300 s, at 10 frames a second. The trace adds a 15th column, the flight phase (`FlightPhase` as u8, append-only order). The Locomotion editor defaults aircraft to Circuit and 240 s, and shows a Phase readout. A browser check ran 240 s and returned 2400 frames, with the circuit track, height profile and rig drawn, and no console errors.
- **Brain nodes** (Locomotion category):
  - **Take off** (height; *finish at height* advances a Sequence once within *arrival tolerance*, default 0.5 m).
  - **Land** (target zone object, or the nearest zone in a group; circuit height).

  They publish `Intent.flight`. Game code's `LocomotionAgent::flight` wins over it, and the movement adapter is the fallback. A Sequence of Take off (200 m) then Land flies a full sortie in `tests/fixed_wing_physics.rs`.
- **MCP.** `preview_locomotion` accepts `circuit` and up to 300 s, and for aircraft reports `phases` (in order) and `touchdown` (time and position). The instructions describe the type and the nodes. `list_locomotion_types` and `list_brain_nodes` come from the engine catalogue. `tests/mcp-vehicles.test.ts` creates a profile with `write_locomotion` and previews a circuit through a template-built game.
- **Measured** (`examples/fixed_wing_bench.rs`, release, Apple M3 Pro, 60 Hz, aircraft spread out in cruise):

  | Workload | Cost |
  | --- | --- |
  | Pilot step alone | 93 ns |
  | 1 aircraft | 3 µs per tick |
  | 32 aircraft | 33 µs |
  | 128 aircraft | 127 µs |
  | 256 aircraft | 255 µs |

  That is about 1 µs per aircraft including Rapier, with no contacts. Ground contact, many aircraft on one runway, and collisions between aircraft are not measured.

**Still open.** Wind and crosswind landings, right-hand circuits, a runway picker in the scene map, a Taxi brain node, and visual control surfaces or propellers. Brain nodes give way to game-code goals; nothing arbitrates between several brains that each publish flight orders (the first one wins, as with altitude).
