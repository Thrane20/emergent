# Iteration 8 macOS geometry smoke

Date: 2026-10-22

Command:

```sh
cargo run --release --features graphics --example geometry_gallery -- --smoke
```

Observed environment and result:

- Adapter: Apple M3 Pro
- Backend: Metal
- Surface format: `Bgra8UnormSrgb`
- Uploaded resources: 4
- Fixed simulation ticks: 120
- Presented frames: 240
- Requested resize to 900 by 600: observed
- Shutdown: clean

The four resources were a solid point marker, solid line segment, convex polygon, and custom indexed pyramid mesh. This proves native resource creation, frame submission, resize handling, and shutdown on this machine. It is a smoke workload and supplies no production frame-time or scale claim. Windows and Linux runtime validation remain pending.
