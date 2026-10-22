# Iteration 5 behaviour graph baseline

Recorded: 2026-10-22

## Purpose

Measure the first SlotMap behaviour graph under linked, steady-state workloads. This is a local regression baseline, not a supported scene-size or frame-rate target.

## Environment

- Emergent 0.5.0 release profile
- Rust 1.99.1 (`aarch64-apple-darwin`, LLVM 23.1.1)
- Apple M3 Pro, 12 logical CPUs, 36 GB memory
- macOS 27.0.1 (26A434)

## Workloads

Run:

```sh
cargo run --release --example behavior_bench -- all
```

Construction is outside the timed region. Each case has 20,000 entities and behaviours and advances 200 fixed ticks at 60 Hz.

The linked-motion case has one linear-motion source and 19,999 position followers linked to it. Every tick evaluates the graph, stages and applies 20,000 transform writes, and propagates the transform graph. This fan-out exercises typed link resolution and the complete transform-writing path; it does not represent a spatially realistic scene.

The scalar-feedback case has 20,000 independent self-linked add-scalar behaviours. Every link reads the previous snapshot, so each node advances exactly once per tick. It isolates behaviour evaluation and simultaneous output commit without transform work.

Three sequential release runs produced:

| Case | Median tick | Observed range | Stable checksum |
| --- | ---: | ---: | ---: |
| Linked motion fan-out | 478.72 us | 464.84–607.36 us | -59939.578125 |
| Independent scalar feedback | 157.78 us | 153.85–164.96 us | 200.000 |

## Interpretation

The feedback result establishes a baseline for direct SlotMap link lookup and snapshot commit. The motion result additionally includes validated world transform updates, dirty marking, and propagation. Their difference cannot be assigned solely to any one operation without profiling.

The benchmark does not measure construction, deletion, rewiring, memory usage, allocator calls, deep hierarchies, heterogeneous behaviour mixes, rendering, or parallel execution. The retained execution-order and transform-write buffers avoid deliberately allocating new work collections each tick, but allocation-free steady state has not yet been verified with allocator instrumentation.

As a whole-tick regression check, five additional warmed runs of the existing 100,000-entity Iteration 4 benchmark produced medians of 164.77 us for empty schedules and 206.29 us for the scheduled dense update. The observed ranges were 161.61–177.46 us and 205.73–244.57 us respectively, with the existing `100099.000` checksum. These noisy local results show no measured regression against the earlier medians of 179.13 us and 208.04 us; they do not isolate the cost of evaluating an empty behaviour graph.
