# Texture Creator — prompt to project texture

A /textures editor reuses Object Creator layout, local fal credentials, queue SDK,
single-submission transport, project binding and native PNG/JPEG import. Fixed
first endpoint: fal-ai/flux/schnell, square_hd, four inference steps, PNG, safety
checker on; one to four candidates, explicit seed, optional tile-friendly prompt
suffix. Tile-friendly prompting is not a guarantee of seamless edges.

Job journals live under IDE data, scoped by project ID plus canonical root.
Persist submitting intent before the paid call, claim each UUID exclusively,
never automatically resubmit after uncertainty/restart, and resume status/download
using the stored request ID. Only allow HTTPS fal.media downloads without redirects
and cap each image at the current 16 MiB/native dimension limits. Download and
validate before exposing ready previews. Failures distinguish terminal rejection
from transient observation failures; checking status never submits another job.

Each candidate has a durable UUID and optional saved path. Explicit save copies
it through the existing native texture importer into the project asset root,
with a provenance sidecar (prompt, seed, endpoint/request ID, candidate index; no
credentials). A fixed save identity and persisted name permit idempotent recovery
without overwriting unrelated files. Multiple selected outputs save independently;
partial success stays visible and can be retried. Shared image helpers remain
usable for manual uploads.

Project image discovery feeds generated PNGs into existing diffuse and terrain
layer pickers. No renderer, runtime identity, brush painting or height blending
changes. Validation uses mocked provider transport and temporary jobs/projects;
no paid fal call or user asset mutation is needed for automated checks.

Provider schema: https://fal.ai/models/fal-ai/flux/schnell/api (checked 25 Oct 2026).

Validation: [local integration evidence](../platform/texture-creator-macos.md).
Returning from the project rail retains its active scene context. Generated image
sidecars supply human display names while file/folder identities remain unique.
