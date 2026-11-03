# AI balances in shared headers

2026-11-03, macOS, Node 22.22.0. IDE-only change, using the existing shared fal
header component; no game assets, templates, engine ownership or frame paths change.

## Provider contracts

- Vercel's [official spend reference](https://github.com/vercel/vercel-plugin/blob/main/skills/ai-gateway/references/spend-observability.md)
  documents `GET https://ai-gateway.vercel.sh/v1/credits` with a Gateway key.
  It returns the team's balance and cumulative usage. Only balance is displayed.
- OpenAI's [usage API](https://platform.openai.com/docs/api-reference/usage)
  documents usage/cost reporting, not a remaining prepaid-credit endpoint for the
  saved inference key. Anthropic's [usage/cost API](https://platform.claude.com/docs/en/manage-claude/usage-cost-api)
  reports historical usage/cost and requires administrative credentials. Neither
  spend figure establishes remaining credit. Their configured accounts show
  **Billing ↗**, with an explanation in the tooltip and a direct billing link.
  No undocumented Console endpoints or browser-session credentials are used.

`GET /api/ai-balances` reads only local configured-provider metadata, including
inactive providers. `POST /api/ai-balances`, invoked by the small refresh icon,
reads Vercel credit. Initial mount, navigation, focus, visibility, timers and settings
changes do not trigger AI provider balance requests. Settings saves invalidate the
old snapshot and reread local metadata. Navigation starts with an unchecked snapshot.
The existing fal timer/click behavior remains independent.

Keys stay server-side. Queries use the fixed official Gateway URL, reject redirects,
time out after 10 seconds and bound response bodies to 32 KiB. Custom Gateway URLs
show a billing link rather than sending a potentially unrelated key to Vercel.
Responses are private/no-store, include no key/account fields/raw error bodies, and
use `Unavailable` for failures, never an invented zero. Zero and negative balances
reported by Vercel remain valid values. Tooltips include the last successful check
time; badges are snapshots rather than live promises of account/model eligibility.

The shared header keeps fal and the icon visible on mobile; AI badges can scroll
horizontally within the remaining space. The refresh control has an accessible name,
keyboard focus styling and is disabled while its request is pending.

## Validation

From `src/emergent-ide`:

- `npx tsx --conditions=react-server --test tests/ai-balances.test.ts tests/ai-settings.test.ts tests/fal-balance.test.ts`:
  **10 passed**. Includes zero provider calls for metadata reads, no-key handling,
  explicit refresh/authentication, unsupported providers, configured-key discovery,
  malformed/oversized bodies, finite balances, network/auth failures and custom URLs.
- `npx tsc --noEmit`: passed.
- Targeted ESLint on the balance modules, route, components, settings, tests and smoke
  script: passed.
- `EMERGENT_NEXT_DIST_DIR=.next-ai-balances-build npm run build`: passed, including
  TypeScript and route generation; existing inspection filesystem-tracing warnings
  remain unrelated to this change.
- `npx tsx scripts/ai-balances-smoke.ts`, against a production server on **3001**:
  configured-provider badges, placement beside fal, no refresh on three minutes of
  simulated time/focus/visibility, explicit refresh, timestamps, errors/retry,
  credential-change invalidation, no-key hiding and desktop/mobile layout passed.
  Provider replies were mocked. Desktop/mobile screenshots were visually inspected.
- The isolated no-key balance API returned an empty list and private/no-store headers.

Test service stopped after validation; generated production output/type includes
removed. User port 3000 and real provider settings were not changed. No live billing
reads or paid model requests were needed for this change.

Prior live read-only diagnostics established that the user's saved Gateway key
authenticated and returned about USD 4.99. Authenticated model discovery marked
`anthropic/claude-sonnet-4.6` ineligible with `plan_restricted`. A positive balance
does not establish paid-tier access; account scope/entitlement remains unresolved.
