# Skeletal animation on macOS (SKEL-1, SKEL-2)

Recorded 2026-11-05 on macOS arm64, Apple M3 Pro, Metal backend.

Project: a disposable copy of `templates/standard` in the session scratch directory,
with its engine path pointed at this checkout. A generated 12-vertex column model
(`hips` and `spine` joints, clip `wave`: spine at 90° about Z at t = 0, upright at
t = 1 s) was saved as `assets/models/column.glb` with a `kind = "model"` entity asset.
No `projects/first_project` content was used.

- `cargo run -- --inspect-model assets/models/column.glb` reported `skinned = true`,
  12 vertices, 20 triangles, joints `hips`, `spine` (parent `hips`) and clip `wave`
  (1.0 s, 1 channel).
- `--preview-scene` of a scene with the column and no animator: the column stands
  upright (rest pose) with a straight shadow.
- The same scene with `[[entities.components]] type = "emergent.animator"`,
  `clip = "wave"`: at tick zero the upper half is bent 90° to the left, and so is its
  directional shadow. This confirms the scene component → `Animator` palette →
  `set_skin_palette` → skinned colour and shadow pipelines path in the real project host.

## SKEL-3 retargeted mocap (2026-11-06)

Same project. A 19-joint VRM-named stick rig (A-pose, facing −Z, metres) was written as
`assets/models/rig.glb`, and the CMU-style test walk as `assets/mocap/walk.bvh`, both
from the `tests/mocap.rs` helpers via a throwaway scratch binary.

- `--inspect-model assets/mocap/walk.bvh` mapped all 19 humanoid bones (e.g.
  `chest = "Spine1"`, `leftUpperArm = "LeftArm"`); `--inspect-model` of the rig
  reported `skinned = true`, 144 triangles, `humanoid_missing = []`.
- `--preview-scene` without an animator shows the rig's A-pose; with
  `emergent.animator` `motion = "assets/mocap/walk.bvh"`, `clip = "walk"` the first walk
  frame is applied at tick zero (arms lowered further, left forearm bent, knees bent),
  shadows included.

Not checked: a live `--run-scene` window playing the clip over time (the time path is
covered by `tests/skinned_model.rs::an_animator_component_plays_its_clip_on_the_fixed_tick`),
and real Mixamo/VRoid assets.

## SKEL-5 third-person mannequin (2026-11-07)

Project: a fresh copy of `templates/standard` (engine path pointed at this checkout),
scene `assets/scenes/third-person.scene.toml`.

- `--inspect-model assets/models/mannequin.glb`: skinned, 528 vertices, 264 triangles,
  all 21 humanoid bones mapped; each `assets/mocap/*.bvh` maps fully (22 channels;
  idle 3.0 s, walk 1.1 s, run 0.7 s).
- `--preview-scene` at tick 0: both mannequins stand on the floor in the idle pose with
  shadows; ramps and hills render.
- `--preview-ticks 100 --preview-keys W`: the hero walks away from the camera
  mid-stride. With `W,Shift`: running gait (bent elbows), and the watcher's head has
  turned towards the player. `--preview-ticks 75 --preview-keys D`: the hero has turned
  to face +X, mid-stride.
- `--run-scene` opened the native window (Metal, M3 Pro) and ran until stopped.
  Driving it interactively was not possible from this session (no macOS
  accessibility/screen-recording grant), so live keyboard play was not watched.
