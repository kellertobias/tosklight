# External Screen Fixed Full-Screen Pane

## Status

**Doing — refactoring queue item 25.** Implementation is claimed. Screen configuration, native
window rendering, view-only pane behavior, persistence, help, and executable acceptance coverage
must be completed before this plan moves to `finished/`.

## Goal

Allow an optional external screen to show one fixed, view-only pane across its complete pane workspace. This is intended for a desk running on a touch-only computer with a second display that has no touch input, such as a monitor dedicated to the Fixture Sheet, Stage, a Cue list, or show text.

The fixed pane is configured from **Show > Desk Setup > Screens & playback**. The operator must not need to interact with the external screen itself to choose its content or change the pane's settings.

This content mode is distinct from the existing native-window fullscreen setting. Native fullscreen controls how the optional screen window occupies its physical display; **Fixed full-screen pane** controls what ToskLight renders in that screen's pane workspace.

## Screen configuration

Each optional screen gains a content choice between its normal configurable Desktop and **Fixed full-screen pane**. Selecting the fixed-pane mode exposes:

- **Pane**: an allowlisted view-only pane type;
- the selected pane's applicable display settings; and
- any required fixed object, such as the Cuelist or text to display.

The pane occupies all space normally available to Desktop panes. It has no pane header, grid resize controls, drag behavior, remove action, Desktop switching, or on-screen Pane Settings entry point. Its configuration remains available from the controlling touch screen in Desk Setup.

The optional screen may still show **Playbacks** and **Page Controls**. These settings remain independently configurable because their state is useful even when the main content is a fixed display. They reduce the space available to the fixed pane in the same way they do on a normal optional screen.

## Allowed fixed panes

Only content that has a useful view-only presentation may be selected:

- **Fixture Sheet**, with its displayed heads, ordering, filters, columns, name details, and Group-shortcut visibility configured in Desk Setup;
- **Stage - 2D**, with the relevant 2D display settings configured in Desk Setup;
- **Stage - 3D**, with the relevant 3D display settings configured in Desk Setup;
- **Cues - Cuelist**, fixed to one explicitly selected Cuelist and showing its Cue list and live current/next state; and
- **Text**, using the Text Editor's view-only presentation with one explicitly selected stored text file and an applicable Plain Text or Rendered Markdown display mode.

These variants are display surfaces on the external screen. Fixture selection, Cue selection or editing, Stage selection and marquee gestures, text editing, file picking, save actions, and other pane mutations are unavailable there. Live data and running-state feedback continue to update.

The selector must not offer interactive or unsuitable panes. This explicitly excludes:

- DMX output;
- Preset Pool;
- Group Pool;
- Help;
- Cuelist Pool and the legacy Cuelists navigation pane;
- Virtual Playbacks;
- File Manager; and
- other setup, editing, pool, diagnostic, or control panes not included in the allowlist above.

Adding another pane later requires an explicit view-only external-screen contract; being available in the normal window manager is not sufficient.

## Dock constraint

**Show Dock** is incompatible with **Fixed full-screen pane**.

When the operator selects a fixed full-screen pane, ToskLight must immediately turn **Show Dock** off for that optional screen and disable the Show Dock control while this mode remains selected. The saved screen configuration must not contain an active Dock together with a fixed full-screen pane, including after migration, API updates, or loading malformed desk data.

Leaving fixed-pane mode makes **Show Dock** configurable again. It remains off until the operator deliberately enables it; leaving the mode must not silently restore the Dock.

## Ownership and persistence

The selected mode, pane type, fixed object, and pane-specific display settings belong to the optional screen's desk-local configuration, not the portable show file. References to show objects must use stable identities and recover safely when the selected Cuelist or text file is missing, renamed, moved, or unavailable.

The external screen must show a clear non-interactive empty or unavailable state rather than substituting another Cuelist, another text, or an unsuitable pane. Configuration stays editable from Desk Setup on the controlling screen even when the physical external display is disconnected.

## Acceptance coverage

1. An optional screen can be changed from its normal Desktop to a fixed full-screen pane entirely through **Screens & playback** on the controlling screen.
2. The fixed pane fills the complete pane workspace without pane chrome, layout editing, or an on-screen settings entry point.
3. Fixture Sheet, Stage - 2D, Stage - 3D, one fixed Cuelist's Cues, and one selected text in view-only mode are the only selectable fixed panes.
4. Each allowed pane exposes its relevant display or object settings inside Desk Setup and restores them with that optional screen.
5. The external pane accepts no selection, editing, file, save, navigation, or mutation interaction while continuing to show live output and Cue state.
6. DMX output, Preset Pool, Group Pool, Help, Cuelist Pool, Cuelists navigation, Virtual Playbacks, File Manager, and every other non-allowlisted pane cannot be selected.
7. Selecting a fixed full-screen pane immediately turns **Show Dock** off, prevents it from being enabled in that mode, and persists a valid dock-free configuration.
8. **Show Playbacks** and **Show Page Controls** remain independently configurable in fixed-pane mode and reserve their normal screen space when shown.
9. Leaving fixed-pane mode makes **Show Dock** available again without silently enabling it.
10. Missing or unavailable fixed content produces a clear empty state and never silently changes the configured content.
11. The mode remains distinct from native-window fullscreen and works whether that native screen window is fullscreen or windowed.
12. Browser-only operation does not claim support for optional native external-screen windows.
