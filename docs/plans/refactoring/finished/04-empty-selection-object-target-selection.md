# Empty-selection Object Target Selection

## Status and source contract

Finished. Plan 02's application pool adapters and shared pool intent contracts are stable. Implemented
[`../../Done/56-empty-selection-preset-and-effect-target-selection.DONE.md`](../../Done/56-empty-selection-preset-and-effect-target-selection.DONE.md).

Do not run this plan concurrently with the active Storybook lane: it changes the same production
pool surfaces, adapters, interaction stories, and focused UI verification.

Estimated effort: 0.5–1 Codex day.

## Required work

1. Add one authoritative action that resolves the ordered targets stored by a populated Preset.
   Dynamics are explicitly outside this plan and retain the gesture defined by the canonical
   Dynamics plans.
2. With an empty programmer selection, the first ordinary tap selects those targets and does not
   recall or apply the object.
3. A second tap, or any tap with an existing selection, keeps ordinary recall/apply behavior.
4. Preserve Store/Record, Update, and Set precedence.
5. Include unpatched fixtures; skip missing targets with an actionable, unobtrusive warning.
6. Keep Stage, Fixture Sheet, command line, OSC, attached hardware, and all pool surfaces on the
   same authoritative programmer selection.

## Acceptance and verification

- Cover Color, Position, and Mixed Presets, ordered target unions, empty slots, missing targets,
  unpatched fixtures, armed modes, first tap, and second tap.
- Add domain/action tests plus API, OSC/hardware, and real UI Playwright parity checks.
- Verify shared pool components emit intent only and do not hold local-only selection state.

## Result

Implemented in the semantic-release commit
`feat(programmer): select preset targets on first tap`.

### Changes

- Added one application-owned Preset target resolver. It unions fixture and Group-owned values,
  expands whole fixtures to selectable logical heads, deduplicates targets, includes unpatched
  fixtures, skips missing identities, and reports an actionable warning.
- An ordinary Preset activation with an empty selection now installs one authoritative static
  selection without recalling values or changing the active Preset context. A second activation,
  or an activation with an existing selection, retains ordinary recall behavior.
- Preserved empty-slot behavior and the existing Update, Set, and Store/Record priority order.
- Kept Dynamics outside this gesture according to their canonical plan.
- Reworked the typed application, HTTP, WebSocket, generated wire, desktop writer, and pool
  adapters around a live Preset activation result that distinguishes `targets_selected` from
  `recalled`. Removed semantic replay identity from this live-control action while retaining
  WebSocket envelope correlation.
- Extended the semantic bench to prove the visible Preset pool, typed API, command line, Stage,
  Fixture Sheet, attached OSC hardware, and Highlight all converge on the same authoritative
  selection. Shared UI components continue to emit activation intent and hold no local selection.
- Updated operator help and regenerated and verified the PDF and offline HTML manuals.

### Tests

- All 413 `light-application` tests passed, including Color, Position, Mixed, ordered union,
  logical-head expansion, missing target, empty Preset, first-tap, second-tap, revision, event,
  and unchanged non-empty recall coverage.
- All focused Preset HTTP and WebSocket runtime tests passed. The broad
  `light-headless-runtime` run passed 472 tests with one ignored; its only failure was the
  sandbox-denied CITP socket test, which passed when rerun with the required local socket access.
- All 2,007 desktop Vitest tests passed. The focused Preset writer/transport set passed 18 tests,
  and the Preset priority plus Stage/Fixture Sheet projection set passed 23 tests.
- The real `BENCH-PRESET-001` Playwright scenario passed against the built frontend and live
  headless, API, WebSocket, OSC, Art-Net, and sACN test bench.
- Desktop typecheck, Rust formatting, focused Biome checks, generated wire/schema regeneration,
  and `git diff --check` passed.
- `npm run manual` rebuilt and verified the 142-page PDF and offline HTML manual.

### Limitations

- Persisted Presets currently store fixture and Group owners in maps and therefore carry no
  cross-owner target order. The resolver uses the documented deterministic active-desk fixture
  order fallback; no persisted-show migration or inferred ordering was introduced.
- Current attached controls expose OSC/keypad commands but no dedicated Preset pool buttons.
  Hardware parity therefore verifies that attached OSC actions observe and operate on the
  authoritative selection created by the pool/API activation; it does not fabricate unavailable
  hardware-pool evidence.
- Dynamics remain governed by their separate canonical gesture and were not changed.
