# Emergent — Agent Guidelines

Emergent is a game engine built from scratch in Rust. Gaming-grade speed, predictable frame times, and efficient memory use are primary design requirements. SlotMap-based hierarchies are a foundational architectural choice.

## Core architecture

- Use SlotMap storage and typed generational keys as the default ownership and identity model for engine objects and hierarchy nodes. SlotMap is core infrastructure, not an optional implementation detail.
- Represent parent, child, and cross-object relationships with keys. Avoid raw pointers, reference-counted ownership graphs, and long-lived references into storage as substitutes for this model.
- Keep ownership explicit: a subsystem owns its storage; other systems refer to its objects through typed handles. Distinguish key types for different object domains, and keep handles scoped to the correct owning store or world.
- Treat stale handles as an expected consequence of deletion. Validate handles at boundaries and define how missing objects are handled. Never silently resolve a stale handle to a replacement object.
- Maintain hierarchy invariants: no cycles, consistent parent/child links, and explicit semantics for reparenting and subtree removal. Make multi-link changes coherent before traversal resumes.
- Do not persist or transmit SlotMap keys as durable identity. Use an explicit mapping for serialization, networking, and world reconstruction.
- SlotMap supplies identity and ownership; it does not automatically provide optimal traversal locality. Use dense work lists, packed component data, secondary storage, or cached traversal orders where measurement justifies them, while retaining SlotMap handles as canonical runtime identity.
- Changes that replace SlotMap as the core ownership or hierarchy model require an explicit architectural discussion with the user before implementation.

## Performance priorities

- Optimize for sustained real-time workloads and predictable frame pacing. Consider CPU time, GPU time, memory footprint, allocation rate, and tail latency rather than average throughput alone.
- Keep frame-critical paths allocation-free in steady state where practical. Reuse buffers, reserve known capacities, and avoid repeated cloning, formatting, temporary collections, and redundant work.
- Prefer cache-friendly layouts, linear iteration, batching, and bounded work. Account for hierarchy depth and size; avoid unbounded recursive traversal of user-controlled scene graphs.
- Track dirty state and update only affected data when this reduces total cost. Document cache invalidation rules and verify correctness after hierarchy mutations.
- Avoid blocking I/O, unnecessary synchronization, and lock contention on frame-critical threads. Move expensive loading and preparation off those paths with explicit ownership and scheduling.
- Use parallelism when workload size and measured gains justify scheduling and synchronization costs. Preserve deterministic behavior where required by simulation or replay contracts.
- Choose static dispatch and simple concrete data structures in hot paths when appropriate. Evaluate indirection, dynamic dispatch, and abstractions by their actual runtime and maintenance cost.
- Establish a workload and frame-time budget before claiming a target is met. Do not assume a universal frame rate, scene size, or target platform.

## Rust engineering

- Prefer safe, idiomatic Rust with clear borrowing and ownership. Do not use `unsafe` merely to bypass an inconvenient design.
- Introduce `unsafe` only for a demonstrated requirement or measured benefit. Keep it small, document its safety invariants, and add focused validation, including Miri where applicable.
- Avoid `Rc<RefCell<_>>` and broad `Arc<Mutex<_>>` designs in core engine storage. Use them only at justified boundaries with understood costs.
- Handle recoverable failures explicitly. Keep panics and unchecked assumptions out of normal runtime paths; use assertions to expose broken internal invariants during development.
- Keep dependencies deliberate and lightweight. Building from scratch means owning the engine architecture; reuse focused crates when they improve correctness or performance without importing another engine's architecture.
- Build the smallest coherent implementation needed for the current feature. Avoid speculative frameworks and generalization that obscure data flow or add runtime cost.

## Measurement and validation

- Establish a baseline before performance-sensitive changes and compare using optimized builds under equivalent conditions. Record workload, build profile, hardware, and relevant metrics with performance claims.
- Benchmark representative scenes and hierarchy operations, including creation, deletion, reparenting, traversal, and high object churn. Include realistic scale and pathological shapes when relevant.
- Use profiling to identify bottlenecks. Never describe a change as faster solely because the code looks cheaper; label unmeasured benefits as hypotheses.
- Test hierarchy invariants, stale-handle behavior, and mutation edge cases. Add regression tests for substantive bugs and use property-based tests when they provide meaningful coverage of graph mutations.
- Run formatting, relevant tests, and Clippy for Rust changes when the project supports them. Run targeted benchmarks for changes to hot paths or storage layout. Report commands run, results, and any checks that could not run.
- Do not trade correctness for performance without an explicit, documented contract. Performance regressions need evidence and a clear reason, not accidental acceptance.
- The user runs their app on port 3000. If you want to run and inspect, fire up your own engine on port 3001. When you're done, close down the test service.


## Working in this repository

- Read existing code and local guidance before changing architecture. Keep edits focused on the requested outcome.
- For substantial new systems, first write a concise design brief covering ownership, SlotMap integration, data flow, hot paths, expected scale, and validation. Avoid broad implementation before the boundaries are clear.
- Preserve unrelated work. Do not introduce competing object identity or hierarchy systems as shortcuts.
- Explain what changed, why it fits the engine architecture, and how it was validated. Distinguish measured results from intended improvements and unresolved risks.

## Documentation

- Follow `docs/README.md`: `GOALS.MD` holds goals and stage gates, `plan.md` holds the live queue, and `docs/implementation-status.md` is the delivered ledger.
- Keep docs current with the code. A change that finishes work updates `plan.md`, the status row, and any GOALS.MD checkbox or capability status in the same change.
- Do not add root-level planning or iteration files. Put area plans in `docs/plans/` and leave `docs/history/` frozen.
