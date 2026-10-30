# Shared fal balance header — 2026-10-30

Every editor and Settings/Player header now mounts the same `FalBalance` component. It displays the available account credit and its currency (USD from the live check), shows `$0` when no fal key is configured, and shows `Unavailable` for billing/network/authentication errors rather than reporting an invented zero. Hover text explains permission or connection failures. The badge can be clicked to refresh; visible pages also refresh each minute and after key settings change. Main-editor mobile headers wrap into two rows to retain visible controls and the balance.

`GET /api/fal-balance` reads `GET https://api.fal.ai/v1/account/billing?expand=credits` on the server with the existing saved/environment key. The required permission is `billing:usage:read`; fal documents BILLING/READONLY/FULL presets as including it. The browser receives only normalized balance/currency/status, never the key, account username or raw provider error body. Responses are private/no-store. Cached billing reads last60seconds, failures15seconds, explicit refreshes have a5second cooldown; concurrent reads are deduplicated. Key changes isolate cached data and a removed key avoids contacting fal entirely. Client requests are aborted when superseded or unmounted.

## Validation

- Real read-only billing request with the current saved key returned HTTP200 and USD8.76. No generation or account mutation was performed.
- 114 IDE tests passed. New tests cover no-key zero with zero network requests, billing parameters, concurrency/cache expiry, key replacement/removal, error privacy and permission/malformed-response handling.
- TypeScript, focused ESLint and production build passed.
- Production Chromium smoke on IPv6 port3001 verified the real local no-key API response and all editor/settings header placements, ready/no-key/unavailable states, click refresh, key-change event refresh, mobile badge visibility/layout, existing rail behavior and unsaved-navigation guards. Browser billing responses were mocked; no saves or paid calls were made. Screenshots: `/tmp/emergent-fal-balance-header.png`, `/tmp/emergent-fal-balance-mobile.png`.
- Test service stopped; user port3000 untouched. No Rust runtime or game-asset changes were needed.

Balance is an account-wide snapshot reported by fal, not a reservation or exact prediction of queued generation costs. Keys lacking billing permission continue to work for the model operations permitted by that key, while the badge explains that billing cannot be read.
