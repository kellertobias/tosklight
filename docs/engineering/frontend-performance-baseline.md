# Frontend request and responsiveness baseline

This document records the request-path baseline measured before the frontend event and state-ownership refactor. It is an architecture contract, not a promise that polling at these rates is acceptable.

The measurement follows the complete operator-visible path from an input gesture through frontend orchestration, HTTP or WebSocket traffic, server mutation, event publication, reconciliation, and the state update that makes the result visible. Static request counts are exact for the code paths listed below. Elapsed-time acceptance remains covered by focused release-build and browser benchmarks as those boundaries are introduced.

## Baseline method

The baseline was captured on 2026-07-18 from the `refactoring` branch by tracing:

- every interval created by the mounted server provider and view projections;
- every request made by the global `refresh()` operation;
- every request family formerly selected by `stateEventRouting.ts` for each facade event;
- every global `refresh()` call site;
- the Add Fixture flow from `placementBatch.ts` through generic show-object mutation; and
- whether mounted-but-hidden panes and secondary screens continue to own subscriptions or polling.

Counts exclude browser cache behavior and server-internal reads. They therefore describe frontend network pressure, not the server-side persistence and compilation cost that follows each request.

## Idle and view polling

One authenticated `ServerProvider` polls even when the corresponding data is not visible:

| Projection | Interval | Requests per second |
| --- | ---: | ---: |
| Desk lock | 500 ms | 2 |
| Playbacks | 250 ms | 4 |
| Highlight | 2,000 ms | 0.5 |
| Matter, when enabled | 1,000 ms | 1 |

The base rate is therefore 6.5 GETs per second per main or secondary screen, or 7.5 with Matter enabled.

Mounted views add independent polling:

| View projection | Interval | Requests per second |
| --- | ---: | ---: |
| Selected-parameter visualization | 400 ms | 2.5 |
| Stage visualization | 200 ms | 5 |
| Fixture Sheet visualization | 250 ms | 4, or 8 for live plus Preload |
| Channels visualization | 250 ms | 4 |
| DMX view | 250 ms | 4 |

A representative active workspace reached roughly 30 GETs per second before mutations or legacy facade-event fan-out, and 31 with Matter enabled. Duplicate panes duplicated their rates. Maximizing one pane did not unmount the others, and a component being mounted did not prove that it was currently visible.

The visualization consumers do not share request de-duplication. Parameter Controls, Stage, Fixture Sheet, and Channels may request the same authoritative projection independently.

## Broad refresh cost

One authenticated global `refresh()` performs 17 GETs:

- bootstrap, patch, Playbacks, shows, configuration, fixture library, fixture profiles, fixture-profile warnings, and media servers; and
- eight complete show-object lists: Groups, Presets, Cuelists, routes, user layouts, Stage layouts, patch layers, and unresolved MVR fixtures.

There are 25 global broad-refresh call sites. Initial connection also eagerly fetches ten resource projections and the eight show-object lists, including fixture-library data for screens that never display the library.

## Legacy facade-event amplification

The pre-refactor event router has no cross-event coalescing or request single-flight. Its request fan-out is:

| Event | Follow-up GETs |
| --- | ---: |
| `programmer_changed` or `programmer_cleared` | 9 |
| `show_object_changed` | 12 |
| `show_opened` | 15 |
| `server_configuration_changed` | 11 |
| `show_renamed` or `show_rolled_back` | 10 |
| `preload_stored` | 10 |
| `preset_stored` | 9 |

A touch fader could publish Programmer mutations once per animation frame. At 60 Hz, the former nine-request Programmer event route could attempt roughly 540 GETs per second for each connected frontend. The retired compatibility socket broadcast the same event to every connected screen.

## Add Fixture operator path

Before the patch boundary is migrated, Add Fixture follows this path:

1. The Patch UI computes placement locally.
2. `placementBatch.ts` calls `patchFixture` once per fixture, serially.
3. Each call sends one generic `PUT` containing a complete fixture definition.
4. The server authenticates, opens the show, normalizes and validates the object, creates a backup, writes one object revision, recompiles or replaces active runtime state, and emits `show_object_changed`.
5. The mutation caller performs global `refresh()` independently of the event.
6. The event performs its own patch, Playback, bootstrap, Programmer, and eight-list reconciliation.
7. React installs the broad refreshed state and the Patch view eventually reflects the fixture.

One fixture therefore causes one mutation plus up to 29 follow-up GETs: 17 from the caller's global refresh and 12 from the event. A fixture count greater than one repeats the entire mutation and refresh path serially.

The target path is one revisioned `PatchFixtures` batch command, one show transaction, one compile and runtime swap, one typed patch event, and one targeted Patch-store delta. It must make zero bootstrap, fixture-catalog, media, show-list, configuration, Playback, or unrelated show-object requests.

## Refactor budgets

The migration is complete only when automated tests enforce these properties:

- Programmer events cause no show-object-list reads.
- One show-object event reconciles only its affected projection and ignores events for inactive shows.
- Bursts for the same projection are coalesced and older revisions cannot replace newer state.
- A Patch Add action sends one batch mutation and makes no unrelated refresh requests.
- Mounted views share authoritative projections and subscribe only while their capability and object scope is visible.
- Reconnect sequence gaps repair from a coverage-complete authoritative snapshot before incremental events resume.
- Optimistic overlays remain separate from authoritative base state, expose pending/error feedback, and roll back or reconcile by request identity and revision.
- Release-build p95 remains below 250 ms server-side and 500 ms from Patch action to visible UI for one fixture; a 100-fixture batch remains below 500 ms server-side.

Runtime benchmarks must record request count, payload bytes, mutation response time, event time, visible-paint time, persistence, compilation, runtime replacement, and projection reconciliation separately. This keeps a fast server response from hiding a slow operator-visible frontend path.

## First reconciliation checkpoint

The first event-routing migration kept facade compatibility while replacing broad discovery with affected-resource reconciliation:

| Event | Before | After |
| --- | ---: | ---: |
| `programmer_changed` or `programmer_cleared` | 9 GETs | 1 bootstrap GET |
| Supported generic `show_object_changed` | 12 GETs | 1 single-object GET |
| Patched-fixture object change | 12 GETs | 1 Patch projection GET |
| Playback or page object change | 12 GETs | 1 Playback projection GET |
| Route object change | 12 GETs | 1 route-collection GET |
| Explicit generic-object deletion | 12 GETs | 0 GETs |
| Irrelevant, malformed, unknown, or inactive-show object change | 12 GETs | 0 GETs |

Same-resource bursts coalesce to the latest event revision, and late responses cannot replace newer state. Route events intentionally read their small coupled collection because the current Patch projection omits route object identity; this keeps Output and Patch projections consistent without a broad refresh. Show open and rollback continue to perform full authoritative hydration.

This historical checkpoint did not make facade notifications the final authority. The compatibility event socket has since been retired in favor of typed v2 subscriptions; the remaining polling and reconciliation targets are tracked by the active refactoring plan.

## Warm-state checkpoint

The 2026-07-26 Plan 05 checkpoint measures the release frontend in Google Chrome at 1280×720. Each profile opens a disabled-warm-up control, then the production warm path. The operator sequence switches 42 built-in windows and toggles the Programmer/Playback command section 40 times. Chrome DevTools Protocol captures network bytes and a compressed trace; test-visible User Timing captures first usable paint, queue lifetime, snapshot activity, event lag, retained model bytes, long tasks, and input-to-paint samples.

The large generated show adds 200 Groups, 800 Presets, 100 Cuelists, 100 Playbacks, and 16 Playback Pages to `assets/demo.show`. The 4× cases apply Chrome CPU throttling. Their raw wall-clock values remain recorded; the acceptance comparison divides input-to-paint and long-task CPU time by four so the same 100 ms and 50 ms CPU-work budgets apply to both profiles.

| Show / CPU | First usable control → warm | Warm-up | Retained model | Warm switch p50 / p95 | Disabled / warm switch snapshots | Warm event lag | Warm transferred |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Demo / 1× | 281.2 ms → 191.5 ms | 238.6 ms | 3.13 MB | 14.7 / 65.7 ms | 85 / 0 | 0 ms | 9.07 MB |
| Demo / 4× | 948.1 ms → 590.0 ms | 570.9 ms | 3.13 MB | 30.3 / 302.4 ms raw; 75.6 ms normalized p95 | 85 / 0 | 2 ms | 9.59 MB |
| Large / 1× | 272.1 ms → 184.1 ms | 271.8 ms | 3.57 MB | 14.8 / 76.3 ms | 85 / 0 | 0 ms | 10.20 MB |
| Large / 4× | 914.1 ms → 588.7 ms | 700.0 ms | 3.57 MB | 30.4 / 335.8 ms raw; 84.0 ms normalized p95 | 85 / 0 | 3 ms | 10.73 MB |

All warm runs completed with a two-request queue peak, no visible loading placeholder during switching, and a 64 MB retained-model budget. Native traces contain no long task over 50 ms overlapping background warm-up. The raw 4× traces record 67 ms and 78 ms maxima, both below 50 ms when normalized for the configured throttle.

`performance.measureUserAgentSpecificMemory()` was unavailable in the test browser, so peak browser-process memory is `null`; the enforced fallback is serialized retained-model size. Quantitative evidence is browser evidence because the packaged Tauri WebView does not expose the repository’s Playwright control bridge. The real `npm run open` path separately built and launched both macOS bundles, returned ready from `/api/v2/readiness`, and rendered populated Fixtures, Presets, Cuelists, and Playback command surfaces through direct macOS accessibility interactions without an observed loading state.
