# Emergent implementation plan

## Epic OC-1 — Object Creator with hosted TRELLIS

Status: **In progress — IDE workflow implemented; live fal acceptance pending**

Decision date: 2026-10-23

Progress: **3 of 5 implementation stages complete** (OC-1.2, OC-1.3, OC-1.4).
OC-1.1 has its design brief; OC-1.5 has local automated and browser validation.
Both remain open until real hosted generation is verified.

### Goal and agreed decisions

Let a developer generate a 3D object from a reference image, inspect it in the
Emergent IDE, and save the chosen result into the linked game project's assets.

- Use fal's hosted `fal-ai/trellis` image-to-3D endpoint initially.
- Start with image upload. Add text-to-reference-image creation later.
- Preview and save GLB models in the IDE first. Runtime importing and rendering
  in the game are a separate engine increment.
- Use the locally installed `@google/model-viewer` package for browser previews.
- Keep generation outside the Rust engine and its frame-critical game loop.
- Preserve durable project asset identities; resolve them to fresh SlotMap-backed
  runtime handles when runtime loading is implemented.

This epic changes the IDE authoring workflow and project asset metadata. It does
not make the engine's planned model importer, texture, or material capabilities
available.

### User workflow

1. Open **Object Creator** from the IDE navigation for the currently linked project.
2. Enter an object name and upload a reference image; inspect that image before
   submitting generation.
3. Generate and follow queued, generating, downloading, ready, or failed state.
4. Orbit, zoom, and reset the generated model in a large preview viewport.
5. Select a candidate from the project's generation history and **Save to project**.
6. Find the saved model and its dependencies in Files & Assets and the Project Map.
   Selecting a saved model reopens its preview.

### Implementation stages

#### OC-1.1 — Provider feasibility and design brief

- [x] Write a concise implementation design brief covering ownership, durable
  asset identities, data flow, resource limits, and validation before adding the
  substantial job/asset system.
- [ ] Use a configured fal account to submit one real image-to-3D request.
- [ ] Download the result, validate that it is GLB, and open it with the chosen viewer.
- [ ] Record the observed output format and integration result. The provider API's
  example file metadata is inconsistent, so actual GLB compatibility is an
  acceptance gate, not an assumption to carry into implementation.

#### OC-1.2 — Provider settings and persistent jobs

- [x] Add **Object generation** settings with a fal API key and connection status.
  Follow the existing server-only IDE settings pattern and expose only a masked
  key hint to the browser.
- [x] Add a small server-only provider adapter using `@fal-ai/client`, supporting
  only `fal-ai/trellis` initially. Upload images and submit requests through the IDE
  server.
- [x] Start with the documented provider defaults: 1,024-pixel textures and default
  sampling/simplification settings. Generate and record an explicit seed.
- [x] Persist local job IDs, provider request IDs, settings, timestamps, errors, and
  artifact locations under the IDE data directory, partitioned by project.
- [x] Poll while Object Creator is open; recover known jobs after page refresh or
  IDE restart without submitting another paid request.
- [x] Download completed artifacts into the local job cache. Preview and save from
  that cache rather than relying on provider URLs remaining available.
- [x] Handle transient status failures as recoverable. Show uncertain submission
  outcomes explicitly; never automatically resubmit a possibly accepted request.
  Retrying a confirmed failure is an explicit user action.

#### OC-1.3 — Object Creator interface and preview

- [x] Add Object Creator to the IDE navigation, bound to the linked project.
- [x] Provide object naming, reference-image upload/preview, Generate, status,
  useful errors, and project-specific generation history.
- [x] Load `@google/model-viewer` only in the browser and provide orbit, zoom, and
  reset-view controls for cached GLB output.
- [x] Keep unsuccessful attempts and unsaved candidates in generation history;
  do not add every attempt to the project's asset tree.
- [x] Validate GLB output before enabling Save to project. A successful provider
  response alone does not establish a valid model.

#### OC-1.4 — Save model bundles and connect the project graph

- [x] Save accepted objects as a bundle under a declared project asset root:

  ```text
  assets/models/<object-slug>/
    model.glb
    reference.<original-image-extension>
    object.asset.json
  ```

- [x] Define a versioned descriptor containing a durable asset ID, display name,
  project-relative model/reference paths, provider endpoint/request ID, seed,
  generation settings, and timestamps. Record the model revision when supplied;
  otherwise mark it unavailable. A recorded seed is provenance, not a guarantee of
  identical future hosted output.
- [x] Publish the complete bundle only after every file is ready. Never overwrite
  an existing object; request another name on collision. Repeated saves of the
  same candidate return the existing saved asset.
- [x] Add explicit descriptor-to-model and descriptor-to-reference graph edges.
  Support preview when selecting the saved model from Files & Assets.
- [x] Retain existing file-scoped chat and Statements support for saved assets.
- [x] Serve cached/project artifacts through scoped routes with validated paths,
  bounded uploads/downloads, and enforcement of the project's declared asset roots.
  Never persist runtime SlotMap or ECS keys.

#### OC-1.5 — Verification and documentation

- [x] Test missing/invalid credentials, failed generation, interrupted polling,
  refresh/restart recovery, and prevention of unintended duplicate submissions.
- [x] Test malformed model output, duplicate saves, naming collisions, incomplete
  bundle failures, and paths outside the linked project or cache.
- [x] Use mocked provider responses for automated tests; keep the real paid
  generation as an explicit integration check.
- [x] Run IDE lint, TypeScript checks, and production build.
- [ ] Verify live upload → generate → preview → save → reopen with fal. The same
  workflow, including server restart, passes with mocked generation and a real GLB fixture.
- [x] Update IDE/user documentation and capability documentation where applicable,
  distinguishing IDE previews from engine runtime model support.

### Epic acceptance criteria

- A developer can configure fal, upload an image, generate an object, inspect its
  GLB, and save it into `first_project`.
- Refresh/restart can resume a known generation; reopening the IDE can preview a
  saved model without depending on a temporary provider URL.
- Saved assets and provenance appear with meaningful connections in the Project Map.
- Errors preserve existing valid project assets, and provider credentials remain
  outside the project graph and browser responses.
- Generation and preview introduce no work into the Rust simulation/render loop.
- All verification above has recorded outcomes. No runtime model capability is
  claimed on the basis of browser preview alone.

### Dependencies and follow-up epics

Implementation needs a funded fal account and key configured through IDE settings
or server configuration. Never store that key in the game project or source control.

Later increments: text-to-reference-image creation; evaluation of TRELLIS.2;
runtime GLB importing and textured materials; model placement in authored scenes;
self-hosted generation workers. None are required to complete OC-1.

### Research references

- [Microsoft TRELLIS](https://github.com/Microsoft/TRELLIS)
- [TRELLIS text generation and GLB export example](https://github.com/microsoft/TRELLIS/blob/main/example_text.py)
- [fal TRELLIS API and queue contract](https://fal.ai/models/fal-ai/trellis/api)
- [Model Viewer](https://modelviewer.dev/)
- [TRELLIS.2 model card for later evaluation](https://huggingface.co/microsoft/TRELLIS.2-4B)

Research establishes documented feasibility. Actual hosted generation and local
compatibility of a real TRELLIS result remain unverified until OC-1.1 passes.

### Implementation evidence — 2026-10-23

- Design: `docs/design/object-creator.md`.
- Implemented `/objects`, object-provider settings, persistent job and artifact
  routes, browser GLB preview, safe bundle saving, and explicit project graph edges.
- Service tests cover journal recovery, duplicate paid-call prevention, failure
  handling, GLB validation, collision safety, path/symlink checks, and key masking.
- `pnpm test`, `pnpm exec tsc --noEmit`, `pnpm lint`, and `pnpm build` pass.
- `pnpm test:ui` passes against the production server using an isolated temporary
  game. Generation is mocked; the real GLB viewer, settings, save routes, server
  restart recovery, and project-tree selection are exercised in headless Chrome.
- Browser screenshot inspection caught a stale Turbopack CSS cache; a fresh build
  and an added computed-layout assertion verify the intended desktop layout.
- No fal key is configured in the current environment or IDE settings. No paid
  generation has been submitted. Enter a key in Settings → Object generation to
  perform the remaining provider acceptance check.
- No Rust engine/runtime model-import capability was changed or claimed.
