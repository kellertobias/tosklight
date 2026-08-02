# Expanded Playback Footprints

## Status

**Specification only.** This plan records a future playback-layout capability. It does not implement runtime behavior, persistence, UI, or executable tests.

This is the seventh item in the current [Next plan order](README.md).

## Goal

Allow a playback to use a larger footprint when the current desk or configured screen playback layout has compatible adjacent controls available. The normal one-slot playback remains the default.

An expanded playback may be either:

- **taller**, by claiming the compatible playback position directly above it; or
- **wider**, by claiming the compatible playback position directly to its right.

A playback cannot be both taller and wider in the initial implementation.

## Taller playbacks

Where two configured playback rows align, the lower playback may claim the topmost button from the playback position directly above it. That button becomes an additional button of the lower playback, allowing a four-button layout when the lower row already provides its normal button, fader, and lower buttons.

The claimed upper position is no longer an independently assignable or operable playback while it belongs to the expanded playback. The UI must visually communicate the shared footprint and route software, keyboard, OSC, and attached-hardware input to one authoritative page/playback identity.

## Wider playbacks

A playback may claim the compatible position directly to its right and span two playback columns.
The combined footprint belongs to one authoritative playback rather than two independently
addressable playbacks.

The wider layout mirrors the normal playback control stack into the claimed right-hand column. This
is the initial fixed slider layout: the authoritative bottom-left playback keeps its existing
column, and the added column is placed to its right. It may therefore expose a second fader and
additional buttons, but none has an implied role such as Master or X-fade.

Every button, fader, encoder, or other control in the combined footprint is independently
assignable through the same typed playback-control function vocabulary used by ordinary playbacks.
The claimed right-hand position is never a separate playback, and expansion never occurs to the
left.

## Availability and conflicts

Expansion is offered only when the current desk or screen topology contains the required compatible neighboring position. Existing assignments, another expanded playback, row boundaries, screen boundaries, or incompatible hardware must prevent that position from being claimed.

The bottom-left playback position is the authoritative anchor for both taller and wider layouts.
When a screen layout shrinks, a desk lacks the required topology, or an expanded footprint otherwise
does not fit, only the anchor's ordinary one-slot controls are available. Extra controls do not
move, remap, or replace another playback. Their stored assignments remain preserved so they return
if a compatible layout is restored.

## Persistence and compatibility

The desk-local screen topology determines only whether the required positions physically exist.
The expanded-footprint choice and every per-control function assignment are show-persisted.
Existing shows and desk layouts load unchanged with every playback using its normal one-slot
footprint.

Expanded-footprint state must preserve explicit page/playback identity and must not duplicate or merge the underlying Cuelist, Group Master, Speed Master, or Special assignment.

## Required acceptance coverage

1. Normal one-slot playbacks remain unchanged.
2. A compatible upper-row button becomes the lower playback's additional button.
3. A wider playback expands only to the right, owns both columns, and can expose a second fader.
4. Claimed positions cannot execute or be configured as independent playbacks.
5. Taller and wider modes are mutually exclusive.
6. Conflicting, occupied, out-of-range, and incompatible neighbors cannot be claimed.
7. Page changes retain correct explicit playback identity.
8. Layout changes and unsupported desks use only the bottom-left anchor's ordinary controls without
   losing, moving, or redirecting stored extra-control assignments; restoring a compatible layout
   restores them.
9. Software, keyboard, OSC, and attached-hardware feedback agree on the expanded footprint.
10. Legacy show and desk data migrate without changing existing playback behavior.
11. Every control in an expanded footprint is independently assignable and round-trips through the
    show file.
