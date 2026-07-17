# Cuelist and Cue Settings Layout

## Completion

Implemented. In Cuelist View, Cuelist Settings now replaces the complete right-hand sidebar and uses its full inner width while the Cue table remains visible. Closing dirty Cuelist settings requires an explicit Save changes, Discard changes, or Stay decision. Cue Settings is named and remains inline for the selected Cue; row selection updates it without executing a Cue. The Cuelist Pool hold shortcut retains the same settings as an overlay because that surface has no Cue sidebar.

## Status and scope

Correct the Cuelist View settings layout: Cuelist Settings uses the full width of the sidebar, while Cue Settings remains inline with the selected Cue.

## Layout contract

Opening **Cuelist Settings** replaces the sidebar content with a full-width Cuelist-level editor. It must not be squeezed into the narrower selected-Cue form or leave an unused nested column. Cuelist mode, tracking, wrap, Chaser, timing, priority, renumber, and other Cuelist-owned fields use the complete available sidebar width.

Selecting a Cue shows **Cue Settings** inline in the normal selected-Cue area. Editing title, Fade, Delay, trigger, trigger time, and other Cue-owned fields must not open a separate overlay or replace the Cuelist table. Selecting another Cue updates that same inline editor without executing either Cue.

The two levels must never mix ownership: playback configuration stays on the playback, Cuelist settings stay on the Cuelist, and Cue settings stay on the selected Cue. Dirty edits, revision conflicts, Save/Cancel behavior, and narrow layouts require explicit handling.

## Acceptance criteria

1. Cuelist Settings occupies the sidebar's full inner width at every supported window size.
2. Cue Settings remains inline and the Cue table stays visible and selection-only.
3. No field appears at the wrong object level or is silently lost when switching levels.
4. Unsaved edits receive a deterministic Save/Discard/Stay flow before changing Cuelist or Cue context.
5. Keyboard, touch, and software/hardware-connected navigation reach every field without horizontal clipping.

## Verification

- CuelistWindow component coverage proves sidebar replacement, persistent table visibility, explicit Cue Settings identity, and dirty Stay/Discard behavior without a save mutation.
- `CUELIST-LAYOUT-001` measures the production sidebar and settings bounds at 1280×720, checks every visible field for clipping, proves the Cue table remains visible, exercises dirty Stay/Discard, and selects another Cue without execution.
- The existing Cuelist/Cue acceptance suite, focused component tests, production build, full unit coverage, and generated manual pass.
