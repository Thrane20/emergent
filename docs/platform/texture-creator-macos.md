# Texture Creator — local integration evidence

25 October 2026, macOS / Apple M3 Pro. IDE authoring feature; no runtime renderer
or SlotMap ownership changes and no new dependencies.

Passed:

- 59 IDE tests, including six new texture job/provider/input tests; lint and types.
- Next.js production build with /textures and scoped texture job/artifact routes.
- `pnpm --filter emergent-ide test:texture-creator`: its own temporary project,
  distinct Cargo package and owned production server on port 3004. Mocked fal
  submissions/results; real native PNG validation, project saves, asset discovery
  and material assignment. No paid provider calls and no user asset changes.

Browser checks cover prompt/batch input, two returned candidates, selecting and
saving both, idempotent save retry, restored history after reload, narrow layout,
object material assignment and generated images in a terrain layer picker. The
editor screenshot was visually inspected. Temporary worker/server/project removed.

Unit checks cover one paid submission per UUID, persisted uncertain/restart state,
transient observation recovery, invalid image rejection, project-root scoping,
save recovery with stable identity/name, provenance sidecars, duplicate display
names, provider payload schema, download host restrictions and input limits.

The fal schema was checked at https://fal.ai/models/fal-ai/flux/schnell/api.
Production adapter uses the existing server fal key, queue API, square_hd PNG,
four steps, 1–4 images and safety checker enabled. Live account/model access and
actual generated texture quality were not exercised in automated checks.

Limits: latest 100 jobs shown; no automatic cleanup of unsaved generations;
seamless edges are not guaranteed by tile-friendly prompting. Terrain layer
painting/height blending remain separate. Windows/Linux native execution was
not checked in this run.
