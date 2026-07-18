# Cuelist and Cue Settings Layout

## Completion

Implemented, then refined. In Cuelist View, the selected-Cue editor remains inline and thumbnail-led while Cuelist Settings opens as a modal. Closing dirty Cuelist settings requires an explicit Save changes, Discard changes, or Stay decision. Row selection updates the editor without executing a Cue. The Cuelist Pool hold shortcut opens the same modal.

## Status and scope

Keep selected-Cue editing compact in the sidebar and give Cuelist-owned settings a modal with enough width for explained operator controls.

## Layout contract

Opening **Cuelist Settings** presents a centered modal. Its title bar owns Mode, Renumber Cues, Save, and Close. The body groups Priority, Restart behavior, and Timing into three columns with descriptions. Chaser speed uses a typed multiplier and crossfade fader.

Selecting a Cue updates the inline selected-Cue area. The heading is omitted to save space; the Stage preview fills the sidebar width with a padded white-on-black selected-Cue label over it. Title, Fade, Delay, trigger, and trigger time use closely spaced frameless full-width rows. When those rows do not all fit below the preview, the preview remains visible and the non-scrolling sidebar switches to current-value targets with the instruction to press SET and then the attribute value. Physical and software SET share this path and open the normal keyboard, number pad, or trigger-choice modal. Selecting another Cue updates that same editor without executing either Cue.

The two levels must never mix ownership: playback configuration stays on the playback, Cuelist settings stay on the Cuelist, and Cue settings stay on the selected Cue. Dirty edits, revision conflicts, Save/Cancel behavior, and narrow layouts require explicit handling.

## Acceptance criteria

1. Cuelist Settings is a modal whose grouped controls remain reachable without horizontal clipping.
2. The selected-Cue editor remains inline and the Cue table stays visible and selection-only; constrained heights retain the preview and provide the SET/value modal path without scrolling.
3. No field appears at the wrong object level or is silently lost when switching levels.
4. Unsaved edits receive a deterministic Save/Discard/Stay flow before changing Cuelist or Cue context.
5. Keyboard, touch, and software/hardware-connected navigation reach every field without horizontal clipping.

## Verification

- CuelistWindow component coverage proves modal ownership, title-bar controls, persistent table visibility, compact selected-Cue rows, and dirty Stay/Discard behavior without a save mutation.
- `CUELIST-LAYOUT-001` measures the production thumbnail, overlay badge, frameless row spacing, constrained-height SET/value fallback, modal columns, and clipping, proves the Cue table remains visible, exercises dirty Stay/Discard, and selects another Cue without execution.
- The existing Cuelist/Cue acceptance suite, focused component tests, production build, full unit coverage, and generated manual pass.
