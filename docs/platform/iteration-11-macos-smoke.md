# Iteration 11 macOS graphics smoke

Date: 2026-10-23

## Command

```sh
cargo run --release --features graphics --example geometry_gallery -- --smoke
```

## Result

Passed on Apple M3 Pro using the Metal backend and `Bgra8UnormSrgb`. The lit geometry gallery created the lighting/shadow pipeline, submitted its bounded light and directional shadow pass, observed its resize request, and shut down after 120 fixed ticks without WGPU validation errors.

The window was occluded in this automation environment, so WGPU reported zero presented frames. This validates resource and pass submission, not visible image quality, performance, or Windows/Linux runtime compatibility.
