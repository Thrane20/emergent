# Brain modes

Status: **slices 1–4 delivered 2026-11-03** (engine, flight nodes, IDE panels, MCP); runtime mode
switching from the IDE is open (see below). Extends the [brain editor](brain-editor.md)
and the `emergent.brain` component's "Also run" stack.

## Goal

One object carries several named brains ("modes"), e.g. an aircraft with Park, Taxi, Take off,
Hold, Mission and Land. Any number of modes may be active at once. A mode that finishes hands
over to its **on next** mode; a mode that is aborted falls back to its **on previous** mode.
Modes are switched by the mode finishing, by brain nodes (including in reaction to a signal from
another object), by game code and by the IDE/MCP.

Example: a parked plane is told "take off" → Taxi (to the holding point) → Take off → Hold or
Mission. A rejected take-off goes back to Taxi.

## Decisions (agreed 2026-11-03)

1. **on next** runs when a mode completes; **on previous** runs when it is aborted/fails.
2. Handing over deactivates the mode that hands over (the default; a per-mode `keep_running`
   option keeps it on).
3. Mode switches come from three sources: signals between objects (via brain nodes), game code
   and the IDE/MCP.
4. A mission is an ordinary brain (e.g. a Sequence of Fly-to waypoints); no new asset type.

## Authoring (scene TOML)

```toml
[[entities.components]]
type = "emergent.brain"
max_speed = 30
[[entities.components.modes]]
name = "park"     # unique within the component; 1–32 chars [a-z0-9_-]
brain = "assets/brains/park.brain.toml"
active = true     # active when the scene starts
on_next = "taxi"  # optional
[[entities.components.modes]]
name = "taxi"
brain = "assets/brains/taxi.brain.toml"
on_next = "takeoff"
[[entities.components.modes]]
name = "takeoff"
brain = "assets/brains/takeoff.brain.toml"
on_next = "hold"
on_previous = "taxi"
```

- `on_next` / `on_previous` name another mode of the same component; unknown names, duplicate
  names and self-references are load errors.
- **Compatibility:** `brain` + `stack` keep working. They load as modes named `main`, `stack1`…
  that are all active and have no transitions. Saving from the IDE writes `modes` only when
  modes are actually used (any transition, inactive mode or rename), so existing scenes don't
  churn.
- New component field kind `Modes` (an ordered array of tables, max 16 modes) with
  validation in `component.rs`, as for `Pairs`.

## When is a mode finished?

Today a node returning nothing means either "done" or "condition not met right now". Modes need
the difference, so completion is explicit:

- The mode's root is a **non-looping Sequence that has run past its last child**, or
- the root is a **finishing leaf** that reports arrival (`take_off` / `depart_airport` with
  *finish at height*, `altitude` with *finish on arrival*), or
- a **Next mode** node runs (see below).

Anything else that returns nothing is idle, not finished. A mode with no `on_next` that finishes
just deactivates.

## New brain nodes (category "Modes")

| Node | Effect |
| --- | --- |
| **Next mode** | Completes the mode it is in (runs `on_next`). |
| **Previous mode** | Aborts the mode it is in (runs `on_previous`). |
| **Switch mode** (`mode`, `exclusive`) | Activates the named mode; `exclusive` also deactivates every other mode. |

Put under *On signal* they turn signals from a tower or player object into commands, e.g.
`On signal "cleared_takeoff" → Switch mode "taxi"`. A node naming a mode the object doesn't have
does nothing. (Not yet reported by validation: a brain is shared between objects with different
modes, so the check belongs to the scene validator.)

Flight nodes (category Locomotion):

| Node | Effect |
| --- | --- |
| **Taxi** (target/group, `stop`, `finish_on_arrival`, `arrival_radius` 5 m) | `FlightGoal::Taxi` to the point; finishes once within the radius (and below 0.5 m/s when it stops). |
| **Hold course** (`keep_heading`, `heading`, `height`, `speed`) | `FlightGoal::Hold`; *keep heading* captures the way the object faces when the node starts (brain snapshots now carry each object's forward axis); speed 0 uses the profile's cruise speed. Never finishes. |

## Runtime and ownership

- `BrainAgent.layers` becomes the mode list: each `Layer` gains `name`, `active`, and
  `on_next` / `on_previous` as **resolved layer indices** (`Option<u8>`) fixed at load. Names
  exist only in files and the API; ticks never compare strings.
- Mode requests produced during a tick (from nodes) are queued in the agent as small
  `(kind, index)` entries in a reused buffer, and applied **after** all layers are evaluated
  that tick. A request takes effect on the next tick, like signals. This keeps one tick's
  evaluation order-independent and makes replays deterministic.
- When a mode is activated its node state resets (fresh Sequence, timers). Deactivating keeps
  nothing.
- Merging is unchanged: movement adds up, the first look/flight order wins in **list order**, and
  every signal is sent. Authors put the controlling mode first. A mode that is not active costs
  one branch per tick.
- Requests applied in one tick are bounded (at most one transition per mode per tick), so chains
  can't loop within a tick.

**API** (entity ID + mode name at the boundary; a stale entity or unknown mode returns an error,
never a different object):

- `World::set_brain_mode(id, name, active: bool)`
- `World::switch_brain_mode(id, name)` (exclusive)
- `World::brain_modes(id) -> impl Iterator<Item = (&str, bool)>`

## IDE and MCP

- **Brain component card:** the main Brain and "Also run" fields are replaced by a list of
  collapsible mode panels: name, brain picker, *Active at start*, *On next*, *On previous*,
  *Keep running*, plus **Add new**. Collapsed panels show the name, brain and an active dot.
- **Not built: live state and manual switching.** The inspection session never ticks (it fails
  if gameplay advances), so modes there are always the start state; the running game has no
  control channel from the IDE yet. Both need that channel first; `World::set_brain_mode` is
  ready for it.
- **MCP:** `set_component` accepts `modes` (brain refs inside mode tables resolve to paths) and
  `describe` lists them; brain `usedBy` includes mode references. The `set_brain_mode` tool waits
  for the same run-time channel.

## Validation

- Unit: load errors (unknown/duplicate/self names), legacy `stack` loading as modes, completion
  rules, Next/Previous/Switch nodes, request timing (next tick), no mode switching more than once
  per tick.
- Integration (`tests/fixed_wing_physics.rs`): park → taxi → take off → hold driven by a
  signal from a second object; an abort returning to taxi.
- Performance: the `brains` benchmark at 256 agents with 4 modes each (1 active) against today's
  baseline; inactive modes should add a negligible amount, which will be measured, not assumed.
- `--validate-brain` and the scene validator report mode errors; IDE smoke for the mode panels.

## Slices

1. **Delivered.** Engine: mode list, completion, requests, Next/Previous/Switch nodes, API,
   legacy loading, tests. The IDE accepts and round-trips `modes` and shows them read-only.
   Measured (release, Apple M3 Pro, 2026-11-03, `measure_inactive_mode_cost`): 256 flocking
   agents 0.365–0.394 ms/tick with one mode vs 0.375–0.379 ms/tick with three extra inactive
   modes, i.e. within run-to-run noise. As built: `World::brain_modes` returns owned names.
2. **Delivered.** Taxi and Hold course nodes; `tests/fixed_wing_physics.rs` flies park → (tower
   signal) → taxi 80 m to a holding point → take off → hold course at 80 m on the start heading.
   The abort path is covered by `tests/brains.rs`, not yet by an aircraft scenario.
3. **Delivered.** Brain card mode panels (`brain-modes-editor.tsx`, `lib/brain-modes.ts`): name,
   brain, Active at start, On next, On previous, Keep running, move up/down, remove, Add new;
   renames follow links, removals clear them, plain lists save in the legacy form, open panels
   survive auto-save. Verified in headless Chrome on a disposable template project (rename,
   add, link, auto-save, engine validation of the written scene).
4. **Delivered.** MCP `modes` via `set_component`, `describe`, `usedBy`; instructions updated
   (`tests/mcp-brain-modes.test.ts`).
