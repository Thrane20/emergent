# Surface quality comparison

2026-10-25. Compare the current 24x12 sphere / 32-cubed RGBA8 derivative bump
against (A) analytic sphere normals + object-space finite-difference bump gradient
at the same geometry/map resolution, and (B) those fixes with 64x32 geometry and
64-cubed maps. Keep camera, seed, lighting, bump strength and colour fixed. Capture
native images before modifying the original implementation; preserve that executable
and source snapshot for reproducibility. Include optional 128x64/128-cubed results.

Keep SlotMap identity and renderer resource ownership. Sphere normals use radial
analytic values, while Flat shading retains face normals. Bump samples a central
object-space noise gradient, transforms it by the normal matrix and projects it
onto the smooth tangent plane. Avoid screen-space derivatives that depend on each
triangle. Six extra texture samples run only for lit, enabled bump materials.

Use RGBA16Float packed noise for finer height precision. Add per-material
noise_resolution=32/64/128; retain default 32 to avoid forcing every existing object
to a larger volume. A material-wide 8,388,608 octave-evaluation budget bounds baking;
64 cubed supports four eight-octave maps, while 128 cubed requires fewer maps/octaves.
Map storage: 32=256 KiB, 64=2 MiB, 128=16 MiB, allocated only when maps exist.
Sphere-only authored limits become sectors<=128, stacks<=64; other shapes stay at 32.

Expose map resolution in the live inspector. Provide a comparison scene so results
can be judged in the IDE without altering user-authored objects. Measure optimized
CPU upload/bake time and warmed captured-frame time including readback; these are
specific-workload measurements, not FPS or GPU-only claims. Validate radial normals,
quality bounds/budgets, nonuniform transforms and unchanged material/shadow controls.
