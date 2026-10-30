# Image Generator and project image library

The `/images` page collects supported PNG, JPEG, and WebP files from every declared
asset root in the currently linked project. Tiles show the existing generated
display name or the filename without its extension. A native modal dialog provides
an enlarged preview, keyboard focus containment, and Escape dismissal.

Generation reuses the existing server-side `TextureJobs` journal and fal FLUX.1
Schnell provider. This page submits one square image with the plain prompt and
`tileFriendly: false`; it does not append texture-specific instructions. Existing
fal settings, queued status, uncertain-submission handling, bounded downloads,
native image validation, and restart history remain authoritative. The browser
retains the request ID and seed after a lost submission response so retrying the
same draft does not create another paid request.

Completed candidates stay in the job cache until explicitly added. Dragging a
candidate into the library and the accessible Add to library button use the same
existing idempotent save operation. Images enter the project's texture asset root
with durable UUIDs and provenance descriptors. Existing images are indexed in
place, without copying them. Local PNG/JPEG imports reuse the validated import API.

`GET /api/textures?projectId=…` returns only image paths, names, and modification
timestamps for the linked project. Existing project-asset access checks serve
previews. The library refreshes after saves and imports, and offers manual refresh
for external file changes. Project-relative image paths remain usable by existing
material tools and future main-system integrations. No new runtime engine image
type, global cross-project store, or automatic scene assignment is introduced.

Validation: image discovery/plain-prompt unit tests, existing texture job/provider
regressions, and a dedicated browser smoke with a temporary project and mocked fal
submission. The smoke exercises real native image validation, project publication,
drag/drop, modal keyboard behaviour, import, refresh/reopen, and lost-response retry.
No live paid request is required for these checks.
