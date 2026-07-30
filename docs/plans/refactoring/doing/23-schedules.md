# Schedules

## Status

**Doing — refactoring queue item 23.** Implementation is now claimed. Runtime behavior,
persistence, production UI wiring, command/API behavior, events, documentation, and executable
tests remain required before this plan can move to `finished/`.

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

An Interval Schedule fires repeatedly after a configured duration, such as every five minutes. The interval is measured from a documented anchor:

- from the moment the Schedule becomes active;
- from the previous successful scheduled occurrence; or
- from a configured start date and time.

The implementation must choose one initial default and display it clearly. Intervals must be bounded by a minimum duration so a bad configuration cannot flood the desk with playback or macro starts.

### Calendar expression

A Calendar-expression Schedule fires according to a wall-clock recurrence rule. The operator-facing model may use a guided editor, a cron expression, or both, but it must support rules equivalent to:

- every Monday at 14:00;
- the first day of every month;
- the first Monday of every month;
- every second day; and
- other cron-style recurring dates and times that the selected expression format can represent.

The expression format must be documented before implementation. The desk must reject expressions whose behavior is unsupported or ambiguous, such as impossible dates, unsupported seconds fields, unsupported nth-weekday forms, or rules that behave differently across common cron dialects.

### One-time

A One-time Schedule fires once at a specific date and time, including seconds. After it runs successfully, the Schedule must either disable itself or remain as a completed historical record according to the chosen product behavior.

One-time Schedules in the past must not silently fire on save. The operator must explicitly choose whether a past fixed time is invalid, runs immediately once, or is saved disabled for recordkeeping.

## Scheduled actions

### Playback action

The reviewed UI exposes **Go**, **Pause**, **On**, **Off**, **Release**, and **Toggle**, plus an
optional Playback-master level and fade time. The implementation must route each supported action
through the same authoritative Playback service and reject combinations that do not apply to the
selected Playback.

The plan must settle whether the target is addressed by page and playback position, by a stable
Playback assignment identity, by Cuelist identity, or by another explicit show object. It must
also define what each supported action means for each Playback type:

- Cuelist Playback: GO, Load, Restart, or another explicit action;
- Group Master: set level, flash, or no supported scheduled action;
- Speed Master: set rate or no supported scheduled action;
- Special Playback: action-specific behavior; and
- empty or unsupported Playback slots: invalid Schedule target.

The Schedule must use the same authoritative Playback service as UI, keyboard, OSC, hardware controls, HTTP, and Cue-triggered playback actions. Scheduled execution must therefore produce normal playback state, feedback, audit, events, and error handling.

### Start Macro

Once Macros exist, a Schedule can start a Macro by stable Macro identity. Scheduled Macro execution must use the same Macro service as manual, Cue, Playback, Timecode, HTTP, OSC, or other supported Macro triggers.

The Schedule feature must not define Macro language, package, permission, Programmer, dialog,
duplicate-instance, or lifecycle behavior. Those semantics are owned exclusively by
[Macros](../pending/30-macros-and-scheduled-macros.md). This plan supplies the wall-clock occurrence,
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
configuration, target action, last-run metadata, and history summary. They do not persist a
per-Schedule timezone or configurable missed-run policy. Existing shows load with no Schedules and
no behavior change.

Schedule execution is active only while the owning show is active. Loading another show stops evaluating Schedules from the previous show. Partial Show Load must eventually be able to import selected Schedules, preview Playback and Macro dependencies, rewrite references where required, and block unresolved targets.

Deleting or moving a referenced Playback, Cuelist, or Macro must not leave a Schedule silently pointing at a different target. The Schedule becomes invalid, disabled, or requires explicit retargeting according to the chosen product behavior.

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
18. Once Macros exist, a Schedule can start a Macro through the same Macro service used by other Macro triggers.
19. Failed scheduled occurrences are recorded without blocking output, rendering, other Schedules, or normal desk operation.
20. Command/API, WebSocket, UI, and future OSC or hardware surfaces use compatible Schedule vocabulary and state.
