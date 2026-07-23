# Virtual Playbacks

## Status

**Specification only.** This plan records future playback-runtime and UI behavior. It does not implement playback storage, runtime state, UI changes, API behavior, OSC behavior, or executable tests.

## Goal

Define virtual playbacks as real playback runtime objects that behave like ordinary playbacks without taking over or shifting the physical desk's playback positions.

Virtual playbacks are not independent from the desk state: they still participate in playback arbitration, release, cue timing, output ownership, and feedback. They should, however, have their own addressing model so they do not overlap with the playback buttons and faders shown on actual desks.

## Decided (maintainer, 2026-07-23)

- **Numbering:** virtual playbacks live in the **same playback space** as physical
  playbacks but use their own independent number range **starting at 1001**. Physical
  buttons keep the low numbers with plenty of headroom; virtual playbacks are always
  recognizable by their number. Virtual numbering is completely independent of any
  physical playback assignment.
- **Page binding:** a virtual-playback window/pane is configurable to either **follow
  the main page of the desk it is on** or to be **pinned to its own fixed page**.
- **Fixed layout:** the virtual-playback pane's grid is configured explicitly (e.g.
  10×10) and **stays exactly that size regardless of the window size** — resizing the
  window never adds or removes playback cells; it scales/scrolls the fixed grid.

## Required behavior

- decide which virtual playbacks are shown in the UI and why;
- keep virtual playback runtime behavior compatible with regular playbacks, including GO, stop, release, fade, restart, and state feedback;
- make current-page physical playback addressing distinct from virtual playback addressing;
- ensure Preload Go can start or stage virtual playbacks without creating sources that cannot be turned off; and
- expose virtual playback state consistently in software UI, command/API, OSC, and attached hardware feedback where applicable.

## Acceptance coverage

1. Virtual playback identity never changes the visible physical playback position on the current desk page.
2. Starting, releasing, and stopping a virtual playback follows the same runtime semantics as a regular playback.
3. Virtual playback page and number labels are unambiguous to the operator.
4. A page-pinned virtual-playback pane is never retargeted by physical page changes; a
   follow-main-page pane follows them deliberately and visibly per its configuration.
5. Active virtual playbacks appear in running-source feedback and can be stopped deliberately.
6. A pane configured as an N×M grid renders exactly N×M cells at every window size.
