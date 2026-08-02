# Fixture Sheet Filtering and Compact Mode

Use a fresh copy of a representative show for each scenario. Keep Show Patch, Stage, Fixture Sheet,
and DMX output available, and record the active show identity plus desk-layout state before every
mutation. Fixture Sheet values below mean ordinary pre-Dynamic/FAT bases, not sampled output.

## FIXTURE-SHEET-002-001 — Scenery never becomes a programmable row

1. Add four independently identifiable objects: a Venue fixture, a non-Venue `visual_only` profile,
   legacy scenery whose complete ID begins `0.`, and an ordinary programmable fixture.
2. Include an ordinary multi-head fixture and expose All, No sub heads, and No master heads in turn.
3. Exercise active-only and Cuelist filtering, select each hidden object from Show Patch and Stage,
   save/reopen the show, and export/re-import it through the normal MVR path.

Venue, `visual_only`, and complete-`0.` objects remain in Show Patch, Stage, selection, persistence,
and MVR but never appear in the full or pane Fixture Sheet. The ordinary fixture and applicable
master `.0`/head `.1` rows remain. A hidden-only selection produces no selected substitute row.

## FIXTURE-SHEET-002-002 — Every authoritative group reports a stable base

Program one populated fixture with Intensity, Color, Position, Beam, Shapers, Focus, Control, and
Media attributes. Include semantic indexed functions, Media Folder/File plus Mask Folder/File,
unsupported groups, Programmer and Playback ownership, Preload, two simultaneous Dynamics on one
member, and an embedded Dynamic snapshot without a pool number.

Every enabled group column shows semantic ordinary bases and applicable per-member Dynamic identities.
Media and Mask pairs stay distinct; zero differs from unavailable. Preload reports pending bases and
identities. No cell invents a Beam/Focus placeholder or exposes raw DMX.

## FIXTURE-SHEET-002-003 — Dynamic output changes without repainting the base

Set one patched dimmer's ordinary Programmer base to 50%, start a numbered looping Dynamic on its
Intensity, and open Fixture Sheet beside DMX output. Advance the deterministic test clock through
at least three distinct Dynamic phases.

DMX output changes between phases. Fixture Sheet remains at the same 50% base and retains the same
Dynamic icon/number with no sampled value field. Stopping the Dynamic removes its identity without
changing the ordinary base.

## FIXTURE-SHEET-002-004 — Compact modes preserve information at small size

At a supported 430 px Fixture Sheet width, select all eight value groups and populate at least 24
rows including `.0`/`.1` IDs, Preload, multiple Dynamics, source ownership, Group-master limiting
and Flash, active Highlight bypass, step/base selection, and an unavailable value.

Compare Off, Icon only, and Text only. Off uses equal 43 px detailed rows. Both compact modes use
equal 32 px rows and show materially more complete rows. Icon only keeps graphical bases and removes
ordinary value text; Text only does the reverse. All modes retain Dynamic identities and status.
Configured columns remain present behind real horizontal scrolling, and no row, ID, marker, or
current/Preload distinction overlaps or clips.

## FIXTURE-SHEET-002-005 — Mode ownership is per surface and desk-local

Set one normal pane to Icon only, another to Text only, the full built-in to Off, and a fixed external
Fixture Sheet to Icon only. Restart the desk and reopen the portable show on a second desk.

Each original surface restores its own mode. Changing or removing one surface does not alter another,
the selection, Programmer/Cue data, Fixture Sheet filters, or the portable show. Old saved `dimmer`
visibility becomes Intensity; old and new surfaces default to Off and gain no newly visible value
groups without an explicit Columns choice.
