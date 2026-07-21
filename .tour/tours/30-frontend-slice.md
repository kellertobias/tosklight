---
slug: frontend-slice
title: A Frontend Slice in Detail
components: [control-ui, ui-library]
order: 40
---

# A Frontend Slice in Detail

`apps/control-ui/src/features/showObjects/` is the reference slice. The frontend rules look strict
until you know what each one prevents, so this page names the failure behind each.

## contracts.ts

Types and the port the store depends on. The store never imports a transport implementation, so it
is testable without a network.

## transport.ts

Snapshot fetch over HTTP, subscription to `/api/v2/events`.

- **Strict decoding.** Undeclared fields are rejected at every snapshot, outcome, error, projection,
  value, and event-envelope boundary. Prevents a server change producing quietly wrong UI state
  instead of a loud failure.
- **Hydration independent of socket readiness.** Prevents a blank pane while the WebSocket connects.

## store.ts

Revisions, watermarks, optimistic overlays, reconciliation, gap repair.

| Behaviour | Prevents |
| --- | --- |
| Overlay keyed by request identity | Rolling back the wrong write when two are in flight |
| Reconciles either response-first or event-first | A race leaving a stale overlay pinned |
| Narrow repair after a conflict | A full reload wiping unrelated optimistic work |
| Gap detection then snapshot repair | Rendering a state that never existed |
| Malformed events fail closed | A poisoned reconnect loop |
| Selectors suppress unrelated rerenders | A fader movement re-rendering the desk |

Writer policy follows the gesture:

- Continuous (fader, colour drag): retain only the newest pending value per target.
- Ordered barrier (range entry, release, Position Home): must not reorder.
- FIFO with one safe retry (selection): order is operator intent.

## session.ts

Reference-counted lifecycle. The first mounted view hydrates and subscribes; the last to unmount
tears down.

Two subtleties:

- React StrictMode replays effects in development, so disposal must survive replay. Reused sessions
  stay live after the mount cycle while replaced authorities stop promptly. See
  `features/shared/useStrictModeSafeStop.ts`.
- On a server, session, or show change the scope resets and late work from the old scope is
  rejected.

## Dormancy

An inactive pane performs no hydration, opens no socket, subscribes to no selectors, and does no
visualization polling or hardware-listener work.

A desk runs many panes across several screens for hours; eager panes compound into missed frames on
the output path. Separately tested: mounting the global provider performs no request, action-only
consumers stay dormant, covered panes do not subscribe, desk-only views request no runtime
identities.

## No stale fallback

While scoped authority is loading, show loading. Never fall back to stale bootstrap values.

Prevents the operator seeing an empty Programmer, believing their work is gone, and redoing it
during a show.

## Immediate or explicit

Every action either updates visible state promptly, shows a pending state, or opens a progress modal
for slow work such as loading, importing, validating, compiling, or migrating a large
show. Background work publishes success, progress, cancellation or retry where applicable, and
actionable errors.

The operator must never be left guessing whether an action was accepted, still running, failed, or
completed.

## The legacy shape

`apps/control-ui/src/api/ServerContext.tsx` and `features/server/` hold transport, auth,
reconnection, event routing, cached state, optimistic mutations, errors, and nearly every feature
command in one context exposed to most of the UI. An event arrives, everything refetches, everything
rerenders.

`features/server/useServerFeatureStores.ts` instantiates the new stores outside that path. The split
contexts (`ServerCoreContext`, `ServerFixtureContext`, `ServerPlaybackContext`,
`ServerProgrammingContext`, `ServerShowContext`) are the decomposition in progress.

`useServer()` is scheduled for deletion.

## Exercises

1. Mount a Cuelist pane with the network tab open. Confirm one snapshot request and one
   subscription. Cover the pane and confirm teardown.
2. Move a fader fast. Confirm only the newest pending value per target is in flight.
3. Edit the same object from two desks and watch the narrow repair.
4. Drop the WebSocket for ten seconds and watch gap detection and snapshot repair.
5. Find a pane still on `useServer()` and sketch its migrated slice.
