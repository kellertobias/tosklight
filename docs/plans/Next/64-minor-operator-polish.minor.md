# Minor Operator Polish

## Status

**Specification only.** This minor plan collects small operator-facing corrections. It does not implement UI changes, runtime behavior, assets, tests, or documentation screenshots.

## Items

### Command-line fixture history

Fixture history should fold up from the command line instead of appearing as an unrelated modal. The command line remains part of the history surface, so the operator sees the current entry in context while browsing previous entries.

### Special dialogs

Improve special dialogs so they read as professional production UI. The Position special dialog is the priority: it may look visually interesting today, but it needs a more polished control layout, spacing, labeling, and interaction model.

### Relative encoders and programmer fade time

This is not a minor polish item. It belongs in [Programmer Relative Encoders and Fade-Time Scope](65-programmer-relative-encoders-and-fade-time-scope.md).

### Remove direct programmer encoder type

The direct programmer encoder type should be removed again. Fixed actions such as Lamp On, Lamp Off, Reset, and similar preset generation should be handled through special dialogs or through the DMX/timecode/master-macro surface, not as a direct encoder type.

### DMX, timecode, and master macro surface

The surface opened from DMX/timecode/master controls should show Grand Master and Blackout vertically on the left. The right side should show macros such as Lamp On, Lamp Off, Reset, and similar actions. When fixtures are selected, macros should act on the selection and label themselves accordingly, such as Selected Lamps On instead of All Lamps On.

The middle area should show running sources in tabs such as Running Playbacks, Running Dynamics, and Active Programmers. Each tab should show a count. The list should include a clear Stop Everything action, and clicking an individual running item should clearly turn it off.

### Application icon symbol

Change the in-app icon to use only the logo symbol from the application icon, not the full icon artwork. The SVG must be supplied before implementation; if this task is requested without the SVG, the implementer must ask for the SVG first.

### Clock seconds

Make the seconds in the clock slightly larger while preserving the existing clock layout and avoiding overlap in hardware-connected and software-only modes.

## Acceptance coverage

1. Fixture history visually grows from the command line and keeps the current command in context.
2. Position and other special dialogs use a polished, professional layout.
3. Relative encoders and Programmer Fade time are tracked by their own non-minor plan.
4. The direct programmer encoder type is absent from operator-facing encoder choices.
5. DMX/timecode/master controls expose Grand Master, Blackout, macros, and running-source stop controls with clear labels.
6. The app icon symbol update is blocked until the required SVG is provided.
7. Clock seconds are larger without breaking layout.
