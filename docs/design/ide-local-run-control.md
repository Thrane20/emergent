# IDE local run control

## Scope

The first run-control slice lets the IDE start and stop the linked project's
existing Cargo development executable on the local host. It reports build,
running, completion, failure, and stopped states. It does not yet implement
visual project mode, pause/step, hot reload, target-output selection, packaging,
or remote execution.

For the current reference project the executable is headless and exits after
120 fixed ticks. A future game-owned graphics entry point can use the same local
run boundary without changing the IDE control contract.

## Ownership

- The linked game project owns its `Cargo.toml`, executable, authored assets,
  generated code, and runtime world.
- The IDE server owns one local child-process session at a time and exposes a
  narrow start/status/stop API. The browser never supplies a command or path.
- Cargo owns compilation and executable launch. The run controller invokes
  `cargo run --manifest-path <linked-project>/Cargo.toml` with the project as
  its working directory.
- Runtime entity and behaviour identity remains inside the launched process;
  no SlotMap or ECS handles cross into the IDE.

## Data flow and lifecycle

1. The browser reads run status when the studio mounts and polls while a run is
   active.
2. Run submits only the linked project ID. The server resolves the current
   project snapshot and its known Cargo manifest.
3. The server starts Cargo without a shell, captures bounded stdout/stderr, and
   moves through `building`, `running`, and terminal states.
4. Stop terminates the owned process group where the host supports it and marks
   the session stopped after the child closes.
5. The top-bar control displays the latest state and a bounded diagnostic tail.

## Safety and performance

- No client-provided executable, arguments, working directory, environment, or
  shell interpolation is accepted.
- Output is capped so a noisy game cannot grow IDE server memory without bound.
- Process polling is editor-only and stops in terminal states. No work is added
  to the engine's frame loop.
- A second run is rejected while one is active.

## Validation

- Verify the checked-in project tests and a direct local Cargo run.
- Exercise start, status polling, successful completion, and duplicate-start
  rejection through the API.
- Verify the top-bar state transition in the browser.
- Run IDE lint and production build.


## Selected-scene run (2026-10-24)

Run receives the scene picker's current path. /api/run validates linked-project
membership and a contained existing asset file, then passes
`-- --run-scene <project-relative-scene-path>` to Cargo. The manifest is unchanged.
Run snapshots record scenePath, and Stop targets the active run even if selection
changes. Unsaved/busy authored edits disable a new Run. Refresh retained manager
methods during development hot reload without losing the active process handle.

Verified with 34 IDE tests, lint/build, browser request checks for two selected
scenes, and a live selected-physics-scene launch through localhost:3000/api/run.
