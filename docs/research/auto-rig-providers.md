# AI auto-rig providers (SKEL-6 spike)

Checked 2026-11-07 from provider documentation, then verified with one end-to-end paid run
(see *Live run* below).

## Options

| Provider | Access | Input | Output | Cost | Notes |
| --- | --- | --- | --- | --- | --- |
| Meshy Rigging (`fal-ai/meshy/rigging`) | fal queue, same key as Object Creator | `model_url` (GLB), `height_meters` (1.7), optional animation preset (0–696) | `rigged_character_glb`, `rigged_character_fbx`, basic walk/run animations, `rig_task_id` | $0.20; $0.32 with an animation | Humanoid with clearly separated limbs; Meshy's own docs say untextured meshes fail and >300k faces must be remeshed first. Bone naming and licence not stated on fal. |
| Meshy Rigging multi-animation (`fal-ai/meshy/rigging/multi-animation`) | fal | as above plus several presets | one GLB per preset | $0.20 + $0.12 per clip | Useful later for clip packs; we already have BVH retargeting. |
| Meshy 6 image-to-3D `enable_rigging` | fal | image | rigged GLB at generation | bundled | Would rig during Object Creator generation instead of afterwards. |
| Tripo | fal lists segmentation (used by Model Parts), not rigging | — | — | — | Tripo's own API has rig/retarget tasks; not on fal, would need a second key. |
| UniRig (open model) | self-host only | mesh | skeleton + skin | GPU time | No hosted endpoint found; heavier to operate. |
| Mixamo | manual web upload | FBX/OBJ | FBX | free | No API; documented as a manual path (import result via Blender → GLB). |

## Decision

Use **Meshy Rigging on fal**: it reuses the existing fal key, upload, queue journal and
no-resubmit guarantee of Model Parts splits (`src/emergent-ide/src/lib/model-split.ts`,
`RIG_ENDPOINTS`). The IDE offers it only for unskinned `.glb` models, asks for confirmation
with the price, and saves the result through the same engine-checked import as a hand-imported
rigged model, so the rig report immediately shows whether the engine's humanoid map recognises
Meshy's bone names.

## Live run (2026-11-07, user-approved budget ≤ $2)

Driven through the IDE's own routes on a template-derived scratch project:

1. FLUX schnell reference (2 candidates): "full body … T-pose … front view, plain grey background".
2. Object Creator / Trellis → textured GLB, 6,417 vertices, 8,419 triangles, unskinned.
3. *Auto-rig humanoid* (Meshy Rigging) → queued → ready in about a minute → saved through the
   engine-checked import as `assets/models/explorer-rigged.glb` + entity.

Findings:

- **Skeleton:** 24 joints, Mixamo-style names without a prefix (`Hips`, `LeftUpLeg`, `LeftArm`,
  `LeftForeArm`, `LeftToeBase`, lowercase `neck`, extra `head_end`/`headfront`). All 21 required
  humanoid bones mapped with no name table.
- **Spine numbered downwards:** `Hips → Spine02 → Spine01 → Spine`. The name rules made `Spine02`
  the chest; `HumanoidMap::detect` now assigns spine/chest/upper chest by hierarchy (lowest
  first), with a regression test for both numbering orders.
- **Output:** 25,257 vertices (Meshy splits vertices per skin seam; triangle count unchanged) and
  one 0.033 s bind-pose clip `Armature|clip0|baselayer`, harmless next to BVH motion.
- **In game:** swapped for the mannequin in the template's third-person scene: idle, walk (W),
  run (Shift) and turn-to-strafe (D) all retarget from the template BVH takes with feet on the
  floor at 1.7 m scale, turning to face its movement, no manual fix-up. Native frames from `--preview-scene … --preview-ticks 40 --preview-keys …`.
- Cost: about $0.25 in total (two FLUX images, one Trellis, one Meshy rig).
- Still open: licence terms of Meshy output via fal; quality on non-T-pose or loose-clothing
  meshes; weights around the backpack/jacket were not inspected closely.

Sources: [fal Meshy Rigging](https://fal.ai/models/fal-ai/meshy/rigging/api),
[fal Meshy Rigging llms.txt](https://fal.ai/models/fal-ai/meshy/rigging/llms.txt),
[multi-animation](https://fal.ai/models/fal-ai/meshy/rigging/multi-animation/llms.txt),
[Meshy rigging docs](https://docs.meshy.ai/api/rigging.md),
[Meshy 6 image-to-3D](https://fal.ai/models/fal-ai/meshy/v6/image-to-3d/api).
