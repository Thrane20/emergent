# Iteration 4 runtime integration baseline

Recorded: 2026-10-22

## Purpose

Measure the complete headless fixed-tick path after production Bevy ECS integration. This is a local baseline for later profiling, not a frame-rate target or a general Bevy benchmark.

## Environment

- Emergent 0.4.0 release profile
- Rust 1.99.1 (`aarch64-apple-darwin`, LLVM 23.1.1)
- Apple M3 Pro, 12 logical CPUs, 36 GB memory
- macOS 27.0.1 (26A434)

## Workload

Run:

```sh
cargo run --release --example runtime_bench -- all
```

Each case creates 100,000 canonical SlotMap entities with mapped Bevy component entities, then advances 200 exact fixed ticks. Construction is outside the timed region.

The empty case runs the three empty schedule phases, lifecycle commit, clean transform propagation, and cached-archetype component-store integrity checks. The update case also stores `Position` and `Velocity` components and runs one Bevy system that updates every position during `Fixed`.

Three sequential warmed runs produced:

| Case | Median fixed tick | Observed range |
| --- | ---: | ---: |
| Empty schedules | 179.13 us | 172.97–179.31 us |
| Scheduled dense update | 208.04 us | 206.43–212.27 us |

The update checksum was `100099.000` in every run.

The median difference between the complete empty and update cases was 28.91 us per tick. Treat that subtraction cautiously because the cases use separate world constructions and were not interleaved within a single process. The benchmark is intended to reveal future whole-tick regressions; focused ECS query costs remain in the Iteration 3 comparison.

## Interpretation

The release integrity checks avoid the three full entity scans considered during implementation by querying only archetypes that lack the required identity or resource marker. The dominant empty-tick work at this scale includes the existing clean transform scan over 100,000 canonical entities. Later work should profile schedule execution, transform-root discovery, and command commit separately before optimizing any of them.
