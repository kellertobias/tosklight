# Chaser Crossfade Percentage

## Status and scope

Express **Chaser X-fade** as a percentage of the effective Chaser step duration rather than as milliseconds or seconds.

## Data and runtime semantics

The operator enters a value from `0%` to `100%`. `0%` snaps between steps; `100%` uses the full effective step interval for the crossfade. The runtime resolves the actual duration from the selected Speed Group, current BPM, and speed multiplier each time the cadence changes. A percentage therefore follows live speed changes without rewriting the stored setting.

Store the percentage in a stable normalized or integer-percent field on the Cuelist. Migrate legacy `chaser_xfade_millis` values deterministically using the legacy effective step duration available at migration time, clamp impossible values visibly, and preserve the old value for recovery/audit where the schema process requires it. Newly saved shows must not continue treating the value as absolute milliseconds.

Disable Cue Timing still makes the effective Chaser crossfade zero without changing the stored percentage. Chaser cadence remains authoritative, and percentage changes must not cause overlapping or skipped step triggers.

## Acceptance criteria

1. Cuelist Settings labels and validates the field as percent with a `0–100%` range.
2. Changing BPM or multiplier changes effective crossfade duration while the stored percentage remains constant.
3. `0%`, `50%`, and `100%` produce exact snap, half-step, and full-step transitions.
4. Legacy shows migrate predictably and survive Save/Reload without rounding drift.
5. Disable/Force Cue Timing, pause/resume, GO, and restart retain documented cadence and transition behavior.
