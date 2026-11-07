# Skeletal animation CPU cost (SKEL-2)

Recorded 2026-11-05 on macOS arm64, Apple M3 Pro, `rustc 1.99.1 (58cf8f9c3 2026-10-13)`. Optimized `release`
build, no optional features. Headless, single thread, CPU only; GPU skinning and
palette upload are not included. No frame-rate claim.

Reproduce:

```sh
cargo run --release --example skeletal_bench [characters] [ticks]
```

Workload: each character has its own `Animator` on a shared 65-joint chain
(Mixamo-sized) and one clip that rotates every joint (30 linear keys per channel).
One tick = `Animator::advance(1/60)` for every character: reset to rest, sample all
65 channels (binary search + slerp), rebuild the model-space palette.

| Characters | Ticks | Median ms/tick | p99 ms/tick | Max ms/tick |
| --- | --- | --- | --- | --- |
| 100 | 1000 | 0.149–0.155 | 0.20–0.67 | 0.23–1.42 |
| 1000 | 1000 | 1.50 | 1.71 | 2.01 |

Two runs at 100; the first had one outlier tick (max 1.42 ms), likely scheduling noise.
Cost is about 1.5 µs per character per tick and scales linearly. Pose and palette
buffers are reused; unit tests assert their capacity does not change while playing.

SKEL-4 (2026-11-06, same machine and build): `skeletal_bench [characters] 1000 1`
speed-blends two such clips half and half (two samples plus a mix per joint), the
worst case. Single-clip runs re-measured after the change.

| Mode | Characters | Median ms/tick | p99 ms/tick | Max ms/tick |
| --- | --- | --- | --- | --- |
| single clip | 100 | 0.155 | 0.19 | 0.36 |
| single clip | 1000 | 1.60 | 2.06 | 4.90 |
| blend 50/50 | 100 | 0.305 | 0.95 | 1.84 |
| blend 50/50 | 1000 | 3.06 | 4.74 | 16.8 |

One run each; the blend tails (p99, max) are noisy and unexplained, not investigated.

SKEL-5 (2026-11-07, same machine and build): `skeletal_bench [characters] 1000 2` plays
the template mannequin's retargeted walk (21 joints) with foot IK against a stub
ground 5 cm above the floor (both legs solved every tick: hips shift, two two-bone
solves, foot tilt, four model-space rebuilds) and a look-at target.

| Characters | Median ms/tick | p99 ms/tick | Max ms/tick |
| --- | --- | --- | --- |
| 100 | 0.154 | 0.21 | 0.38 |
| 1000 | 1.53 | 1.88 | 4.53 |

About 1.5 µs per character, all-in. Not measured: the two Rapier ray casts per
character per tick in a real scene (the stub ground is a closure).

Not measured: the per-frame `queue.write_buffer` per skinned mesh, GPU vertex cost,
and the render-thread lookup of each animator component.
