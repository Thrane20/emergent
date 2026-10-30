# fal model selection — macOS validation, 2026-10-30

- Added live searchable, paginated model selection to Image Generator, Texture Creator and Sprite options / image-to-video generation.
- Default endpoints remain available when catalogue browsing fails. Non-default models are checked against their current input/output schema before generation is enabled, and rechecked server-side before a paid submission is claimed.
- The selected endpoint and adapted settings survive saved job reload, polling and asset provenance; sprite animations still produce separate atlas variants within Sprite.
- Replaced frame-playback and fal-credit paragraphs with hover / keyboard-focus info tooltips, including links to the selected model's documentation and pricing.

## Evidence

- Real read-only catalogue requests returned 50 active image models and 50 animation models on the first page, with next-page cursors. Sample compatible schemas included Nano Banana Pro, Nano Banana 2 and Kling v3 Pro (`start_image_url`). Some other sample schemas required an adapter and were rejected. The unauthenticated discovery quota returned HTTP429 during repeated inspection; server caching and use of the saved fal key reduce discovery traffic.
- `pnpm test`: 108 passing IDE tests, including catalogue filtering/pagination/schema handling, incompatible models, selected-endpoint submission/status/result, and journal/provenance recovery.
- Targeted ESLint, TypeScript and `EMERGENT_NEXT_DIST_DIR=.next-fal-models-build pnpm run build`: passed.
- Production Chromium smoke on IPv6 port3001: compatible image and animation selection, incompatible-model blocking, hover/focus tooltips, real library writes, local atlas baking, source preservation, independent generation/deletion, particle assignment and mobile layout. Paid provider calls and catalogue browser responses were mocked. Real local ffmpeg/native image validation remained enabled. User service on port3000 was left running; test service stopped afterward.

## Limits

Model-specific parameter editors are not included. Alternatives use each model's defaults; existing 30-second / 2048-pixel video and image byte/format limits remain enforced after download. The catalogue is broader than the adapters: models requiring other inputs or unsupported response schemas cannot generate yet. No paid live fal generation or model-quality comparison was performed.

## Sprite options layout follow-up

Sprite options now uses a responsive two-column dialog, up to 1100 pixels wide. Sprite-sheet settings/preview sit beside animation model/prompt controls; actions span the bottom. Narrow screens retain a single scrollable column.

Shared info tooltips now use top-layer popovers with coordinates clamped to the visible dialog or viewport, escaping ancestor overflow clipping. Hover and keyboard focus work; Escape dismisses the tooltip before the dialog. Scrolling/resizing dismisses stale popup positions.

Validated with TypeScript, targeted ESLint, production build, and the production media-library Chromium smoke. At 1280x800 the entire desktop dialog fits without scrolling. Every modal info tooltip passed bounding-box and hit-testing checks on desktop and mobile; Escape retained the open dialog. Existing sprite configuration, animation, generation and deletion checks passed with fal mocked. Test service stopped; user port3000 untouched.
