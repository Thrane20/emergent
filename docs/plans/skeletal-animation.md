# Skeletal animation, skinning, IK and mocap (SKEL)

Drafted 2026-11-05. Status: SKEL-0 agreed; SKEL-1 and SKEL-2 delivered 2026-11-05, SKEL-3 and SKEL-4 2026-11-06, SKEL-5 and SKEL-6 2026-11-07; next: the "Later" list.

## Goal

Import or generate a humanoid (later: quadruped, creature), bind its mesh to a skeleton,
play and blend clips (including motion capture), and adjust the pose at runtime with IK
so feet land on terrain, heads look at targets and hands reach. This unblocks the
Legged locomotion types in [locomotion](../design/locomotion.md) and the GOALS.MD
Animation and "skinning data" capability rows.

## Where we are

- IK appears only as a placeholder: "Biped IK walk ○ (needs skeletons and an animation
  system)" in [locomotion](../design/locomotion.md). Legged profiles move like Slide.
- `src/model.rs` reads `.glb` with the `gltf` crate, flattens node transforms and merges
  everything into one static `Mesh` (position, normal, UV). Skins, joints and
  animations in the file are ignored.
- The GPU `Vertex` (`src/graphics.rs`) has no joint or weight attributes; the geometry and
  shadow shaders have no skinning.
- Animation today is scalar tweens (`src/tween.rs`) and per-part components (spin) on
  [model parts](model-parts.md), which suits rigid parts like rotors, not deforming skin.

## Standards to follow

| Need | Standard | Use in Emergent |
| --- | --- | --- |
| Rigged mesh + clips, runtime interchange | **glTF 2.0** skins and animations (`.glb`) | Primary and only native mesh/skin format. Already parsed; extend `model.rs`. |
| Humanoid bone naming | **VRM 1.0 humanoid** (`VRMC_vrm`, a glTF extension): `hips`, `spine`, `chest`, `upperChest`, `neck`, `head`, `leftUpperLeg`, `leftLowerLeg`, `leftFoot`, `leftToes`, `leftShoulder`, `leftUpperArm`, `leftLowerArm`, `leftHand`, fingers, … | Canonical humanoid profile. Read it from VRM files directly; for other rigs, map bone names to it (Mixamo `mixamorig:*` and common Blender/Unreal names get built-in maps). |
| Motion capture clips | **BVH** (Biovision Hierarchy, text): hierarchy + per-frame channels | Native import. Simple to parse (no dependency), and it is what free libraries such as the CMU Graphics Lab database ship. |
| Commercial clip libraries | **FBX** (Mixamo, Rokoko, most stores, most AI video-to-mocap services) | Not parsed natively: proprietary, no maintained Rust parser. Convert to GLB offline (Blender, FBX2glTF). The IDE can explain the step. |
| Raw optical marker data | **C3D** | Out of scope: markers need solving onto a skeleton first. |
| Future pipeline formats | USD Skel, SMPL/SMPL-X (research body models, restrictive licence) | Watch only. |

Most mocap sources (BVH, glTF, video-to-mocap exports) use a different skeleton from the
character, so **retargeting onto the humanoid profile** is the key piece, not the file
parser.

## Architecture

Ownership follows AGENTS.md: SlotMap handles stay the canonical identity for world
objects; dense arrays carry hot data.

- **Skeleton asset (shared, immutable).** Joint names, parent indices, rest
  (local) transforms and inverse bind matrices, stored **topologically sorted** so
  `parent[i] < i`. That makes cycles impossible by construction and lets pose
  evaluation be one linear pass with no recursion. Optional humanoid map
  (profile bone → joint index). Limit 256 joints (VRM ~55, Mixamo 65).
- **Joints are not world entities.** A character with 65 joints stays one `EntityKey`
  carrying a skinned-mesh component. Making every bone a SlotMap entity would multiply
  hierarchy cost and churn per character. Joint *indices* are runtime-only; authored
  files and MCP refer to joints **by name** (durable identity, no keys persisted).
  *Confirmed with the user on 2026-11-05.*
- **Sockets.** To attach a sword or a camera to `rightHand`, a child entity has a socket
  binding (joint name, resolved to an index at load). The parent's world transform pass
  reads the joint's model-space matrix. Missing joint = load error, not silent fallback.
- **Pose (per instance).** `Vec<Transform>` local pose + `Vec<Mat4>` skinning palette in
  a component on the entity, buffers sized once at spawn and reused every frame.
- **Clip asset.** Per-joint channels (translation, rotation, scale) with keyframe times,
  linear/slerp and step interpolation (glTF cubic spline later). Bound to a skeleton by
  joint name at load, giving a dense channel → joint table.
- **Animator component (`emergent.animator`).** Current clip(s), time, weights, loop
  mode. Later a small state/blend graph authored like brains, with inputs from
  locomotion (speed, turn rate, gait phase, grounded).

### Frame data flow

```
simulation tick → locomotion/brain set animator inputs
              → sample clips → blend → (retarget) → IK passes → local pose
              → linear pass: model-space matrices → palette (× inverse bind)
render        → upload palette per instance (one storage buffer, offset per instance)
              → skinned vertex shader: 4 joint indices + 4 weights → position/normal
```

- **Skinning: linear blend, GPU, 4 influences.** Skin data in a *second vertex stream*
  (`u16x4` joints, `unorm8x4` weights) so the static `Vertex` and every existing
  pipeline stay unchanged. Shadow pass gets the same skinned variant. Dual-quaternion
  skinning only if candy-wrapper artefacts show up at elbows and wrists.
- Pose evaluation is CPU, allocation-free in steady state, parallel per character only
  once measured.

## IK

Built as post-processes on the sampled pose, cheapest first:

1. **Two-bone analytic IK** (law of cosines + pole vector): legs and arms. Covers foot
   placement and hand reach for humanoids and most quadrupeds.
2. **Look-at / aim** chain with per-joint weights and limits: head, eyes, spine, turrets.
3. **Foot placement**: raycast each foot to ground, shift hips down to the lowest
   contact, two-bone IK each leg, align foot to the surface normal. Needs the public
   physics/terrain ray query from the Physics backlog.
4. **FABRIK** for longer chains (tails, tentacles, spines), bounded iteration count.
5. Later: full-body IK, ragdoll (needs physics joints).

Legged locomotion then becomes clip-driven walk/run with gait phase from the
locomotion profile, foot IK correcting slopes and steps, and a root-motion contract
(clip drives displacement, or locomotion drives it and the clip speed matches).

## Rigging a mesh

Three ways to get a rigged character, in order of effort:

1. **Import already rigged GLB/VRM** (Mixamo via Blender, VRoid Studio, store assets).
   Engine only needs SKEL-1.
2. **AI auto-rig.** Generated Object Creator meshes have no skeleton. Candidates: Tripo's
   rig/animate tasks, Mixamo's auto-rigger (manual, web), open models such as UniRig.
   SKEL-6 chose Meshy Rigging on fal ([spike](../research/auto-rig-providers.md)); a live run
   rigged a generated character that then walked and ran in the template scene.
   Expected input: a T- or A-posed humanoid mesh; output: GLB with skin.
3. **IDE rigging** (place joints on the mesh, automatic weights by bone heat/distance,
   weight-paint view). Large; only if 1 and 2 fall short.

## Stages

| Stage | Delivers | Validation |
| --- | --- | --- |
| SKEL-0 ✓ | This plan and a design brief; confirm "joints not entities"; frame budget for N characters | Review with user |
| SKEL-1 ✓ | GLB skin import (skeleton asset, skin stream, inverse binds), GPU skinning in colour and shadow passes, bind pose renders | Synthetic skinned GLB built in the test (no game fixture); malformed skins, >4 influences, joint-limit errors; native screenshot |
| SKEL-2 ✓ | GLB clip import, `emergent.animator` single-clip playback, IDE preview and MCP describe | Sampling unit tests (slerp, step, loop boundaries); allocation-free tick check |
| SKEL-3 ✓ | Humanoid profile, built-in name maps, rest-pose (T/A) alignment, retarget; BVH import | BVH parser tests incl. malformed files; retarget a BVH walk onto a VRM-mapped rig |
| SKEL-4 ✓ | Blending (crossfade, 1D speed blend), locomotion inputs, root motion | Blend weight tests; walk/run preview in the IDE |
| SKEL-5 ✓ | Two-bone IK, look-at, foot placement on terrain; Legged locomotion type | IK convergence/limit tests; feet on a slope in a template-derived project |
| SKEL-6 ✓ | Rigging pipeline in the IDE: rigged import flow, AI auto-rig provider spike, weight view | Provider spike recorded in `docs/research/` |

Later: morph targets (faces), animation state graph editor, ragdoll, compression,
LOD for skeletons, compute skinning.

### SKEL-1 delivered (2026-11-05)

- `src/skeleton.rs`: `Skeleton` (names, parents with `parent < child`, rest TRS, inverse
  binds), `palette()` two-pass and allocation-free once sized.
- `src/model.rs`: one skin per model; joints reordered topologically and `JOINTS_0`
  remapped; root joints absorb non-joint ancestors; non-skinned meshes under a joint
  ride it rigidly; `JOINTS_1`, missing weights, plain nodes between joints, sheared
  joints and >256 joints are rejected. `ImportedModel.skeleton` carries the result.
- `src/geometry.rs`: `MeshSkin` on `Mesh` (`with_skin`, validated).
- `src/graphics.rs` + shaders: `SkinVertex` stream (`u16x4` joints, `unorm8x4` weights
  summing to 255), per-geometry palette storage buffer, skinned colour and shadow
  pipelines, `set_skin_palette`. Skinned meshes skip clustering and frustum culling.
- Known limits: shader-graph materials and flat shading fall back to the standard smooth
  surface on skinned meshes (`shortcut:` comments); one palette write per skinned mesh
  per frame; scenes show the rest pose until SKEL-2 adds an animator.
- Validation: `cargo test --features graphics,physics` (all pass), Clippy clean,
  `tests/skinned_model.rs` bends a synthetic two-joint GLB on the GPU with a shadowing
  light. No native scene screenshot or benchmark yet.

### SKEL-2 delivered (2026-11-05)

- `src/animation.rs`: `AnimationClip` (validated channels per joint: translation,
  rotation, scale; linear/slerp and step; clamped sampling) and the `Animator` component
  (clip by name, time, speed, loop, playing; reusable pose and palette). `step` runs
  after physics and map refresh, before `post_fixed`.
- `src/model.rs`: glTF animations → clips on rig joints; channels on non-joint nodes and
  morph weights skipped; cubic-spline keeps values only (`shortcut:`); 1M key budget;
  duplicate names get `#index`.
- `emergent.animator` built-in component (`clip`, `speed`, `loop`, `playing`); requires a
  skinned model and a known clip name; empty clip plays the first. The project host
  writes each animator's palette to its geometry every frame (run and inspection).
- `--inspect-model <glb>` (tool server too) reports size, joints and clips; MCP
  `describe` of a model asset includes it, and the MCP instructions mention the animator.
- Validation: `tests/skinned_model.rs` (clip import, animator over 120 engine ticks,
  component errors), unit tests in `animation.rs`, native preview in a template-derived
  project ([platform](../platform/skeletal-animation-macos.md)), CPU benchmark
  ([benchmark](../benchmarks/skeletal-animation.md): ~1.5 µs per 65-joint character per tick).
- Not done: an IDE clip picker (the inspector shows a text field), live-window
  playback observed by eye, real Mixamo/VRoid files.

### SKEL-3 delivered (2026-11-06)

- `src/humanoid.rs`: `HumanoidBone` (22 VRM 1.0 body bones; fingers/eyes not yet),
  `detect_bone` for VRM/VRoid (`J_Bip_L_*`), Mixamo (`mixamorig:*`), CMU/BVH, Unreal
  (`upperarm_l`, `spine_01`), Rigify (`DEF-thigh.R`) and 3ds Max Biped names;
  `HumanoidMap::detect` (first joint per bone wins) and `missing()` against the 15
  required bones. No authored override map yet.
- `src/bvh.rs`: `Motion::parse/load` — one ROOT, JOINT/End Site, up to 6 channels per
  joint, rotations composed in channel order (degrees), ≤256 joints, ≤100k frames,
  ≤64 MiB; rest pose = zero-rotation offsets; one clip named after the file.
- `src/retarget.rs`: `retarget` bakes at 30 fps. Mapped bones copy the source's
  world-space change from rest; a per-bone arc aligns rest directions (T- vs A-pose);
  end bones inherit their parent's arc; facing is aligned from the hip axis; hips use
  the source's model-space position × leg-length ratio (absorbs cm vs m). Both models
  are assumed to stand on y = 0 at their origin.
- `emergent.animator` gains `motion` (a `.bvh` or rigged `.glb`); its clips are
  retargeted at scene load and added to the model's own. `--inspect-model` accepts
  `.bvh` and reports `humanoid` and `humanoid_missing`.
- Validation: `tests/mocap.rs` — BVH parsing and 8 malformed cases; a CMU-style walk
  (cm, T-pose, +Z, unmapped `LHipJoint`) onto a VRM-named rig (m, A-pose, −Z, twisted
  knee axes) keeps every mapped bone's world direction within 2e-3 over the clip;
  hips land at the scaled, turned position; a scene animator plays the BVH on a skinned
  GLB over 61 engine ticks; non-humanoid targets fail with the missing bones listed.
  Native preview in the template-derived project ([platform](../platform/skeletal-animation-macos.md)).
- Not done: authored humanoid overrides, finger mapping, VRM `VRMC_vrm` extension
  reading (VRoid names are detected instead), BVH files in the IDE asset library,
  real CMU/Mixamo files, foot sliding correction (needs IK, SKEL-5).

### SKEL-4 delivered (2026-11-06)

- `Animator::crossfade(clip, seconds)`: the outgoing clip keeps playing and is mixed
  out linearly (lerp translation/scale, slerp rotation).
- Speed blend (`Animator::set_blend`, component field `blend = "idle:0, walk, run"`):
  1D piecewise-linear weights from the object's smoothed horizontal ground speed
  (world-position delta per tick, in the object's own space, 0.15 s smoothing), so it
  works for locomotion, physics, character control or game code alike. Clips share one
  normalised phase so feet stay in step; above the top speed the top clip plays faster.
- Root-motion contract: **locomotion drives displacement.** `make_in_place` removes
  the root-most translated joint's horizontal drift and start offset (sway and height
  kept) and reports the removed travel speed, which is each blend clip's default speed.
  `in_place = true` applies it without a blend. Clip-driven displacement is not built.
- `motion` takes comma-separated files (one mocap take per file).
- Validation: unit tests for weights, shared phase, scaled objects, standing still,
  crossfade and in-place; `tests/mocap.rs` blends retargeted walk and run BVH takes on
  a rig moved at 0.9 m/s by game code (ground speed 0.9, hips stay over the origin) and
  rejects an unknown blend clip. [Benchmark](../benchmarks/skeletal-animation.md):
  a half/half blend costs about 2× a single clip.
- Not done: clip-driven root motion, turn-rate or gait inputs, a state graph, a way for
  brains or modes to trigger `crossfade`, an IDE walk/run preview (no native capture
  this stage).

### SKEL-5 delivered (2026-11-07)

- `src/ik.rs`: analytic `two_bone` (law of cosines; keeps the animated knee's hinge
  plane, falls back to the pole for straight limbs; out-of-reach targets straighten
  towards them) and `look_at` (chain shares the turn, angle-limited, weighted).
  `Skeleton::globals` exposes model-space matrices.
- Animator post-process (`Animator::advance_with`, `AnimatorInputs`), run by
  `World::step_animators` each fixed tick after physics:
  - **Foot placement** (`foot_ik`, default on for humanoids): one physics ray per foot
    straight down (static colliders only, so the character's own dynamic body is
    ignored); each foot keeps its animated height above the ground under it (±45% of
    leg length), the hips drop to the lower foot, two-bone IK per leg with the knee
    pole forward, foot tilted to the ground normal (≤ 34°); all smoothed over 0.08 s.
  - **Look-at** (`look_at` instance): neck and head share the turn, ≤ 69°, fading out
    for targets more than 115° off the face.
  - **Turn to movement** (`turn_rate` °/s): yaws the object to face its smoothed
    horizontal velocity; model forward comes from the hips' left→right axis, so +Z
    (glTF) and −Z (VRM 0.x) models both work.
- `biped` locomotion type (Legged): Slide kinematics without self-turning, for brains;
  the animator supplies clips, facing and feet.
- Try-out kit in `templates/standard` (generated by `examples/mannequin_assets.rs`):
  `assets/models/mannequin.glb` (21-joint VRM-named segmented mannequin),
  `assets/mocap/{idle,walk,run}.bvh` (procedural CMU-style takes) and
  `assets/scenes/third-person.scene.toml` (WASD walks, Shift runs via the new character
  controller `run_speed`, Space jumps; ramps, hills; a second mannequin watching you).
- Also: `.vrm` files load as models; Text component fields may declare a longer limit
  via `range` (`motion` 512, `blend` 256); `--preview-scene … --preview-ticks N
  [--preview-keys W,Shift]` renders after scripted input.
- Validation: IK unit tests (reach, pole side, straightening, unlinked chains, look
  limit and sharing); `tests/mocap.rs`: feet on two static blocks 0.2 m apart (hips
  drop 0.2, each ankle 0.1 above its own ground, knee forward), head faces a look-at
  target within 0.02 rad, the template scene walks (1.4 m/s), runs (3.8), turns to −Z
  then +X, stops and stands with ankles on the floor; native frames
  ([platform](../platform/skeletal-animation-macos.md)); [benchmark](../benchmarks/skeletal-animation.md).
- Not done: ray cost measured only with a stub ground; feet on dynamic bodies; toe
  IK and foot locking (feet still slide when clip and movement speeds differ); hand
  IK targets; FABRIK; a brain-driven biped demo; IDE access to tick previews; tilted
  parents (yaw and foot placement assume upright objects); the mannequin's takes are
  procedural, not captured motion.

### SKEL-6 delivered (2026-11-07)

- **Import** (project files → *Import model or mocap…*): `.glb`/`.vrm` go to
  `assets/models/` with a `kind = "model"` entity, `.bvh` to `assets/mocap/`. The engine
  (`--inspect-model`) must load the file before it is kept; a rejected file is removed and
  the engine's message shown. Name clashes are refused, never overwritten.
- **Rig panel** under the model preview (model files, model entities, generated objects and
  BVH takes): vertices, joints, humanoid bones mapped/missing and clips, from the engine.
- **Weight view**: three.js view of the skinned mesh at rest with the skeleton drawn;
  vertices coloured by their strongest joint, or a heat map for a picked joint.
- **AI auto-rig**: unskinned `.glb` models offer *Auto-rig humanoid · fal ≈ $0.20*
  (Meshy Rigging) after a price confirmation. It reuses the Model Parts fal job journal
  (upload once, never resubmit an unconfirmed paid call); a ready result is saved through
  the same engine-checked import. [Provider spike](../research/auto-rig-providers.md).
- Validation: `tests/model-rig.test.ts` (import paths, entity, name clash, BVH/GLB sniffing,
  engine rejection rollback, path escape; Meshy input and job listing); real engine report
  and `.vrm` import through the route on a template-derived project; headless Chromium
  check of the panel, weight view and joint heat map with no console errors.
- Live run: FLUX → Trellis → Meshy rig → walk/run/turn in the third-person scene, about $0.25;
  it found Meshy's downward-numbered spine, now mapped by hierarchy in `HumanoidMap::detect`.
- Not done: Meshy output licence unchecked; no
  weight painting or joint placement in the IDE; the weight view ignores morph targets
  and shows the rest pose only; no MCP import/auto-rig tool.

## Budgets and measurement (hypotheses until measured)

- Per skeleton: ≤ 256 joints, 4 influences per vertex, existing model vertex limits.
- First benchmark: 100 characters × 65 joints, one clip each, plus two-bone IK on legs;
  report CPU pose time, palette upload size and GPU time with `examples/` bench on the
  recorded machine. No frame-rate claim until that runs.

## Risks and open questions

- Joints-as-indices vs. entities (above) needs user sign-off.
- Retargeting quality varies with rest pose and proportions; finger mapping is often
  absent in mocap.
- Foot placement depends on a physics ray query that does not exist yet.
- AI auto-rig providers and licences unverified.
