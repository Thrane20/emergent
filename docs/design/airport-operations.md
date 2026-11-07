# Airports and the control tower (AIR) — design brief

Builds on the physical fixed wing (`docs/design/fixed-wing-physics.md`) and the scene map (`docs/design/scene-map-editor.md`). The pilot already flies concrete goals: taxi to a point, take off, fly a circuit and land on a zone. This adds:

- **What an airport is**: tags on scene-map items.
- **Ground routing** along taxiways.
- **A tower** that turns "arrive here" and "depart from here" into those concrete goals, and keeps several aircraft apart and in turn.

## Describing an airport

Airports are ordinary scene-map items carrying metadata, using the `aeroway` key and OpenStreetMap's values (familiar, documented, and leaving room for more):

| Item | Map item | Tags | Meaning |
| --- | --- | --- | --- |
| Aerodrome | circle or rectangle zone | `aeroway=aerodrome`, optional `name`, `pattern_height` (full-size m, default 300), `scale` (0.01–10, default 1) | The airport's boundary. Aeroway items inside its footprint belong to it. `scale` makes a miniature airport (see below). |
| Runway | rectangle zone | `aeroway=runway`, optional `circuit=left\|right` (default left), `direction=forward\|reverse\|both` (default both) | Landing along the zone's local −Z (forward) or +Z (reverse). The threshold is the near end. |
| Threshold | rectangle zone over a runway end | `aeroway=threshold` | Landings aim from its outer edge and take-offs line up in it. When a runway has any, only the ends with one are used (overriding `direction`); one at each end lets the wind choose. |
| Taxiway | waypoint lines | `aeroway=taxiway` | The ground network. Lines need not be linked to each other: they join where they cross, and a line end joins another line within 15 m. Routes are the cheapest path through it, so zone costs and blocks apply. |
| Holding position | waypoint near a runway end | `aeroway=holding_position` | Departures wait here before entering the runway. If unlinked, it joins the nearest taxiway line. |
| Stand | rectangle zone (or waypoint) | `aeroway=parking_position` | Parking at its centre. Holds one aircraft. If unlinked, it joins the closest point on the nearest taxiway line. |
| Tower | any object, usually a rectangle zone | `aeroway=tower` | Marks the airport as controlled. Without one, aircraft still use the airport but do not sequence. |
| Traffic pattern | linked waypoint path, ending on final before a threshold box | `aeroway=circuit`, `leg=entry\|downwind\|base\|final`; waypoint `entry_from`/`entry_to`, `exit_from`/`exit_to` (compass °, clockwise), `altitude` (m above the runway) | Replaces the generated circuit for that runway end. Aircraft cross each point within its entry range and leave within its exit range. |
| Holding fix | waypoint | `aeroway=holding_fix` (optional) | Where arrivals wait when the circuit is full. Defaults to a point 3 km out on the extended centreline. |

Without any aerodrome zone, every aeroway item in the scene forms one implicit airport. The scene-map editor gets an **Airport** tool group that places these items with the tags already set (Aerodrome circle, Runway, Threshold, Stand and Tower boxes, Taxiway lines, Holding position). The inspector edits the tags like any other metadata.

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

## Delivered: drawn airports (thresholds, taxiway lines, box stands)

**Code.**
- `src/taxi.rs`: each airport's ground network, built from its items whenever the map generation changes. Drawn links count (one-way stays one-way); crossing lines are split and joined at the crossing; a taxiway line end within 15 m of another line joins it; unlinked stands and holding positions join the closest point on the nearest line. Edge costs use `World::segment_cost`. Dijkstra with reused buffers; building is O(segments²) once per map change.
- `src/tower.rs`: `runway_geometry` moves the threshold to a threshold box's outer edge for that landing end, so the pilot, the tower and the map overlay all use the shortened runway. `active_runway` uses only ends with a threshold box when any exist. Taxi routes come from the airport's `TaxiGraph`.
- IDE: Threshold, Stand and Tower are box tools; the Taxiway tool's hint describes auto-joining; MCP instructions describe the new tags.

**Validated.** `tests/airport.rs` (10 pass): the threshold box makes the runway land northbound from 20 m in (usable length 880 m); a departure from a box stand routes over four never-linked lines (a crossing and a 10 m end join) to an unlinked holding position and starts its roll inside the threshold box; an arrival touches down past the box and parks on a box stand. The seven earlier airport tests pass unchanged. IDE `scene-map.test.ts` (6 pass).

**Limits.** Moving taxiway items while the game runs does not move the network until the map changes. Aircraft park at a stand's centre with whatever heading they arrive on. Occupancy uses the usable (shortened) runway.

## Delivered: drawn traffic patterns

**Model.**
- `emergent.waypoint` gains optional `entry_from`/`entry_to`, `exit_from`/`exit_to` (compass degrees, clockwise, so 350→10 wraps through north) and `altitude`. Registry fields can now be `optional` (unset means no constraint).
- A pattern is a waypoint path tagged `aeroway=circuit`, followed along its stored links from the point nothing links to. It attaches to the runway end whose threshold box is nearest its last point. Each point's `leg` tag (entry, downwind, base, final) sets the phase flown towards it; unlabelled points carry the previous label. A missing altitude carries the previous one, starting at pattern height.
- `tower::Patterns` caches them per map generation; aircraft share them through `Arc<[PatternFix]>` (no per-tick allocation).

**Pilot** (`fixed_wing::Pilot::fly_pattern`).
- A point with an entry range is crossed on a committed course inside the range (the one nearest the direct bearing), tracking the line through it. Without enough room to turn onto it, the aircraft flies out to a point five turn radii back and returns. After two failed tries it flies straight at the point, so a pattern drawn too tight can't loop forever.
- After a point with an exit range, it turns onto the exit course nearest the next point before heading on.
- The tower's rules are unchanged because legs are labelled: downwind is extended past the last downwind point while turning base is held; clearance and go-arounds work as before. After the last point the normal final approach takes over, so the final point should sit on the extended centreline near the 3° glide path.
- A go-around rejoins the pattern at its first downwind point.

**IDE.** A **Traffic pattern** tool draws the path with default legs (entry, downwind…, base, final). The inspector edits the courses and altitude. With a pattern drawn, the airport overlay shows the landing direction but not the generated circuit.

**Validated.** `tests/airport.rs` (12 pass):
- An aircraft flies a pattern drawn on the side opposite the generated circuit. It loops once to cross the first downwind point on course, crosses the downwind end, base and final points within their ranges, lands with no go-around past the threshold box and parks.
- The overlay omits the generated circuit only when a pattern is drawn.

A unit test covers ranges wrapping through north. The full Rust suite and the IDE map tests pass. `fixed_wing_bench` is unchanged at about 1.1 µs per aircraft (release, Apple M3 Pro).

**Limits.** Turns start at a point, not before it, so points closer together than the aircraft's turn radius (about 300 m for the default profile) cause loops. The overlay doesn't draw the pattern's legs (its waypoints and links are on the map).

## Delivered: miniature airports

Game airports are often far smaller than real ones, while the circuit was sized for a real aircraft: at least 400 m out, 300 m up, with the holding fix 3 km away.

**Scale.**
- An aerodrome's `scale` tag (0.01–10, default 1) gives `Runway::scale` to every runway inside it. Aerodrome distances are full-size metres times the scale.
- Scaled with it:
  - the circuit's minimum spacing, pattern height (including an explicit `pattern_height`), touchdown aim cap and holding-fix distance;
  - the tower's spacing, slow-down, departure-gap and holding-stack distances, runway occupancy margins, line-up points and taxi reach;
  - the pilot's go-around, base-descent and clearance distances and its stabilised-approach gates (with floors a small aircraft needs);
  - the taxiway join distance.
- Not scaled: each aircraft's turn radius and speeds, which come from its profile. A miniature airport needs slow aircraft (e.g. lift-off at 6 m/s turns in about 18 m); a fast one still flies wide.

**Pilot changes that also apply at full scale.**
- Taxi speed is at most 40% of lift-off speed, so small aircraft can make their ground turns. This doesn't change the default profile, since 8 m/s is already lower.
- The stabilised-approach check for distance from the centreline applies only once the aircraft is within 30° of the runway heading. The glide-path check is unchanged. At miniature scale, the turn onto final starts below the height gate, and the old check sent every approach around.

**Map overlay.** The circuit is drawn for the widest-turning physical aircraft in the scene (the room the airport needs), falling back to the default profile when the scene has none.

**Validated.**
- `tests/airport.rs` (13 pass): a 20 kg aircraft lifting off at 6 m/s arrives at a `scale=0.1` airfield with a 90 m runway. It flies the circuit within 120 m of the runway and under 45 m high, lands with no go-around, taxis and parks on a box stand.
- A `map_tools` unit test (physics + graphics) shows the overlay circuit for a 0.1-scale field is over 500 m out for the default aircraft and under 100 m with the slow aircraft in the scene.
- The full Rust suite and the IDE map tests pass. `fixed_wing_bench` is unchanged at 1.1 µs per aircraft.

**Fixed on the way.** Zones drawn under a scaled or turned parent (such as a prefab whose root is an airport model at scale 4) were stored with their world size as local fields, so they came out larger by the parent's scale. They are now stored as `width`/`depth`/`radius` ÷ the parent's world scale, with its yaw taken off (`localZone`, the inverse of the map's `zoneShape`). The Runway tool's minimum width is now 1 m (was 10).

**Limits.** The overlay shows one aircraft's circuit; faster aircraft in the same scene fly wider. Climb-out still reaches 50 m before turning after take-off (the pilot doesn't know the airport's scale on departure).
