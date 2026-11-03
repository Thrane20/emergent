# Flight-control baseline (FLIGHT-0)

Recorded 2026-11-02 on macOS arm64, Apple M3 Pro, rustc
`1.99.1 (58cf8f9c3 2026-10-13)`. Optimized `release` build, optional `physics`
feature only. Headless, engine fixed timestep 1/60 s. No GPU/frame-rate claim.

Reproduce:

```sh
cargo run --release --features physics --example flight_baseline
```

The example has assert-based correctness checks and uses only public engine APIs.
It records three separate references; the kinematic and physical routes are not
equivalent flight-controller performance comparisons.

## Observed trajectories

- Existing kinematic `drone`, max speed/climb 2 m/s, scripted Stop course for 20 s:
  end `[0,5.999996,-17.033394]`, peak height `6.328437 m`, final speed `0.000210 m/s`.
  This course requests y=6 and z=-20; its existing hover behaviour stops about
  2.967 m short horizontally. This records current behaviour, not successful
  precise fly-to/hold acceptance.
- Unpowered dynamic unit cube, 1 kg, initial centre y=5, no ground, after 60 ticks:
  centre y=`0.074564 m`, vertical speed `-9.810013 m/s`. It falls under gravity.
- The same physical body with 9.81 N upward submitted every tick holds y=5. All
  measured constant-lift body counts below had maximum height error `0.000000 m`
  at the printed precision. Force is applied on the first body step.

## Whole fixed-tick baseline

Separated dynamic cubes (3 m spacing), no ground/contact, no controller, wind,
brain or rendering. Constant world-space lift submitted for each body each tick.
Warm up 120 ticks, measure 1,200 ticks using one continuously started engine and
`advance_frame`, with one fixed step per call. Wall-clock samples include force
submission, physics, transforms and engine fixed-tick overhead, not compilation or
initialisation. Sorting/printing occur outside measured ticks.

| Bodies | Median µs | P95 µs | P99 µs | Max µs |
| --- | ---: | ---: | ---: | ---: |
| 1 | 6.292 | 8.542 | 8.833 | 67.583 |
| 16 | 13.625 | 25.125 | 30.833 | 68.833 |
| 64 | 34.208 | 42.125 | 83.250 | 210.667 |
| 128 | 54.500 | 66.333 | 122.791 | 243.625 |
| 256 | 111.500 | 132.375 | 231.667 | 1702.500 |

These are one local run, with concurrent workstation activity uncontrolled. They
are a baseline, not evidence that a controlled/colliding drone workload meets the
plan's budgets. No allocation or per-subsystem cost measurement is claimed for
these existing physics ticks. Later comparisons must match the build, machine,
fixture, warm-up and timing boundary, and include clustered contacts and churn.

After adding transient torque reset/command handling, the same command and build
profile produced identical trajectory results and zero printed hover drift:

| Bodies | Median µs | P95 µs | P99 µs | Max µs |
| --- | ---: | ---: | ---: | ---: |
| 1 | 6.167 | 16.750 | 17.208 | 95.167 |
| 16 | 9.625 | 30.750 | 33.750 | 59.166 |
| 64 | 30.042 | 34.084 | 70.375 | 160.208 |
| 128 | 59.084 | 66.709 | 115.917 | 248.708 |
| 256 | 118.709 | 129.542 | 167.708 | 293.334 |

This second local run overlapped debug workspace validation. The mixed median/tail
changes do not establish a speedup or a statistically reliable regression. This
comparison records the added physics-path work; full isolated repeated workload
profiling still belongs to FLIGHT-7.

The initial implementation uses the existing Rapier backend at 60 Hz. A second
standalone force integrator was not implemented: the existing public force path
already establishes gravity/lift behaviour, and duplicating contact integration
is not needed to unblock PID. The optional comparison remains a later experiment
if workload measurements justify it. See the [frozen implementation contract](../design/flight-control.md).
