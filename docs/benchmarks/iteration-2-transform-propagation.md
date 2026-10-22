# Iteration 2 transform propagation baseline

Recorded: 2026-10-22

This is an observed baseline for the initial transform graph. It is not a performance target and should only be compared with equivalent builds and workloads.

## Environment

- Hardware: Apple M3 Pro, 12 CPU cores, 36 GB memory
- Operating system: macOS 27.0.1 (26A434)
- Architecture: `aarch64-apple-darwin`
- Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
- Cargo: `cargo 1.99.1 (5f94df478 2026-08-27)`
- Profile: Cargo `release`
- `glam`: 0.34.1
- `slotmap`: 1.1.1

## Workload

Command:

```sh
cargo run --release --example transform_bench
```

The benchmark constructs two 20,000-entity worlds:

- deep: one chain 20,000 entities long;
- wide: one root with 19,999 direct children.

Each dirty iteration changes the root transform, marks the full subtree dirty, and propagates every global transform. Each clean iteration calls propagation without intervening mutations. Reported values are total elapsed time divided by 100 iterations. Two complete process runs were recorded.

## Observed result

| Shape | State | Run 1 average | Run 2 average |
| --- | --- | ---: | ---: |
| Deep | Dirty | 514.45 us | 502.94 us |
| Deep | Clean | 44.46 us | 44.43 us |
| Wide | Dirty | 184.83 us | 191.74 us |
| Wide | Clean | 32.70 us | 32.89 us |

## Interpretation and limits

Deep propagation is slower because traversal follows scattered child storage and repeatedly reads parent caches. Wide propagation has better access locality. Clean propagation performs no transform updates, but it still scans all entities to find dirty-subtree roots, so its current cost is linear in world size.

This lightweight harness does not control CPU frequency, process scheduling, or thermal state and does not report a distribution. Repeat measurements before using it to assess a change. A later benchmark suite should add warm-up, multiple samples, allocation instrumentation, and representative mixed hierarchies when optimization work is justified.
