# Airports and the control tower (AIR) — design brief

Builds on the physical fixed wing (`docs/design/fixed-wing-physics.md`) and the scene map (`docs/design/scene-map-editor.md`). The pilot already flies concrete goals: taxi to a point, take off, fly a circuit and land on a zone. This adds:

- **What an airport is**: tags on scene-map items.
- **Ground routing** along taxiways.
- **A tower** that turns "arrive here" and "depart from here" into those concrete goals, and keeps several aircraft apart and in turn.

## Describing an airport

Airports are ordinary scene-map items carrying metadata, using the `aeroway` key and OpenStreetMap's values (familiar, documented, and leaving room for more):

| Item | Map item | Tags | Meaning |
| --- | --- | --- | --- |
| Aerodrome | circle or rectangle zone | `aeroway=aerodrome`, optional `name`, `pattern_height` (m, default 300) | The airport's boundary. Aeroway items inside its footprint belong to it. |
| Runway | rectangle zone | `aeroway=runway`, optional `circuit=left\|right` (default left), `direction=forward\|reverse\|both` (default both) | Landing along the zone's local −Z (forward) or +Z (reverse). The threshold is the near end. |
| Taxiway | linked waypoints (a path) | `aeroway=taxiway` | The ground network. Routes are the cheapest path through it, so zone costs and blocks apply. |
| Holding position | waypoint linked into the taxiways, near a runway end | `aeroway=holding_position` | Departures wait here before entering the runway. |
| Stand | waypoint | `aeroway=parking_position` | Parking. Holds one aircraft. |
| Tower | any object | `aeroway=tower` | Marks the airport as controlled. Without one, aircraft still use the airport but do not sequence. |
| Holding fix | waypoint | `aeroway=holding_fix` (optional) | Where arrivals wait when the circuit is full. Defaults to a point 3 km out on the extended centreline. |

Without any aerodrome zone, every aeroway item in the scene forms one implicit airport. The scene-map editor gets an **Airport** tool group that places these items with the tags already set (Aerodrome, Runway, Taxiway path, Holding position, Stand, Tower). The inspector edits the tags like any other metadata.

## Orders and the tower

Game code or a brain gives an aircraft a high-level order. The pilot never sees it directly:

- `FlightGoal::Arrive { airport }`: join the circuit, land on the active runway, vacate, taxi to a free stand and park.
- `FlightGoal::Depart { airport, altitude }`: taxi from where it is to the holding position for the active runway, wait for the tower, line up, take off along the centreline and climb out to `altitude`. The order is then complete. The aircraft holds runway heading at `altitude` until it is given the next order.

Each fixed tick, before locomotion, the **tower** (`src/tower.rs`, a world resource) reads every aircraft with an order. For each one it writes a **dispatch**: the concrete goal the pilot flies this tick, plus two gates.

- **Active runway**: the first runway, or one tagged `active=true`, in its first permitted direction. Choosing it into the wind waits for wind (AIR-3).
- **Arrival sequence**:
  - Arrivals are numbered in the order they ask. A go-around goes to the back.
  - The first `CIRCUIT_CAPACITY` (4) fly the circuit.
  - The rest hold over the holding fix, stacked 150 m apart above circuit height, and descend into the circuit as places free up.
- **Spacing**:
  - Number *n* may **turn base** only once its remaining path to the threshold (along-runway distance plus base leg) is 2 km longer than number *n−1*'s, which is about 70 s, enough to land and vacate. Until then it extends downwind, which is what controllers ask pilots to do.
  - Each aircraft on downwind also slows to approach speed when it closes within 1.2 km of the one ahead.
- **Landing clearance**: given only while the runway is clear (no other aircraft on it, on the ground or below 30 m over it). Without clearance by 1 km from the threshold, the aircraft goes around and rejoins the back of the sequence.
- **Departures**:
  - They queue at the holding position in arrival order.
  - The first is cleared to line up and take off when the runway is clear and no arrival is on final within 3 km (or cleared to land).
  - The next one waits until the previous one is airborne past the runway end.
- **After landing**: the aircraft rolls out, then taxis to the nearest taxiway waypoint (vacating the runway), routes to the nearest free stand and parks. A stand is taken when assigned.
- **Uncontrolled airports** (no tower): the same procedures, with every gate open (no sequencing). Two aircraft can then conflict; that is the realistic consequence.

## Ownership and identity

- Airport items are scene entities. The tower refers to them and to aircraft by `EntityId`, validated each tick. A deleted runway or stand ends the affected orders explicitly: arrivals hold where they are, and departures stop taxiing.
- Tower state (sequence, stage per aircraft, stand assignments) lives in one world resource keyed by `EntityId`. Entries for missing aircraft or changed orders are dropped each tick.
- Airports are rebuilt from tags when the map generation changes (one pass over tagged items), never per aircraft.
- Pilot gates are plain values on the agent. The pilot still owns flight, and physics still owns the pose.

## Pilot additions

- **Circuit side**: a right-hand circuit mirrors the left-hand one.
- **`Taxi { point, stop }`**: `stop: false` keeps taxi speed through a route point.
- **`TakeOffFrom { runway, altitude }`**: take-off roll tracking the runway centreline.
- **Landing gates**: `may_turn_base` and `cleared_to_land` (both true for a plain `Land`).

## Stages

| Stage | Scope | Acceptance |
| --- | --- | --- |
| **AIR-1 airport model and ground** ✓ | Tags → airports, right/left circuits, taxi routing on taxiways, line-up and centreline take-off, vacate and taxi to stand, `Arrive`/`Depart` orders (uncontrolled), Airport map tools, brain nodes Arrive / Depart | One aircraft departs from a stand via the taxiways and holding position and climbs out. It arrives, lands, vacates and parks on a stand. A right-hand circuit flies on the right. |
| **AIR-2 tower** ✓ | Sequencing, holding stack, base-turn spacing, landing clearance, departure release, go-around re-sequencing | Five aircraft arriving together all land in turn with no go-around and no two on the runway at once. Departures and arrivals share the runway without conflict. A deleted runway is handled. |
| **AIR-3a wind and map overlay** ✓ / AIR-3b rig (later) | Uniform wind (airspeed relative to air), active runway into the wind, crabbed crosswind approaches, propeller and control-surface rig visuals | Lands in a 5 m/s crosswind on the centreline. The runway direction flips with the wind. |

## Hot path and scale

The tower costs O(aircraft + airport items) per tick, with no allocation in steady state (reused vectors). Routes are computed once per taxi leg with the map's cached A*. Expected scale is tens of aircraft per airport. It is measured with the 5- and 20-aircraft scenarios.

## Out of scope

Radio phraseology and UI, IFR approaches, multiple runways in use at once, intersection departures, ground collision avoidance beyond one aircraft per stand and runway, and traffic avoidance between aircraft outside the airport.

## Delivered: AIR-1 and AIR-2

**Code.**
- `src/tower.rs`: airports from tags, active runway, the `Tower` resource with its dispatch, sequencing, stands and routes.
- `src/fixed_wing.rs`:
  - right/left circuits (`Runway.right_circuit` from `circuit=right`);
  - `Taxi { point, stop }`;
  - `TakeOffFrom { runway, altitude }` with centreline tracking;
  - `Gates` (turn base, cleared to land, slow);
  - the `Arrive`/`Depart` orders.
- `src/locomotion.rs`: the tower runs first each locomotion tick, and `fly()` takes the tower's dispatch.
- `src/brain.rs`: **Depart airport** and **Arrive at airport** nodes.
- IDE: an **Airport** tool group in the scene-map editor (Aerodrome, Runway, Taxiway, Holding position, Stand, Tower) placing tagged items. A runway's long side becomes its landing axis.
- MCP: the instructions describe the tags and nodes.

**Validated.** `tests/airport.rs` (physics feature, a controlled 900 m runway airport with a parallel taxiway, an apron taxilane and five stands):

| Test | Result |
| --- | --- |
| Departure | From a stand: TaxiOut → HoldShort → LineUp → TakeOff → Departed, rolling within 6 m of the centreline. |
| Five simultaneous arrivals | Four fly the circuit while the fifth holds over the fix. All land in turn with **no go-arounds**, never more than one aircraft on the runway, then vacate and park on **five different stands**. All five are down in about 570 s. |
| Two departures and three arrivals | All complete, one on the runway at a time, no go-arounds. |
| Right-hand circuit | A runway tagged `circuit=right` is flown with downwind east of the runway. |
| Brain sortie | Depart airport → Arrive at airport flies a full sortie and parks. |

IDE: a unit test for the airport tool builds, and a browser check drawing a runway (east–west drag, so turned 90°), a taxiway and a stand, with the tags written to the scene and no console errors.

**Measured** (release, Apple M3 Pro):
- The five-arrival scenario, about 37,000 ticks, runs in 0.36 s including scene load: roughly 10 µs per tick for the whole engine with 5 aircraft, the tower and physics.
- `examples/fixed_wing_bench.rs` is unchanged at about 1 µs per aircraft (tower idle).

**Lessons from tuning.**
- "Turn base once the leader is within 1 km" cascaded into 20 km downwinds. Comparing remaining approach paths fixed it.
- Stands linked straight to one taxiway point made routes cut across parked aircraft. Aprons need a taxilane in front of the stands (as real ones have); the engine does no ground collision avoidance.

**Next (AIR-3).**
- Uniform wind: airspeed relative to the air, choosing the runway direction into the wind, crabbed crosswind approaches.
- Propeller and control-surface rig visuals.
- Then: a scene-map airport overlay (circuit legs and sequence), radio-style tower messages for game UI, and several active runways.

## Delivered: AIR-3a (wind and the map overlay)

**Wind.**
- A scene's `[wind]` table (`speed` m/s, `from` compass degrees) becomes the `Wind` world resource (`src/wind.rs`). Game code may replace it at runtime.
- Physical aircraft fly against the air: airspeed, lift, drag and side force use velocity relative to the wind, while wheels, taxiing, tracks and the glide path use the ground.
- Every air leg is a ground track. The pilot crabs by asin(crosswind ÷ airspeed), at most 30°. A go-around climbs out along the runway's course, not the nose heading.
- The turn onto final leads by the larger of the air and ground turn radii, so a tailwind on base doesn't overshoot.
- The tower lands into the wind on runways tagged `direction=both` (the default). It turns round only when the current direction has more than 2 m/s of tailwind (hysteresis).

**Map overlay.**
- `--map-layout` adds `wind` and `airports`: each runway's landing direction in use, and the active runway's circuit (final turn, threshold, far end, join, base turn), pattern height and holding fix. These come from the same `circuit_outline` geometry the pilot flies, for a default `fixed_wing_physics` aircraft.
- The scene-map editor draws:
  - the landing arrow and heading;
  - dashed Downwind, Base and Final legs with arrows;
  - the join point at pattern height and the holding fix;
  - a wind compass.

  An **Airport overlay** toggle and **Wind** controls (speed, from) write `[wind]` through the engine-validated scene edit.
- MCP gains `set_wind`.

**Validated.**
- **Crosswind:** a 6 m/s crosswind landing crabs about 12° on final, stays on the centreline and glide path, and lands with no go-around.
- **Headwind:** an 8 m/s headwind cuts the take-off ground roll by more than 25%.
- **Runway choice:** with wind from the south, the tower lands a northbound runway southbound, then parks.
- **Overlay geometry:** `airport_views` reports the southbound direction, an east-side left-hand circuit and a holding fix 3 km out.
- **IDE:**
  - the `[wind]` table writer;
  - an integration test that draws a runway and tower with the tools, checks the calm overlay (north, west circuit), sets the wind from the south (runway turns round, circuit moves east), and refuses out-of-range wind;
  - a browser check with no console errors.
- The airport suite (7) and the flight suite (13) pass.

**Fixed on the way.** The lighting editor's scene reads were still capped at 256 KiB from before scenes grew to 2 MiB.

**Still open.**
- AIR-3b: propeller and control-surface visuals.
- Wind for kinematic movers and drones (FLIGHT-4).
- Gusts and turbulence.
- Live traffic on the map while the game runs (the overlay shows the plan, not aircraft).
- Radio-style tower messages.
- Several active runways.
