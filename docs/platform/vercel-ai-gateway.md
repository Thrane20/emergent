# Vercel AI Gateway — research and validation

2026-11-03, macOS development workspace, Node 22.22.0. IDE-only provider extension;
no engine runtime, SlotMap ownership, template or game project changes.

## Verified upstream contract

- Vercel's product is **AI Gateway**. Its [Responses API](https://vercel.com/docs/ai-gateway/sdks-and-apis/responses)
  uses `https://ai-gateway.vercel.sh/v1`, Bearer Gateway keys and `creator/model` IDs.
  It supports tool calls, image input and JSON-schema output, so Emergent reuses its
  existing Responses transport for both embedded MCP chat and scoped editor requests.
- [Tool calling](https://vercel.com/docs/ai-gateway/sdks-and-apis/responses/tool-calling)
  uses `function_call` and matching `function_call_output` continuation items.
  [Structured outputs](https://vercel.com/docs/ai-gateway/sdks-and-apis/responses/structured-outputs)
  use `text.format`.
- [Dynamic discovery](https://vercel.com/docs/ai-gateway/models-and-providers)
  exposes an unauthenticated `GET https://ai-gateway.vercel.sh/v1/models` catalogue.
  A live read confirmed language-model types, `tool-use` / `structured-output` tags,
  names and creator-prefixed IDs. The UI received **264** matching models in this run;
  the count is a snapshot, not a fixed supported-model promise.

The catalogue proxy is fixed to Vercel's public endpoint, validates its response,
has a 15-second timeout, and sends no saved key. It does not infer account-specific
availability. Manual IDs remain available for custom models and discovery failures.
Generation uses the saved Gateway URL/key/model, with existing request timeouts,
execution limits, project binding and mutation validation. No dependency was added.

Settings remain in the existing Git-ignored IDE data directory, outside game assets.
The Gateway record is independent of existing providers; legacy settings migrate
without losing keys. Credentials are local JSON with owner-only file permissions
(`0600`), not encrypted storage. Public responses contain only masked hints.

## Checks

Commands from `src/emergent-ide`:

- `npx tsx --conditions=react-server --test --test-timeout=20000 tests/ai-settings.test.ts tests/ai-tool-conversation.test.ts tests/vercel-models.test.ts tests/ai-chat.test.ts tests/statement-ai-claude.test.ts`:
  **12 passed**. Covers fresh/legacy settings, private keys and file permissions,
  provider isolation, model validation, key removal, all three provider tool loops,
  continuation/reasoning/image/error results, incomplete responses and catalogue
  filtering/failures.
- `npx tsx --conditions=react-server --test --test-timeout=60000 tests/statement-ai-vercel.test.ts tests/mcp-chat.test.ts tests/chat-implementation.test.ts`:
  **3 passed**. Includes default `/api/chat` through the real MCP client/server and
  local Vercel Responses stand-in; structured schema submission, runtime model change,
  incomplete/authentication errors; existing staged compilation and conflict guards.
- `npx tsc --noEmit`: passed.
- Targeted ESLint on settings, catalogue, settings page, and touched/new tests: passed.
- `EMERGENT_NEXT_DIST_DIR=.next-vercel-build npm run build`: passed, including TypeScript
  and route generation. Two existing whole-project filesystem-tracing warnings from
  `inspection-session.ts` remain.
- Headless Chrome through Playwright, production server on **3001**, disposable data
  directory and fake key: live catalogue, model selection, save/reload, blank password
  after reload, masked public response, manual model/save during simulated catalogue
  outage, refresh recovery, key removal, and no page errors all passed. Screenshot
  visually inspected. Empty catalogue state also checked after the final UI adjustment.

The test service was stopped. The user's port 3000 service and real saved provider
settings were not changed. Temporary build output and generated TypeScript include
entries were removed after validation.

## Remaining validation

No Gateway key was supplied. Authenticated Vercel generation, billing/credits,
account-specific access, cross-model reasoning/image compatibility and model quality
remain unverified. Local stand-ins establish request/continuation behavior, not live
acceptance of every Gateway model. Models must support the capabilities used by each
request; manual IDs deliberately bypass catalogue capability filtering.
