# ADR 0004: Behaviour graph identity and snapshot evaluation

Status: accepted for the first behaviour graph.

## Decision

`World` owns a behaviour graph whose nodes use world-scoped typed SlotMap keys. Each behaviour belongs to one canonical `EntityId`; links store `BehaviorId` keys and typed static port identifiers. The entity ownership hierarchy and the behaviour link graph remain separate structures because links may cross hierarchy branches and form cycles.

The initial graph contains three closed built-in behaviour kinds: linear motion, position following, and scalar addition. Project-defined registration remains deferred until its schema and batched execution contract can be designed without weakening the runtime storage model.

Every fixed tick reads only the previous committed output snapshot. Evaluation stages each next output and every transform write, then commits outputs together. Graph insertion order is stable for diagnostics, but cannot change a tick's results. Links never recursively execute their source, so feedback cycles advance once per tick and require no cycle traversal.

An entity may have multiple behaviours and at most one enabled transform-writing behaviour. Transform writes use the existing validated world API before hierarchy propagation. Entity subtree removal deletes all behaviours owned by that subtree and compacts the retained execution order once. Surviving links to deleted behaviours remain stale and resolve as unavailable; SlotMap generations prevent them from aliasing replacement nodes.

## Unavailable inputs

Each built-in behaviour declares its response rather than relying on an implicit graph-wide fallback:

- Position following suspends and retains its last output when its link is absent, stale, or disabled.
- Scalar addition uses its configured default input.
- A disabled behaviour retains its committed output but is unavailable as a source.

These states are observable through `BehaviorRunState`. Connection validates both worlds, node generations, port direction, and value type before mutating the target link.

## Tick integration

Behaviour evaluation runs after `Fixed` systems and before lifecycle command commit. This lets fixed systems prepare component state, applies behaviour transform writes coherently, then lets queued deletion remove behaviours before transform propagation and `PostFixed` observation.

## Performance implications

Evaluation is an iterative linear scan over a retained order vector. Values are a copyable enum, links resolve directly through SlotMap keys, and transform writes reuse retained storage. The first implementation favors explicit semantics and measurement over graph compilation or parallel scheduling. Dense per-component work continues to belong in Bevy ECS systems; later measurements may justify compiling behaviours into type-grouped work lists while retaining SlotMap handles as canonical identity.

The Iteration 5 benchmark records linked motion and feedback workloads. Its results are a local baseline, not a universal frame-size or frame-rate claim.
