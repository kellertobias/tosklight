---
slug: frontend-slice
title: "State Ownership to Pixels: Snapshots, Events, and Repair"
components: [control-ui, ui-library]
order: 40
---

# State Ownership to Pixels: Snapshots, Events, and Repair

`apps/light-desktop/src/features/showObjects/` is the reference slice. The frontend rules look strict
until you know what each one prevents, so this page names the failure behind each.

Operator truth is in the pane and workflow chapters under `docs/help/`; the paired UI/API
scenarios described by `docs/testing/README.md` prove that a visible projection matches server
authority.

## Six lifetimes before React

Portable show, desk installation, desk interaction, user Programmer, connection/session, and
transient runtime have different owners. React receives immutable projections of those owners; it
does not merge them into one browser authority. [State Ownership](19-state-ownership.md) is the
normative matrix.

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

## Composition and authority replacement

`apps/light-desktop/src/api/ServerRuntime.tsx` and `features/server/` compose the stable connection and
focused capabilities. There is no production `useServer()` consumer. The small
`api/ServerContext.ts` export exists only for legacy test mocks.

Show, desk, session, or server replacement increments scope/generation. Sessions stop old
subscriptions, clear overlays that cannot belong to the new authority, and reject late responses.
Same-show reconnect keeps the last valid content mounted, shows a compact reconnect banner, repairs
the cursor, and resumes.

## Loading and errors

The first boot remains covered until connection, resources, stores, and event transport are ready.
Show switches use one tokenized busy authority so overlapping local actions and server events cannot
hide "Loading show..." early. Secondary Screen and Stage windows gate their first meaningful render
on their own scoped hydration. Accepted controls do not move when an error appears.

## Exercises

1. Mount a Cuelist pane with the network tab open. Confirm one snapshot request and one
   subscription. Cover the pane and confirm teardown.
2. Move a fader fast. Confirm only the newest pending value per target is in flight.
3. Edit the same object from two desks and watch the narrow repair.
4. Drop the WebSocket for ten seconds and watch gap detection and snapshot repair.
5. Pick one inactive pane and trace why it performs no hydration, subscription, or polling.
