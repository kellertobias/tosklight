# Quiet No-Op Feedback

## Purpose

Expected no-ops leave the desk usable without drawing attention. Invalid commands and genuine
connection, persistence or desk failures retain their actionable error treatment.

## NOTICE-001 — Align with nothing selected

Given an open show with no fixtures selected, pressing **Align Off** beside the encoders leaves
Align **Off**, changes no Programmer value and adds no Undo entry. No notice, toast, error,
modal, sound or live announcement appears. Focus stays on the control. Repeated presses behave
the same way. Attached-hardware and keyboard Align gestures use the same server-owned action.
With fixtures selected, pressing it activates **Align Left**. A genuine connection failure still
opens the existing **Desk needs attention** message.

The active mode is shared by all control surfaces. A change from another surface updates the
visible button, and reconnecting restores that same mode. The next press advances from the
server's current mode; Shift-click switches it Off without a notice.

## NOTICE-002 — Empty programming actions

Syntactically valid AT, FixAT, Release and Aim actions with no applicable selected targets succeed
without programming, activation or Undo changes. Encoders, the picker and universal preset recall
behave the same way. Normal, Blind and Preload preserve their existing values. Invalid syntax,
nonexistent explicit preset or aim targets and actual persistence failures remain errors.
Fixture-specific presets retain their documented first-touch selection behavior.

## NOTICE-003 — Expected Color limitations

An unsupported UV request or approximate Color fit is retained. Recording, recall, playback and
Dynamics continue with the requested colour.

- In the Fixture Sheet, a fixture whose output cannot show the request reads its Color value as
  usual with one small, steady, subdued triangle beside it. The triangle comes from the output frame
  that was sent: one report request covers all rows on screen, never one request per row, and an
  unprogrammed fixture never gets one.
- The triangle has an accessible label such as **Color details: UV unavailable on this fixture** and
  a touch-sized hit area. Tapping it, or Enter or Space on it, opens the full **Color** window on that
  fixture's per-fixture details and does not change the selection.
- Nothing announces itself: no toast, banner, sound, alert or live region appears, focus does not
  move and no window opens on its own, also while the output keeps changing.
- The full Color window lists the visible match and UV in separate columns. The compact dialog adds
  no status line.
