# Cuelist and Cue Settings Layout

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
