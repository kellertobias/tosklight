# Schedules

## Status

**Specification only.** This plan records a future scheduling capability. It does not implement runtime behavior, persistence, UI, command/API behavior, OSC behavior, or executable tests.

## Goal

Allow an operator to define show-owned Schedules that automatically start an action at a configured time. The first scheduled action type is starting a Playback. Once Macros exist, a Schedule must also be able to start a Macro by stable identity.

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
- the target action, initially Start Playback and later Start Macro;
- the referenced Playback or Macro;
- the timezone used to interpret wall-clock rules;
- the behavior when the desk or owning show was inactive during an occurrence; and
- whether the Schedule remains enabled after a successful one-time occurrence.

The editor must preview upcoming occurrences before the operator saves the Schedule. Invalid expressions, nonexistent dates, missing target actions, deleted Playbacks, unavailable Macro runtime, or ambiguous timezone settings must block activation with an actionable message.

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

### Start Playback

The initial scheduled action starts a Playback. The plan must settle whether the target is addressed by page and playback position, by a stable Playback assignment identity, by Cuelist identity, or by another explicit show object. It must also define what "start" means for each Playback type:

- Cuelist Playback: GO, Load, Restart, or another explicit action;
- Group Master: set level, flash, or no supported scheduled action;
- Speed Master: set rate or no supported scheduled action;
- Special Playback: action-specific behavior; and
- empty or unsupported Playback slots: invalid Schedule target.

The Schedule must use the same authoritative Playback service as UI, keyboard, OSC, hardware controls, HTTP, and Cue-triggered playback actions. Scheduled execution must therefore produce normal playback state, feedback, audit, events, and error handling.

### Start Macro

Once Macros exist, a Schedule can start a Macro by stable Macro identity. Scheduled Macro execution must use the same Macro service as manual, Cue, Playback, Timecode, HTTP, OSC, or other supported Macro triggers.

The Schedule feature must not select or implement the Macro language. If a Macro target is unavailable, invalid, blocked by permissions, already running in a disallowed duplicate mode, or unable to start, the Schedule records a failed occurrence without blocking the desk.

## Timezone and clock behavior

Each Schedule must store the timezone used to interpret wall-clock dates and recurrence rules. The desk must not depend only on the computer's current local timezone for persisted show behavior.

The scheduler must store enough occurrence identity to prevent duplicate execution across restarts, daylight-saving transitions, clock corrections, and show reloads. It must explicitly define behavior for:

- daylight-saving gaps where a local time does not exist;
- daylight-saving repeats where a local time occurs twice;
- system clock changes while the desk is running;
- timezone changes on the host system;
- editing a Schedule after occurrences have already run; and
- loading a show on a desk in a different timezone.

## Missed-run policy

Each Schedule must declare what happens when an occurrence was missed because the desk was off, the server was not running, the owning show was inactive, the target action was invalid, or the Schedule was disabled.

The supported policies should include at least:

- **Skip missed occurrences**, leaving them recorded as skipped; and
- **Run the most recent missed occurrence once**, subject to a catch-up limit.

Catch-up must have a bounded lookback window and must never replay an unbounded backlog of interval or calendar occurrences. A skipped or failed missed occurrence must be visible in Schedule history.

## Persistence and compatibility

Schedules are portable show objects with stable identities, names, enabled state, trigger configuration, target action, timezone, missed-run policy, last-run metadata, and history summary. Existing shows load with no Schedules and no behavior change.

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

A failed scheduled occurrence must not crash the server, block rendering, block output, or prevent other Schedules from evaluating. Failures must be recorded with a timestamp, occurrence identity, target action, error reason, and whether retry or catch-up remains possible.

The implementation must define retry behavior before launch. If automatic retry is supported, it must be bounded and visible. If retry is not supported, the Schedule history must make that explicit.

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
12. A Schedule stores and uses its configured timezone rather than only the host timezone.
13. Daylight-saving gaps, daylight-saving repeats, host timezone changes, and clock corrections do not duplicate an occurrence.
14. Missed-run policies can skip missed occurrences or run only the most recent missed occurrence within a bounded catch-up window.
15. Loading a different show stops evaluating Schedules from the previous show.
16. A deleted, moved, or unsupported Playback target leaves the Schedule invalid rather than silently retargeted.
17. Scheduled Playback execution reaches the same authoritative Playback service used by UI, keyboard, OSC, hardware, HTTP, and Cue paths.
18. Once Macros exist, a Schedule can start a Macro through the same Macro service used by other Macro triggers.
19. Failed scheduled occurrences are recorded without blocking output, rendering, other Schedules, or normal desk operation.
20. Command/API, WebSocket, UI, and future OSC or hardware surfaces use compatible Schedule vocabulary and state.
