# Unified media library validation — macOS, 29 October 2026

Image Generator now owns Image/Sprite labels, generation, Video import/generation,
background removal and atlas baking. Sprite is an image classification with
project-side playback settings. Right-click library tiles supports configuration,
an animated row-major preview, video generation and deletion. Unreferenced library
images can be deleted; direct and descriptor consumers remain protected.

Particle Factory uses the project Sprite library and copies saved rows, columns,
frame count and fps into the selected emitter. Its generation workshop was removed.
The existing renderer, simulation and canonical runtime ownership are unchanged.

Validated on macOS with Chrome and the production Next build:

- `pnpm test`: 103 IDE tests passed, including metadata bounds, grid divisibility,
  sidecar persistence, project/symlink isolation and deletion/provenance handling.
- Targeted ESLint and TypeScript checks passed.
- `EMERGENT_NEXT_DIST_DIR=.next-unified-media-build pnpm run build`: passed.
- `scripts/media-library-smoke.ts`: a disposable project exercises real label and
  atlas-setting persistence, frame cycling, reload, emitter assignment, video
  import/alpha baking, Sprite promotion, video submission guards and library deletion.
- `scripts/particle-smoke.ts`: native capture, atlas assignment/rendering,
  save/place/reopen, main-editor integration, swatch/discard flows and a native
  120-tick Run/resize/teardown smoke passed with media authoring moved to Images.

No paid fal requests were made. Provider calls were mocked; local FFmpeg baking,
asset writing, native texture validation and native particle rendering ran for real.
The existing native sprite contract allows up to 16 rows and columns and PNG/JPEG
textures; Image library discovery also retains WebP images. Sprite-sheet columns
and rows must divide the source dimensions evenly.

The existing Image Generator browser smoke also passed with the new type badges:
asset discovery, preview focus/Escape, plain prompts, lost-response idempotency,
drag/drop save, duplicate-save handling, image import, access checks, reload and
mobile layout. Saved generated previews read the same atlas metadata as their
library tile when reopening the options dialog. All owned test services were
stopped and temporary Next outputs/type includes were removed.

## Failed Video result deletion — 2026-10-30

Videos in `failed` state now expose Delete failed result, and the existing media delete endpoint accepts both ready and failed jobs. Failed background-removal cards are labelled Background removal rather than claiming a successful removal. Active/uncertain submissions retain the existing deletion guard. Deletion keeps the journal/submission claim marked deleted, so results stay absent after reload and cannot be resubmitted under the same identity. Existing saved-atlas/source preservation remains in effect.

Validated with115 passing IDE tests, TypeScript, targeted ESLint, production build and Chromium media-library smoke. The browser regression uses a persisted failed matte job with Unsupported fal download host, deletes through the real API, reloads and verifies the tombstone, with zero provider submissions during deletion. Existing saved assets remain protected. Removed the user's specific old failed lightning matte journal entry after checking its identity/state/error; no fal requests were made for that cleanup. The failed download itself was not retried. Test service on IPv6 port3001 stopped; user port3000 untouched.
