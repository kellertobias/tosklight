# Playback-runtime desk-scope refactor

Status: design + in-progress implementation (branch `refactoring`).
Owner note: this fixes the dominant remaining `@ui` acceptance gap — on-screen playback
**page changes (and cue/topology operations) silently abort when they overlap a benign
connection/session refresh**, e.g. right after attaching OSC hardware. Engine is correct
throughout (`@api`/`@osc` contracts pass); the defect is entirely in the frontend
playback-runtime scope lifecycle.

## Symptom

`OSC-001 @ui` (and the same class: `OSC-006 @ui`, `CUE-011/012 @ui`, `SHOW-000 @ui`,
`PLAYBACK-SELECT-001 @supplemental-ui`, and assorted `@ui` timeouts). The operator opens the
Playback pages menu and selects "Page 2"; the desk page stays 1. Depending on timing the modal
either silently closes (fast fail) or hangs at "Selecting Playback page…".

Server instrumentation proved the `PUT /api/v1/control-desks/{id}/page` **never reaches the
server** — the request is aborted in the frontend before it is issued.

## Root cause — a four-point cascade from one benign authority change

The frontend `authorityKey` is
`[serverUrl, connectionGeneration, session_id, client_id, user_id].join("|")`. It **conflates two
different things**:

- **Desk identity** — `showId` + `deskId`: *which* desk/show this runtime controls.
- **Authority token** — `connectionGeneration` (bumps on every (re)connect via
  `bootstrapConnection`), `session_id`, `client_id`, `user_id`: *how* we are currently connected.

The playback runtime is **desk-scoped** (a desk's pages/playbacks are shared across sessions and
survive reconnects). But it currently rebuilds itself on **any** `authorityKey` change, including a
same-desk reconnect/session-token refresh. When such a refresh lands while a Page menu is open, this
cascade aborts the in-flight operation:

1. **Store reset** — `PlaybackRuntimeStore.reset(showId, deskId, authorityKey)`
   (`features/playbackRuntime/store.ts:42`) only early-returns when show **and** desk **and**
   `authorityKey` are all unchanged. A same-desk key change therefore does a *full* reset: clears
   `deskState` (→ `beginOptimisticPage` returns null), bumps `scope` (→ `isCurrent(scope)` false),
   and sets status `loading`.
2. **Writer/session recreation** — `PlaybackRuntimeViewProvider`
   (`features/playbackRuntime/PlaybackRuntimeView.tsx:76,92`) memoizes `session` and `actions` with
   `authorityKey` in the deps, so both are rebuilt on the key change even though the writer only
   needs `showId`/`deskId` + a live client ref (`applyDeskPage`/`applyAction` are stable
   `useCallback`s over `playbackClientRef.current`).
3. **Readiness flips** — the modal's `ready` gate (`PlaybackPageMenu`) requires
   `runtimeStatus === "ready"` && `playbackDesk !== null`; the reset in (1) makes both briefly false,
   so the in-flight `select()` bails (`token == null`) and never issues the PUT.
4. **Page menu authority close** — `useOpenedPageMenuAuthority`
   (`components/control/playbackPageMenuLifecycle.ts:4`) closes the menu when either writer it opened
   against (`createPage` from `usePlaybackTopologyActions`, `setActivePage` from
   `usePlaybackRuntimeActions`) is replaced. (2) replaces `setActivePage`; the **PlaybackTopology
   provider** replaces `createPage` the same way. So the menu closes mid-selection.

Two individually-correct targeted patches (store.reset early-return on same-desk; drop `authorityKey`
from the `actions` memo) were each verified **safe** (cross-desk isolation `OSC-003 @ui` still passes)
but **insufficient alone** — the cascade still fires through the remaining points. Hence a coordinated
change.

## The refactor: split desk-identity from authority-token

Make every playback-desk-scoped provider treat **`showId` + `deskId`** as the identity that gates
reset/recreation, and treat an **authority-token change on the same desk** as a non-disruptive
refresh (the transports/writers already read the current client via a ref).

Concretely:

1. **`PlaybackRuntimeStore.reset`** — when `showId`/`deskId` are unchanged, adopt the new
   `authorityKey` in place (no `scope++`, no `deskState.reset()`, no `status: loading`). Full reset
   only on a real show/desk change. The authoritative snapshot re-hydrates over the same identity.
2. **`PlaybackRuntimeViewProvider`** — key the `session` and `actions` memos, and the `store.reset`
   layout-effect, on `showId`/`deskId` (drop `authorityKey`). The writers apply to a desk via a live
   client ref, so a reconnect/session-token change must not replace them. (Keep `authorityKey` passed
   into `store.reset` as the value to adopt, but not as a recreation trigger.)
3. **PlaybackTopology provider** (`api/usePlaybackTopologyBoundaries.ts` +
   `features/playbackTopology/PlaybackTopologyProvider.tsx`) — the same treatment for `createPage`
   and the topology writer/transport, so `createPage`'s identity is stable across a same-desk
   authority refresh. This is what stops point (4) once (2) is done.
4. **`useOpenedPageMenuAuthority`** — no change needed once (2)+(3) make the writer refs stable across
   benign refreshes; the guard still correctly closes the menu when the **desk identity** actually
   changes (writers are then genuinely replaced).

Guiding invariant: *the playback runtime resets and rebinds on show/desk identity changes only;
connection/session-token changes are transparent refreshes handled by re-hydration over the same
identity.*

## Non-goals / careful bits

- Do **not** weaken cross-desk isolation: a different `deskId` (or `showId`) must still fully reset
  and bump `scope`. The isolation tests (`OSC-003`, cross-desk `CROSS-*`, desk-lock `MANUAL-019`,
  `PLAYBACK-SELECT-001`) are the guardrail.
- Do **not** change server behavior; the engine is correct.
- The `scope`/`isCurrent` mechanism still exists and still drops actions across a *real* authority
  (desk/show) change — we only stop bumping it on same-desk token refreshes.

## Verification plan (gate every step)

1. `OSC-001 @ui` must go green (the canonical repro).
2. Isolation guardrail must stay green: `OSC-003`, `PLAYBACK-SELECT-001`, and any `CROSS-*` /
   desk-lock `MANUAL-019` cases.
3. Unskip and re-check the same-class `@ui` cases the fix should also clear: `OSC-006`, `CUE-011/012`,
   `SHOW-000/SHOW-001` fader/topology paths, `PRELOAD-004`.
4. Full `tools/test.sh e2e`, run in isolation for any newly-suspect case (the bench worker is flaky).
5. `npm run test:unit` for the playback-runtime store/view unit tests.
6. Revert on any regression; land only with a clean full-suite delta.

## Files

- `apps/control-ui/src/features/playbackRuntime/store.ts` (`reset`)
- `apps/control-ui/src/features/playbackRuntime/PlaybackRuntimeView.tsx` (`session`/`actions`/reset effect)
- `apps/control-ui/src/api/usePlaybackTopologyBoundaries.ts` + `features/playbackTopology/PlaybackTopologyProvider.tsx` (topology writer / `createPage`)
- `apps/control-ui/src/components/control/playbackPageMenuLifecycle.ts` (verify no change needed)
- Tests to unskip as they pass: `04-osc-*`, `06-cuelist-*`, `06-preload-*`, `00-generate-show-files`, `28-hardware-connected-playback-selection`.
