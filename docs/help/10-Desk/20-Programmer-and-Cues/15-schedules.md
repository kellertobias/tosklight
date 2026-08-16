# Schedules

Schedules run a stored Playback action from the show’s wall clock. Open **Scheduler** from the
window picker to inspect, create, edit, enable, disable, duplicate, or delete them.

Every Schedule uses the timezone shown by the server. A connected browser’s timezone does not
change the preview or execution time, and there is no per-Schedule timezone setting.

## Trigger types

- **Interval** repeats from the instant the Schedule was enabled. The minimum interval is one
  minute. Occurrences remain exact multiples of that anchor, so a slow action does not move later
  occurrences.
- **Calendar expression** uses the five fields `minute hour day-of-month month weekday`. Fields
  accept `*`, numbers, lists, ranges, and positive steps. Weekdays are `0` for Sunday through `6`
  for Saturday; `#1` through `#4` and `L` select ordinal weekdays. A rule cannot constrain both
  day-of-month and weekday.
- **One-time** accepts a server-local date and time including seconds. A successful One-time
  Schedule disables itself and remains visible with its result.

The editor shows the server-calculated upcoming occurrences before saving. A daylight-saving gap
is skipped. A repeated local time runs at most once.

## Playback targets

A Schedule stores the selected page, slot, and stable Playback number. If that Playback is moved,
deleted, replaced in the slot, or changed to an unsupported type, the Schedule becomes invalid
and does not follow the new occupant.

Cuelist and Dynamic Playbacks support **Go**, **Pause**, **On**, **Off**, **Release**, and
**Toggle**. Group Playbacks support **On**, **Off**, **Release**, and **Toggle**. Other Playback
types are not Schedule targets. The optional master setting accepts 0–100% and a fade of 0–60
seconds.

Macro targets are reserved for a later release and are currently shown as unavailable.

## Missed and failed occurrences

Schedules run only while their show is active and the server is running. Missed occurrences are
**Skipped** permanently; they are never caught up or retried. A restart, show change, forward clock
correction, or long inactive period advances to the next future occurrence without replaying a
backlog.

The list shows the next occurrence and the latest **Completed**, **Skipped**, **Failed**, or
**Interrupted** result. A failure records its reason without stopping output, rendering, the desk
interface, or other Schedules.
