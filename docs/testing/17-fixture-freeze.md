# Fixture Freeze

## Purpose

Prove that Freeze retains resolved fixture output without rewriting its underlying Programmer,
Playback, Dynamic, direct-control, or master state, and that every operator surface presents the
same persisted result.

## Full Freeze

1. Patch an intensity fixture and a multi-head color/position fixture, place them in a Group, and
   select the Group through the ordinary Group-selection path.
2. Establish visible Programmer, Cue, Dynamic, and direct-control contributions, with Group Master
   and Grand Master below Full and Blackout off.
3. Press **SHIFT + CLEAR**. Confirm the Fixture Sheet shows `❄ FREEZE` on each resolved fixture or
   head, with `INSIDE` on a master-only row where applicable.
4. Change every contributing source, move Group Master and Grand Master, and enable Blackout.
   Confirm the frozen physical and visualization output remains exactly at the captured frame.
5. Press **SHIFT + CLEAR** again. Confirm the marker disappears and the current underlying state is
   visible immediately; no captured value has been written into the Programmer or Cue.

## Partial Freeze

1. Through the authoritative Freeze action, apply Intensity and Color to a mixed fixture selection.
2. Confirm the Fixture Sheet names both families and does not show the full `FREEZE` state.
3. Change Intensity, Color, Position, and Beam sources. Confirm only Intensity and Color retain their
   captured semantic values.
4. Move Group Master and Grand Master and enable Blackout. Confirm partial Freeze output follows all
   three masters.
5. Repeat the same family action. Confirm those families and their retained values are removed.
6. Apply a full Freeze over an existing partial Freeze, then remove it. Confirm no partial-family
   metadata is restored.

## Persistence and parity

1. Save, close, and reopen the show with full and partial fixtures. Confirm retained values, family
   names, Fixture Sheet status, and output are unchanged.
2. Make an unrelated Show Patch edit and repeat the reload check.
3. Exercise **SHIFT + CLEAR** from touch/software keyboard, OSC, and attached hardware wherever that
   surface exposes the chord. Confirm every path reaches the same server-owned live action and its
   ordered portable-show transaction.
4. Confirm Programmer Undo remains desk-local and does not silently undo Freeze. If a dedicated
   Freeze history action is introduced, verify its undo/redo behavior separately; there is no
   generic show-wide Undo surface.
5. Load an older show with no Freeze fields. Confirm it opens with no frozen fixtures and can be saved
   without recovery warnings.
