# Sprite animation variants — macOS, 30 October 2026

Starting Animate sprite keeps the Sprite filter active and immediately displays a
submission spinner alongside the source sprite. The existing scoped fal journal
records sprite output intent. Completed clips are automatically baked locally into
8×4, 32-frame atlases at 8 fps and saved as separate Sprite variants. Static source
pixels are unchanged. A family UUID in project image-library sidecars groups all
variants without replacing the engine's asset/runtime identity contracts.

Repeated generations have independent IDs and atlas assets. Variant deletion uses
the existing image deletion protections, hides its completed job, and does not
remove siblings. Deleting an unused static source leaves animations available;
historical generation provenance does not count as a runtime consumer. Actual
particle/material/entity references still block deletion.

Local baking and interrupted atlas configuration retry from the retained clip,
without resubmitting a paid request. Animated inputs use their first atlas cell
when requesting a further AI generation, rather than uploading a mosaic.

Validation:

- Full IDE unit suite: 104 tests passed.
- Focused sprite lifecycle/recovery test passed, including concurrent family
  assignment, submission idempotency, independent atlases, deletion, provenance
  handling and metadata recovery without regeneration.
- Production Next build, TypeScript and targeted ESLint passed.
- Updated media-library browser smoke uses a disposable project and production
  server: spinner, retained Sprite selection, static/animated variants, repeated
  generations, playback, per-variant/static deletion, reload, separate Video output,
  atlas assignment to emitters and mobile access.

Provider calls are mocked; local FFmpeg processing, image validation, metadata
writes and deletion run for real. Atlas extraction retains the generated video's
appearance; no automatic second paid background-removal request is submitted.
