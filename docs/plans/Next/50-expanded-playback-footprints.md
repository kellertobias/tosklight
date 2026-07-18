# Expanded Playback Footprints

## Status

**Specification only.** This plan records a future playback-layout capability. It does not implement runtime behavior, persistence, UI, or executable tests.

## Goal

Allow a playback to use a larger footprint when the current desk or configured screen playback layout has compatible adjacent controls available. The normal one-slot playback remains the default.

An expanded playback may be either:

- **taller**, by claiming the compatible playback position directly above it; or
- **wider**, by claiming the compatible playback position beside it.

A playback cannot be both taller and wider in the initial implementation.

## Taller playbacks

Where two configured playback rows align, the lower playback may claim the topmost button from the playback position directly above it. That button becomes an additional button of the lower playback, allowing a four-button layout when the lower row already provides its normal button, fader, and lower buttons.

The claimed upper position is no longer an independently assignable or operable playback while it belongs to the expanded playback. The UI must visually communicate the shared footprint and route software, keyboard, OSC, and attached-hardware input to one authoritative page/playback identity.

## Wider playbacks

A playback may claim the compatible position beside it and span two playback columns. The combined footprint belongs to one authoritative playback rather than two independently addressable playbacks.

The wider layout may expose controls that do not fit a normal playback, including a second fader. One intended use is a dedicated X-fade fader alongside the normal playback master. Detailed button placement, supported dual-fader combinations, and which adjacent side may be claimed must be settled before implementation.

## Availability and conflicts

Expansion is offered only when the current desk or screen topology contains the required compatible neighboring position. Existing assignments, another expanded playback, row boundaries, screen boundaries, or incompatible hardware must prevent that position from being claimed.

The implementation must define deterministic behavior when changing screen layouts, reducing rows or columns, loading the show on a desk without the required topology, or moving between software-only and hardware-connected surfaces. It must never silently redirect the claimed controls to another playback.

## Persistence and compatibility

Planning must settle the ownership boundary between desk-local screen topology and show-persisted playback assignment. Existing shows and desk layouts load unchanged with every playback using its normal one-slot footprint.

Expanded-footprint state must preserve explicit page/playback identity and must not duplicate or merge the underlying Cuelist, Group Master, Speed Master, or Special assignment.

## Required acceptance coverage

1. Normal one-slot playbacks remain unchanged.
2. A compatible upper-row button becomes the lower playback's additional button.
3. A wider playback owns both columns and can expose a second fader.
4. Claimed positions cannot execute or be configured as independent playbacks.
5. Taller and wider modes are mutually exclusive.
6. Conflicting, occupied, out-of-range, and incompatible neighbors cannot be claimed.
7. Page changes retain correct explicit playback identity.
8. Layout changes and unsupported desks degrade safely without losing or redirecting the assignment.
9. Software, keyboard, OSC, and attached-hardware feedback agree on the expanded footprint.
10. Legacy show and desk data migrate without changing existing playback behavior.
