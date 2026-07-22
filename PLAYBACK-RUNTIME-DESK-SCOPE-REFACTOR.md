# Playback page-change fix (postmortem)

Status: **fixed** (branch `refactoring`). This file first proposed a large "desk-scope refactor";
that was based on a wrong root-cause hypothesis. The actual bug was a one-line binding defect. This
postmortem records both so the analysis trail is not lost.

## Symptom

On-screen playback **page changes silently did nothing** (`OSC-001 @ui`, `OSC-006 @ui`/`@osc`, and
page-change steps in other `@ui` cases): the operator opens the Playback pages menu, clicks "Page 2",
and the desk page stays 1. `@api`/`@osc` contracts passed, so the engine was correct. Server
instrumentation proved the `PUT /control-desks/{id}/page` never reached the server.

## Actual root cause — unbound method (`this` lost)

`PlaybackPageMenu.select()` (and `add()`, and `usePlaybackPageControl`) obtain the action by
**extracting the method reference**:

```ts
const setActivePage = runtimeActions?.setActivePage; // runtimeActions is the writer instance
await setActivePage(number);                          // called unbound -> `this` is undefined
```

`PlaybackRuntimeActionWriter` methods are ordinary class methods and the `ActionsContext` provides
the **raw writer instance**, so the extracted reference lost its `this`. Inside, `setActivePage`
immediately hit `this.setActivePageNow` / `this.options` on `undefined` and threw, the surrounding
`catch` then threw again on `this.rejectSetup`, and the promise rejected before any request was
issued — a completely silent on-screen failure. (The topology actions did **not** have this bug: that
provider hands out a plain object whose methods are `writer.method.bind(writer)`.)

Pinned with browser-console instrumentation forwarded through `page.on("console")`: `select()` logged
`ready=true` but `setActivePageNow` never logged its (synchronous) entry — proving `this` was gone.

## Fix

Bind the public action methods in the writer constructor so any extracted reference keeps `this`:

```ts
// PlaybackRuntimeActionWriter constructor
this.setActivePage = this.setActivePage.bind(this);
this.poolPlaybackAction = this.poolPlaybackAction.bind(this);
this.releaseCueListSource = this.releaseCueListSource.bind(this);
this.setGroupMaster = this.setGroupMaster.bind(this);
this.setGroupFlash = this.setGroupFlash.bind(this);
```

This fixes every extraction site at once (there were several) rather than patching each call, and
matches the topology provider's already-bound pattern. Files: only
`apps/control-ui/src/features/playbackRuntime/actionWriter.ts`.

Verified: `OSC-001 @ui`, `OSC-006 @ui`+`@osc`, `PLAYBACK-SELECT-001` green; isolation `OSC-003`
(cross-desk) still green.

## The wrong hypothesis (recorded so it isn't re-attempted)

Before finding the binding bug I hypothesised a "benign authorityKey change (reconnect/session-token
refresh on the same desk) resets the scoped store / recreates the writers / closes the Page menu"
cascade, and implemented a coordinated refactor across `store.reset`, `PlaybackRuntimeView` memos, and
the topology/zones transports (live-token getters). Every piece was internally correct and isolation
stayed green, but it **did not fix the test** — because the real fault was upstream (the method never
ran). That refactor was fully reverted. If a genuine same-desk-reconnect issue ever surfaces, the
live-token / desk-scoped-reset ideas are a reasonable starting point, but they are **not** needed for
this bug.

## Note for the remaining skipped `@ui` cases

CUE-011/012 (cuelist settings/renumber), SHOW-001 (fader-bank assignment), etc. go through the
**topology** actions (already bound), so they are a *different* problem and are not fixed by this
change — re-investigate each on its own rather than assuming the page-change root cause.
