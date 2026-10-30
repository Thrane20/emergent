# Ray-marched biome landscapes

Selectable-biome increment: new defaults choose Hilly; missing biome fields decode
as Blend to preserve saved v2 output. RaymarchedBiome adds Plain, Hilly, Mountainous,
Island, Rivers, Canyon, Underwater and CaveCavern. CPU/WGSL branches agree; island
falloff and river carving add explicit derivative terms to the global bound.
CaveCavern carves a chamber into a raised roof with connecting tunnels. The shared
builder modal exposes these choices, with live camera control through one retained
native worker. See `unified-terrain-builder.md`.

2026-10-30. Replace new test-shape defaults with a version-2 landscape recipe.
Existing recipes without version load as v1 and retain their original field.
No change to World or renderer SlotMap ownership, scene references or transactions.

Recipe: seed, region width (64–4096m), biome scale (80–512m), relief (8–160m),
overhang displacement (0–24m), cave radius (0–16m). Width clips a world-coordinate
field; it does not scale its geological features. Default width 1024m. These are
input bounds, not a measured gameplay/streaming budget. No voxels or mesh are built.

Field: seeded smooth value-noise regional mask, rolling plains, ridged mountain
height and finer rock detail; 3D density displacement allows non-heightfield
surfaces in mountain regions. Two seeded winding tunnel fields carve through
rock and intersect the hillsides. Distance min/max composition closes the region.
Fixed shader biome colours blend meadow, exposed rock and high-altitude snow by
elevation and slope; this does not add vegetation, water or material-library painting.

Smooth value noise lies in [0,1], with derivative at most 1.5 per lattice axis.
For 2D noise, gradient bound is 1.5*sqrt(2)/wavelength; for 3D it is
1.5*sqrt(3)/wavelength. Bound the mask derivative (smoothstep width 0.3), height
products and signed rock displacement, then divide density by their global
Lipschitz bound before tracing. Curved tunnel bounds include centerline derivative.
This conservative first implementation trades steps for correctness; measure it.
CPU and WGSL use the same integer hash, interpolation, fields and bounds. Test
gradient limits, seed determinism, width independence, cave/overhang topology and
CPU/GPU silhouettes. Validate default and kilometre views natively, then editor
save/reopen/scene flow. Keep work capped and record timings without FPS claims.

V2 tracing is capped at 1024 steps, with a 0.003m bound-space tolerance enlarged
to 0.00003 times travelled distance for distant views. The first 512-step budget
missed a sampled surface and was rejected. A cap is still not proof of convergence
for all grazing rays/parameter extremes. V1 retains its original 256 steps/2mm
bound tolerance. Collision extraction must sample the field, not this visual hit
tolerance. Default overhang displacement is 6m; 24m is used to test isolated
non-heightfield topology without cave carving.

Reference: [GPU Gems 3, procedural terrain density fields](https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-1-generating-complex-procedural-terrains-using-gpu).
This increment uses a procedural density field directly, not its mesh extraction.
Collision remains a separate same-field surface extraction/contact task.
