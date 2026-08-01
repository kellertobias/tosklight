# Schedules

## Status

**Finished — refactoring queue item 23.**

## Existing Storybook UI design

The Scheduler UI design is already more or less complete in Storybook:

- [`SchedulerWindow.stories.tsx`](../../../../apps/light-desktop/src/windows/SchedulerWindow.stories.tsx)
  is the application-level design and interaction story;
- [`SchedulerWindow.tsx`](../../../../apps/light-desktop/src/windows/SchedulerWindow.tsx) defines the
  current Scheduler window component contract; and
- [`ScheduleEditorTabs.tsx`](../../../../apps/light-desktop/src/windows/schedulerStory/ScheduleEditorTabs.tsx)
  contains the current schedule-editor controls and workflows.

These components use local Storybook state and are design/prototype evidence, not proof that
Schedules exist in production. During implementation, use these components directly where
possible and evolve them into production components instead of recreating the reviewed UI. A
component may be replaced only where production integration, accessibility, shared-component
adoption, or the authoritative behavior contract requires it.

The backend must implement the capabilities exposed by the reviewed UI design, including:

- fixed date/time and recurring schedules;
- selected weekdays, minute and day intervals, ordinal monthly weekdays, and custom calendar
  expressions;
- enabled/disabled state plus create and edit workflows;
- Macro targets and explicit page/playback targets;
- Playback actions **Go**, **Pause**, **On**, **Off**, **Release**, and **Toggle**;
- optional Playback-master level and fade-time changes; and
- the calendar/list presentation, next occurrences, and status/result state.

The production UI must replace local mock state with authoritative application services,
persistence, validation, API calls, and semantic events. If a Storybook interaction is found to
be intentionally illustrative rather than a supported product capability, that exception must be
resolved explicitly in this plan before implementation; it must not be silently omitted from the
backend.

## Goal

Allow an operator to define show-owned Schedules that automatically perform an action at a
configured time. The first action family is the reviewed set of Playback actions. Once Macros
exist, a Schedule must also be able to start a Macro by stable identity.

A Schedule can trigger in one of three ways:

- at a fixed interval, such as every five minutes;
- from a calendar expression, such as every Monday at 14:00, the first day of every month, the first Monday of every month, every second day, or another cron-style rule; or
- once at a fixed point in time, such as 31 December at 23:59:59.

Schedules are wall-clock automation. They are separate from Cue timing, Chasers, Timecode, Follow/Hang timings, and manual Playback controls.

## Operator workflow

The operator can create, edit, enable, disable, duplicate, and delete Schedules from a show-level schedule view. Each Schedule must show its name, enabled state, trigger type, next occurrence, target action, last result, and any validation or missed-run warning.

When creating a Schedule, the operator chooses:

- the trigger type: Interval, Calendar expression, or One-time;
- the trigger configuration;
- the target action: a supported Playback action or, once Macros exist, Start Macro;
- the referenced Playback or Macro;
- whether the Schedule remains enabled after a successful one-time occurrence.

The editor must preview upcoming occurrences before the operator saves the Schedule. The preview
uses the authoritative server/desk timezone; timezone is not a per-Schedule choice. Invalid
expressions, nonexistent dates, missing target actions, deleted Playbacks, or an unavailable Macro
runtime must block activation with an actionable message.

## Trigger types

### Interval

An Interval Schedule fires repeatedly after a configured duration, such as every five minutes. Its
persisted anchor is the instant at which it most recently became enabled. The UI displays that
anchor in the authoritative server timezone. Occurrences are calculated as exact multiples from
the anchor, never from completion of the previous action, so a slow or failed action cannot cause
drift. Editing the duration while enabled establishes a new anchor; editing unrelated fields does
not.

The minimum interval is one minute. When time advances across one or more missed multiples, the
scheduler records at most one bounded skip summary and advances arithmetically to the first future
multiple. It never enumerates or dispatches an interval backlog.

### Calendar expression

A Calendar-expression Schedule fires according to a wall-clock recurrence rule. The guided editor
stores a typed rule and the advanced editor uses the documented **ToskLight five-field calendar
expression**:

`minute hour day-of-month month weekday`

Each field accepts `*`, a number, a comma-separated list, an inclusive range, or a positive step.
Weekday uses `0` for Sunday through `6` for Saturday. A weekday may additionally use `#1` through
`#4` for an ordinal weekday or `L` for the last such weekday in a month. Seconds are deliberately
unsupported in recurring expressions. A rule constraining both day-of-month and weekday is
rejected instead of inheriting the differing OR/AND behavior of common cron dialects. Names,
macros, Quartz-only fields, years, and other dialect extensions are rejected with a field-specific
message.

The operator-facing model supports rules equivalent to:

- every Monday at 14:00;
- the first day of every month;
- the first Monday of every month;
- every second day from a typed calendar anchor; and
- other cron-style recurring dates and times that the selected expression format can represent.

The guided every-N-days rule is anchored to a persisted local calendar date and does not use the
resetting `*/N` day-of-month meaning. The desk rejects impossible dates, field overflows,
unsupported ordinal forms, and all other unsupported or ambiguous expressions before activation.

### One-time

A One-time Schedule fires once at a specific date and time, including seconds. After it runs
successfully, the Schedule disables itself and remains as a completed historical record.

An enabled One-time Schedule in the past is invalid and cannot be saved or activated. An operator
may explicitly save the same definition disabled for recordkeeping; it never fires immediately or
after a restart.

## Scheduled actions

### Playback action

The reviewed UI exposes **Go**, **Pause**, **On**, **Off**, **Release**, and **Toggle**, plus an
optional Playback-master level and fade time. The implementation must route each supported action
through the same authoritative Playback service and reject combinations that do not apply to the
selected Playback.

The target stores all three parts of the reviewed picker result: explicit page, explicit slot, and
the stable pool Playback number resolved there when the operator saves. Before every occurrence,
the server verifies that the page and slot still resolve to that same stable Playback object. An
empty slot, a replacement in the same slot, moving the Playback elsewhere, deleting it, or changing
it to an incompatible target makes the Schedule invalid until the operator explicitly retargets
it; it never follows the new occupant silently.

The initial action/type matrix is:

- Cuelist and Dynamic Playbacks support **Go**, **Pause**, **On**, **Off**, **Release**, and
  **Toggle**, plus an optional master transition;
- Group Playbacks support **On**, **Off**, **Release**, and **Toggle**, plus an optional master
  transition; **Go** and **Pause** are invalid;
- Speed Group, Programmer Fade, Cue Fade, and Grand Master Playbacks are not valid Schedule targets
  in the first release because their reviewed actions do not have one unambiguous meaning; and
- an empty, unsupported, deleted, moved, or replaced Playback target is invalid.

An optional master transition is one authoritative Playback action containing the target level and
fade duration. Fade is bounded to 0–60 seconds. A zero-duration transition is immediate. The
Playback application service owns this behavior for every caller; the Scheduler must not emulate a
fade by sending repeated fader actions.

The Schedule must use the same authoritative Playback service as UI, keyboard, OSC, hardware controls, HTTP, and Cue-triggered playback actions. Scheduled execution must therefore produce normal playback state, feedback, audit, events, and error handling.

### Start Macro

Once Macros exist, a Schedule can start a Macro by stable Macro identity. Scheduled Macro execution must use the same Macro service as manual, Cue, Playback, Timecode, HTTP, OSC, or other supported Macro triggers.

Until refactoring item 32 supplies that service and stable Macro identities, the persisted target
variant and execution port remain forward-compatible but Macro selection is visibly unavailable and
cannot be activated. Item 23 is complete when that unavailable boundary is tested; the executable
same-Macro-service acceptance belongs to item 32 and must not be claimed early.

The Schedule feature must not define Macro language, package, permission, Programmer, dialog,
duplicate-instance, or lifecycle behavior. Those semantics are owned exclusively by
[Macros](../pending/32-macros-and-scheduled-macros.md). This plan supplies the wall-clock occurrence,
resolved desk-alias audience, and Schedule source context to that service. If a Macro target is
unavailable, invalid, blocked by permissions, already running in a disallowed duplicate mode, or
unable to start, the Schedule records a failed occurrence without blocking the desk.

## Timezone and clock behavior

All Schedule dates, times, calendar expressions, previews, and next-occurrence calculations use the
local timezone configured on the server that runs the desk. In the normal single-computer desk
this is the desk's timezone. If a connected client has a different local timezone, the server
timezone remains authoritative.

Timezone is not stored or selectable per Schedule. Changing the server host's timezone changes
how future wall-clock occurrences are interpreted. The UI must display the currently authoritative
timezone wherever an operator needs it to understand a preview or next occurrence.

The scheduler must retain occurrence identity while it is running so a daylight-saving repeat,
backward clock correction, show reload, or schedule edit does not execute the same occurrence
twice. A local time skipped by a daylight-saving jump or forward clock correction is skipped; it
is not replayed later. A repeated local time runs at most once.

## Missed-run policy

There is one missed-run policy: **skip**.

A Schedule can run only while the server software is running, the Schedule is enabled, and its
owning show is active. If the software is off, the server is unavailable, the Schedule is
disabled, or another show is active when an occurrence is due, that occurrence is skipped. It is
never run later and there is no catch-up or backlog replay.

On startup or when the owning show becomes active, the scheduler advances directly to the first
future occurrence. It must not enumerate or execute an unbounded interval backlog. Status or
history may summarize that occurrences were skipped while inactive, but any represented missed
occurrence must be labeled **Skipped**, never **Completed** or **Failed**.

## Persistence and compatibility

Schedules are portable show objects with stable identities, names, enabled state, trigger
configuration, target action, and the interval/calendar anchor needed to calculate the next
occurrence. They do not persist a per-Schedule timezone or configurable missed-run policy.
Existing shows load with no Schedules and no behavior change.

Occurrence claims, last-result state, and a bounded history of the latest 100 occurrences are
stored separately from the editable Schedule object. Claiming an occurrence is committed before
dispatch. A crash after the claim may leave an occurrence marked **Interrupted**, but startup never
replays it. Completion or failure then updates the same claim. This metadata does not create a
show-object Undo step or an unbounded object-history stream.

Duplicating or importing a Schedule assigns a new stable Schedule identity, resets interval and
calendar anchors as applicable, and never copies occurrence history.

Schedule execution is active only while the owning show is active. Loading another show stops evaluating Schedules from the previous show. Partial Show Load must eventually be able to import selected Schedules, preview Playback and Macro dependencies, rewrite references where required, and block unresolved targets.

Deleting, moving, replacing, or incompatibly editing a referenced Playback leaves the Schedule
present and its enabled state unchanged but makes its runtime projection invalid. It cannot execute
again until the operator explicitly retargets or repairs it. Deleting a future Macro target follows
the same rule.

## Surface requirements

The Schedule feature must expose compatible behavior across:

- show-level schedule UI;
- command/API access for creating, updating, enabling, disabling, and inspecting Schedules;
- WebSocket events for Schedule state, next occurrence, and last result;
- future OSC or hardware surfaces where schedule operations are intentionally exposed; and
- manual/help documentation.

All surfaces must use the same trigger vocabulary: Interval, Calendar expression, One-time, enabled, disabled, next occurrence, missed occurrence, skipped, failed, and completed.

Schedule execution must be observable without being noisy during a show. Operators need enough status to trust what ran and why something did not run, but a normal successful occurrence should not interrupt live operation unless the target action itself does so.

## Failure behavior

A failed scheduled occurrence must not crash the server, block rendering, block output, or prevent
other Schedules from evaluating. Failures must be recorded with a timestamp, occurrence identity,
target action, and error reason. A failed occurrence is not retried or replayed automatically;
later recurring occurrences remain eligible to run normally.

## Acceptance coverage

1. Existing shows load with no Schedules and no changed playback behavior.
2. The operator can create an enabled Interval Schedule that starts a supported Playback every five minutes.
3. Interval Schedules use a documented anchor and do not drift or flood when an occurrence takes longer than expected.
4. The operator can create a Calendar-expression Schedule for every Monday at 14:00.
5. The operator can create a Calendar-expression Schedule for the first day of every month.
6. The operator can create a Calendar-expression Schedule for the first Monday of every month.
7. The operator can create a Calendar-expression Schedule equivalent to every second day.
8. Unsupported or ambiguous cron-style expressions are rejected before activation.
9. The operator can create a One-time Schedule for a specific date and time including seconds.
10. A One-time Schedule does not silently fire when saved with a past timestamp.
11. Every Schedule preview shows upcoming occurrences before save.
12. Every Schedule uses the server/desk timezone, exposes no per-Schedule timezone choice, and
    shows that authoritative timezone with previews and next occurrences.
13. Daylight-saving gaps, daylight-saving repeats, host timezone changes, and clock corrections
    never duplicate or replay an occurrence.
14. Occurrences missed while the software is off, the owning show is inactive, or the Schedule is
    disabled are skipped permanently; startup advances to the next future occurrence with no
    catch-up or backlog replay.
15. Loading a different show stops evaluating Schedules from the previous show.
16. A deleted, moved, or unsupported Playback target leaves the Schedule invalid rather than silently retargeted.
17. Scheduled Playback execution reaches the same authoritative Playback service used by UI, keyboard, OSC, hardware, HTTP, and Cue paths.
18. Until Macros exist, Macro targets are visibly unavailable and cannot activate; once item 32
    supplies the runtime, a Schedule starts a Macro through that same Macro service.
19. Failed scheduled occurrences are recorded without blocking output, rendering, other Schedules, or normal desk operation.
20. Command/API, WebSocket, UI, and future OSC or hardware surfaces use compatible Schedule vocabulary and state.

## Result

Implemented in `df74357d` (`feat(schedules): add authoritative show scheduling`).

- Added show-owned Schedule objects for anchored intervals, typed and advanced calendar rules, and
  one-time occurrences. Recurrence calculation uses the server timezone, skips gaps and missed
  runs, deduplicates repeated civil times, and advances interval backlogs arithmetically.
- Added separately persisted, bounded occurrence claims and results. Claims commit before
  dispatch, interrupted work is never replayed, history is capped at 100 entries, and duplicate or
  imported Schedules receive new identities and reset runtime history/anchors.
- Added authenticated preview, snapshot, create, update, duplicate, enable/disable, and delete
  routes with revision/idempotency checks, contextual validation, stable page/slot/Playback target
  verification, generated schemas, and typed runtime events.
- Scheduled actions use the authoritative Playback application service. Cuelist, Dynamic, and
  Group master fades are one engine-owned transition rather than repeated scheduler writes.
- Replaced the Storybook-only Scheduler mock with a production window/controller, authoritative
  server timezone and occurrence previews, stable Playback selection, event-driven reconciliation,
  a low-rate repair snapshot, persisted pane layout, actionable validation/results, and an explicit
  unavailable Macro boundary until item 32 supplies the Macro service.
- Added operator help at `docs/help/40-Running-a-Show/06-schedules.md`.

Verification completed:

- focused Schedule/domain/persistence/import/backend route and master-transition Rust tests passed;
- `cargo fmt --check`, the affected Rust package checks, frontend typecheck, focused Biome, and 49
  focused frontend tests passed;
- `npm run test:e2e-api`: 26 passed;
- `npm run test:e2e-ui`: 154 passed, 1 skipped, and 5 unrelated or contention-sensitive failures;
  the directly relevant 1,000-fixture acceptance was rerun alone and passed in 11.0 seconds with no
  output deadline misses;
- `npm run open` built and launched the actual Tauri application; `/api/v2/readiness` returned
  ready with an active show and no recovery error, the startup log reached an engine snapshot
  without errors, and the live Fixture Sheet/Stage desktop rendered with stable headers and column
  geometry.

Known repository-wide failures were not hidden or expanded into this item: the architecture gate
still reports pre-existing Grid Dynamics state-label debt, output/dynamic ownership debt,
Running-and-Output direct wire imports, and stale bench/docs inventories; one pre-existing Matter
lock-assumption unit test remains red. The broad four-worker UI run also exposed an OS
`setTypeOfService EINVAL`, two existing OSC-test issues, a marginal large-show 4× CPU p95 miss
(113.7 ms normalized versus 100 ms), and three output deadline misses caused by concurrent heavy
benches; the isolated 1,000-fixture contract passed.
