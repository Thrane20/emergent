# Iteration 5 — Behaviour graph foundation

Status: complete.

## Outcome

Add the first behaviour graph that makes Emergent entities influence one another through typed links. Behaviour identity and link ownership remain SlotMap-based. Behaviour evaluation runs once per fixed tick, reads the previous committed output snapshot, stages new outputs and transform writes, then commits them at a defined boundary.

The reference demo becomes a leader and follower chain driven by behaviours rather than manual transform mutation in `Game::fixed_update`.

## Ownership and storage

- `World` owns one `BehaviorGraph` alongside its entity SlotMap and Bevy component world.
- Each behaviour has a world-scoped typed `BehaviorId`, one owning `EntityId`, one built-in behaviour kind, and one committed output in this initial slice.
- The graph keeps stable insertion order for diagnostics and deterministic iteration. Tick semantics do not depend on that order because every link reads the previous output snapshot.
- Entity subtree deletion removes behaviours owned by deleted entities. Links from surviving behaviours become explicitly unavailable; stale behaviour generations never resolve to replacements.
- An entity may own multiple behaviours, but at most one enabled behaviour may write its transform during this iteration.

## Typed ports and links

The initial value types are `f32`, `Vec3`, `bool`, and `EntityId`. Ports use static identifiers and declare a value type. Connecting a link validates source and target worlds, behaviour existence, port existence, direction, and type compatibility before changing the graph.

Initial built-in behaviours:

- `LinearMotion`: no input; advances a position by velocity and publishes `Vec3` on `position`.
- `FollowPosition`: consumes `Vec3` on `target`, moves toward it by a bounded speed, and publishes `Vec3` on `position`.
- `AddScalar`: consumes `f32` on `value`, adds a configured increment, and publishes `f32` on `value`. It provides the bounded feedback-cycle proof.

Each behaviour has an explicit unavailable-input policy. `FollowPosition` suspends and retains its last output. `AddScalar` uses its configured default input. Disabled source behaviours are unavailable to consumers.

## Fixed-tick data flow

1. Run `PreFixed`, the compatibility callback, and `Fixed` systems.
2. Evaluate each enabled behaviour from committed inputs.
3. Commit all behaviour outputs simultaneously.
4. Apply staged behaviour transform writes through the existing `World` transform API.
5. Commit queued entity lifecycle commands. Deletion removes owned behaviours.
6. Propagate the SlotMap transform hierarchy.
7. Run `PostFixed` and `after_fixed_update`.

No behaviour recursively evaluates another behaviour. A feedback link therefore advances once per tick with a one-tick delay.

## Hot paths and expected scale

- Evaluation is iterative and uses a retained order vector and retained transform-write buffer.
- `BehaviorValue` is a small copyable enum; the initial tick path performs no boxing or formatting.
- Links resolve by typed generational key directly into SlotMap storage.
- Transform writers stage one write each; application reuses the graph buffer and existing world dirty-traversal storage.
- The initial enum of built-in kinds is deliberately closed. A later registration design must preserve batched execution and AI-readable schemas before project-defined behaviour types are admitted.

## Validation

- Validate typed connection, disconnection, and rewiring.
- Validate leader/follower motion and simultaneous snapshot commits.
- Validate a self-linked scalar feedback cycle advances exactly once per tick.
- Validate disabled and deleted sources produce the configured unavailable-input result.
- Validate subtree deletion removes owned behaviours and stale IDs never alias reused slots.
- Validate conflicting transform writers are rejected.
- Validate equivalent headless runs produce equivalent outputs.
- Run debug and release tests, Clippy, rustdoc, the reference demo, and a release behaviour benchmark.

## Deferred

- Project-defined behaviour registration and schema export.
- Multiple inputs or outputs on a single built-in behaviour.
- Pending entity or behaviour handles inside one command batch.
- Parallel behaviour evaluation, graph compilation, and same-tick dependency ordering.
- Behaviour serialization, project assets, visual inspection, and scripting.
- Renderer extraction and `wgpu`.

## Acceptance gate

Iteration 5 is complete when the reference demo is behaviour-driven, link and lifecycle edge cases are tested, feedback cannot recurse within a tick, the tick path retains its work buffers, and an optimized representative benchmark is recorded with its limitations.
