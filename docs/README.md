# Documentation map

Each fact has one home. Other documents link to it and do not repeat it.

| Question | Home | Changes when |
| --- | --- | --- |
| Why does Emergent exist and what must it become? | [GOALS.MD](../GOALS.MD) | A goal, contract or stage gate changes; tick a stage checkbox when its slice lands |
| What is being worked on now and next? | [plan.md](../plan.md) | Work starts, finishes, is reprioritised or a known issue is found or fixed |
| What is delivered, and where is the evidence? | [implementation-status.md](implementation-status.md) | A slice lands: add or update its row in the same change |
| Detailed multi-stage plan for one area | [plans/](plans/) (e.g. [terrain](plans/terrain.md)) | Progress inside that area |
| Why was an architectural choice made? | [adr/](adr/) | A new binding decision; never edit an accepted ADR — supersede it |
| How is a feature designed? | [design/](design/) | Before implementing a substantial feature (AGENTS.md design brief) |
| Did it work on a real machine? | [platform/](platform/) | After native or browser validation; name OS, hardware and date |
| How fast is it? | [benchmarks/](benchmarks/) | After a measured run; record workload, profile and hardware |
| What did we investigate? | [research/](research/) | Feasibility spikes and comparisons |
| What happened before? | [history/](history/) | Frozen archives only — do not update |
| Which capabilities exist (machine-readable)? | [capabilities/engine-capabilities.toml](capabilities/engine-capabilities.toml) | A capability's status changes |
| User guide (HTML) | [docs/](docs/) | A public API or workflow changes |

## Rules

- **Finish means documented.** A change that completes a slice also updates its
  `plan.md` entry, its `implementation-status.md` row and, if applicable, its GOALS.MD
  checkbox and capability status.
- **plan.md stays short.** It holds Now / Known issues / Next / Backlog only. Finished
  work moves to the status ledger and does not stay in the plan.
- **No new root-level docs.** Area plans go in `docs/plans/`, archives in `docs/history/`.
- **Name files by topic, not by date or iteration.** Dates go inside the document.
- **Evidence beats claims.** Status rows link to a `platform/` or `benchmarks/` record. A
  row without one says so explicitly.
