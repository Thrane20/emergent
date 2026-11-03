# Theme colour roles

2026-11-03. Charcoal now defines purple primary (#b79aff, hover #cbb5ff) and cyan secondary (#54dbe8, hover #87edf5), each with foreground and soft-surface tokens. Original green receives matching semantic roles using its existing palette. Existing saved theme preferences and theme IDs are unchanged.

Add Component explicitly uses `action-primary`. Shared save/create/composer actions use the primary colour; focus, selection and secondary accents use cyan. `action-secondary` supplies a reusable secondary action treatment. Existing sage/aqua aliases map to primary/secondary in Charcoal, keeping shared editor controls consistent. Appearance names the palette Purple & cyan. Preview images/material/shader data are not recoloured.

Validation: two theme unit tests pass, including existing initialization/storage boundaries and WCAG 4.5:1 text contrast for both foreground pairs. TypeScript and focused ESLint pass. Chrome smoke on a disposable project verifies the actual Add Component background/foreground, cyan token, purple hover and switching to green/back to Charcoal. Screenshot inspected. Test worker and server stopped and fixture removed. User port 3000 untouched. No Rust changes; native tests and a production build were not repeated for this palette update. Safari/Firefox were not tested.
