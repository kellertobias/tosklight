# Cuelist View and Cuelist Settings Implementation Contract

This document contains implementation guidance for the Cuelist View and its persisted settings. Behavioral acceptance scenarios CUE-011 and CUE-012 live in [`tests/06-cuelist-view-and-settings.spec.ts`](../../../tests/06-cuelist-view-and-settings.spec.ts). Implement the capabilities here before adding or enabling their Playwright coverage.

## Product terminology and surfaces

Use these names consistently:

- **Cuelist Pool** is the numbered pool used to create, find, open, and assign Cuelists.
- **Cuelist View** is the view for one opened Cuelist. It contains the Cue table, selected-Cue editor, and access to Cuelist Settings.
- **Playback controls** are the actual assigned faders/buttons or Virtual Playbacks. They execute GO, GO minus, Toggle, Off, Flash, and related actions. They do not belong in the Cuelist View.

Opening a Cuelist Pool tile navigates to its Cuelist View. Returning to the pool uses the existing title-bar navigation. The Cuelist View is an editor/inspection surface, not another playback surface.

## Cuelist View layout

The table occupies the main/left area and has exactly these columns in this order:

1. Preview image
2. Cue number
3. Cue name
4. Trigger
5. Fade time

Remove the current **Status** column. Current and next Cues may retain non-column row styling, but the table must not present `Active`/`Tracked` as Cue data.

Tapping or clicking a row changes only the selected Cue in the editor. It must not execute, jump to, activate, preview-output, or otherwise change playback state. Keyboard Enter/Space on a focused row follows the same selection-only rule.

The top of the right side contains the selected-Cue editor with:

- **Title** text input, persisted as `Cue.name`.
- **Fade** non-negative number input in seconds, persisted as `Cue.fade_millis`.
- **Delay** non-negative number input in seconds, persisted as `Cue.delay_millis`.
- **Trigger** selector with **GO**, **FOLLOW**, and **TIME**.
- A non-negative trigger-time number input shown only for **TIME**.

Cue Delay is the Cue-level start-delay fallback for values without an explicit per-value delay. It is separate from trigger timing: the Cue must first be triggered according to GO/FOLLOW/TIME, then its Cue/value delay and fade processing begins. The command-line `DELAY` clause on a Cue-record command retains its separately documented trigger meaning.

Editing a Cue uses a revision-checked Cuelist update. Commit on Enter or field blur and show a visible error on validation or revision conflict. A successful edit updates the row immediately but never executes the Cue. Store milliseconds without rounding drift even though the UI displays seconds.

Remove GO, GO minus, Toggle, and Off from the Cuelist View's right side. Those actions remain available on assigned playback controls. A selected Cue must not gain an implicit “go to selected Cue” gesture merely because the row editor exists; any future direct-jump action must be separate and explicit.

## Cuelist Settings access

Add a **Cuelist Settings** action to the Cuelist View title bar. It opens the settings for the currently viewed Cuelist. Pool long-press may remain as an additional shortcut, but it must open the same settings model and must not be the only discoverable path.

Playback-definition settings such as button assignments, fader mode, automatic-off, and GO activation belong to an assigned playback's configuration, not to Cuelist Settings. Their separate implementation contract is [`docs/plans/Done/10-playback-configuration.DONE.md`](10-playback-configuration.DONE.md). Do not mix those fields into this panel merely because the current UI uses one playback-definition modal from the Cuelist Pool.

## Renumber Cues

Cuelist Settings contains a **Renumber Cues** button. It opens a number-input modal with:

- title **Renumber Cues**;
- optional **Start Cue** field;
- **Cancel** and **Renumber** actions; and
- Enter as the confirmation shortcut.

If Start Cue is empty and the operator presses Enter, renumber the Cues consecutively from `1`. If the operator enters a positive whole number such as `10`, use that as the first Cue number and continue with `11`, `12`, and so on. Existing numeric Cue order is retained; “renumber” changes Cue numbers, not Cue row order or Cue contents.

For example, Cues `1`, `1.5`, `2`, and `7` become `1`, `2`, `3`, and `4` with an empty Start Cue, or `10`, `11`, `12`, and `13` with Start Cue `10`.

Renumbering is one revision-checked, atomic Cuelist mutation. It must:

- preserve Cue identity/order, title, changes, per-value timing, Cue timing, trigger, preview state, and tracking semantics;
- preserve the currently selected editor row;
- preserve the active/current Cue and next-Cue position without executing, restarting, or releasing the Cuelist;
- update current/next labels and every number-based internal reference to the new numbers;
- emit one object revision and one audit/undo operation rather than one mutation per Cue;
- reject an invalid, zero, negative, fractional, overflowing, or stale-revision request without partially renumbering; and
- survive Save/Reload exactly.

Cancel and closing the modal change nothing. A one-Cue Cuelist is still renumbered to the requested starting number. Disable the action only when no Cuelist is open or the Cuelist contains no Cues.

Implement a server-side renumber operation or equivalent transaction rather than issuing sequential browser updates. Sequential writes can create temporary duplicate numbers, corrupt an active Cue index, and leave a partially renumbered Cuelist after a conflict.

## Deleting a Cue, including the active Cue

Deleting an inactive Cue removes it atomically from the persisted Cuelist and recalculates future tracking without changing the currently rendered playback state.

Deleting the Cue that is currently active has a deliberate runtime hold:

- remove the Cue immediately from the persisted Cuelist and Cuelist View;
- retain the active Cue's fully reconstructed output contribution exactly as rendered at deletion time;
- do not release, restart, refade, jump, or recalculate output merely because the Cue object was deleted; and
- retain a runtime navigation anchor between the nearest surviving Cue before it and the nearest surviving Cue after it.

From that deleted-active anchor:

- **GO** executes the next surviving Cue in numeric order;
- **GO minus** executes the previous surviving Cue; and
- if GO first reaches the next surviving Cue, a subsequent GO minus follows the now-current list order and reaches the previous surviving Cue, skipping the deleted number.

The held output is runtime state, not a hidden Cue object and not a new persisted delta. Once GO or GO minus executes, reconstruct the destination from the modified Cuelist as though the deleted Cue never existed. Values introduced only by the deleted Cue therefore release or change according to the destination Cue's effective timing.

Playback runtime/API diagnostics must distinguish “holding output from deleted active Cue” from a normal current Cue and expose the deleted number plus previous/next surviving navigation anchors. The Cuelist View has no deleted row to select or mark active; its next indication may identify the next surviving Cue.

Deleting an inactive Cue before the current Cue must not move the active playback merely because vector indexes shifted. Runtime identity/navigation must be based on Cue order/identity rather than an unchecked array index.

The existing only-Cue safeguard remains: deleting the sole Cue is rejected without changing persistence or runtime. Delete, empty-programmer Record-minus, Cuelist-number addressing, and page-playback addressing must share these semantics.

## Persisted Cuelist settings

The Cuelist model needs these settings:

### Mode

- **Sequence**: normal GO-driven ordered Cue playback.
- **Chaser**: automatically advances through Cue steps using the configured Speed Group and rate multiplier.

### Chaser X-fade

Store a non-negative crossfade duration in milliseconds on the Cuelist, not on a particular page assignment. It controls the transition between chaser steps. Validate it against the effective step duration; do not silently create an ambiguous overlap when X-fade exceeds the step interval.

### Speed Group

Select exactly one of Speed Groups A, B, C, D, or E for a Chaser. Retain an explicit fixed-step fallback only for migrated shows that do not yet have a Speed Group. Normal newly configured Chasers require a Speed Group.

### Speed multiplier

Store a positive rate multiplier. `0.5×` means half as fast as the selected Speed Group and therefore twice the step duration. `1×` follows the Speed Group directly. `2×` means twice as fast and therefore half the step duration. The effective step duration is:

`60 seconds / Speed Group BPM / rate multiplier`

The UI should offer common exact choices such as `0.25×`, `0.5×`, `1×`, `2×`, and `4×` while the model validates a bounded positive value.

### Intensity priority mode

Expose **HTP** and **LTP** as the Cuelist's Intensity priority mode. This setting affects only Intensity/Dimmer attributes:

- **HTP** chooses the highest intensity among contributions in the winning numeric priority.
- **LTP** chooses the newest intensity contribution in the winning numeric priority, even when it is lower.

Color, Position, Beam, and other non-intensity attributes always remain LTP. The existing numeric Cuelist priority remains a separate setting and is resolved before HTP/LTP. Do not rename the numeric priority to “priority mode” or serialize these two concepts into one field.

### Wrap Around

Replace the ambiguous boolean `looped` behavior with a three-value setting:

- **Off**: GO at the final Cue does nothing. The final Cue remains current.
- **Tracking**: GO at the final Cue advances to Cue 1 as though Cue 1 numerically followed the final Cue. Values from the final tracked state remain active unless Cue 1 explicitly changes them.
- **Reset**: GO at the final Cue releases the Cuelist's tracked state before applying Cue 1. Attributes explicitly present in Cue 1 transition to Cue 1's values. Attributes contributed by the Cuelist but absent from Cue 1 transition back to the next authoritative underlying source or default. Use Cue 1's effective delay/fade timing for these releases instead of producing a hard Off frame.

Wrap Around applies to normal forward progression at the end. Do not infer reverse wrapping for GO minus without a separate product decision and test.

### Restart mode

Expose two restart modes for turning an Off Cuelist on again:

- **First Cue**: the default. ON, Toggle-to-On, or GO-activates after Off starts the Cuelist at its first Cue and reconstructs tracking from that Cue.
- **Continue Current Cue**: turning the Cuelist on again restores the last Cue that was current immediately before Off, including its reconstructed tracked state. It does not advance to the next Cue merely because the Cuelist was turned on.

If a Cuelist has never had an active Cue, Continue Current Cue starts at the first Cue. If the remembered Cue was deleted while the Cuelist was Off, fall back to the first Cue rather than choosing a neighboring Cue silently. The restart-mode setting persists with the Cuelist; persistence of the remembered runtime Cue across an application restart follows the separately tested playback-recovery policy.

### Force Cue Timing

Retain the separately specified **Force Cue Timing** setting. When enabled, each Cue's master Delay and Fade replace stored per-value Delay and Fade during execution without deleting the per-value data.

### Disable timing

Expose **Disable Cue Timing** as a Cuelist setting for rehearsal and testing. Enabling it preserves every configured duration but treats these durations as zero during execution:

- per-value fade and start delay;
- Cue master Fade and Delay;
- TIME-trigger wait duration; and
- Chaser X-fade.

GO remains manual and FOLLOW remains automatic; only their duration-bearing work becomes immediate. Chaser step cadence still comes from its Speed Group and multiplier so a Chaser remains operable, but each step change is a snap because X-fade is bypassed. Disabling timing must not rewrite Cue, trigger, or Chaser timing fields. Turning the setting off restores the original timing on the next execution.

Timing precedence is unambiguous:

1. If **Disable Cue Timing** is enabled, all affected durations execute as zero and **Force Cue Timing** has no runtime effect.
2. Otherwise, if **Force Cue Timing** is enabled, Cue master Fade and Delay replace per-value Fade and Delay.
3. Otherwise, explicit per-value timing applies and missing components fall back to Cue timing.

## Suggested schema shape

Names may follow repository conventions, but the persisted information must be equivalent to:

```text
CueList {
  mode: sequence | chaser
  numeric_priority: integer
  intensity_priority_mode: htp | ltp
  wrap_mode: off | tracking | reset
  restart_mode: first_cue | continue_current_cue
  force_cue_timing: boolean
  disable_cue_timing: boolean
  chaser_xfade_millis: integer
  speed_group: A | B | C | D | E | null
  speed_multiplier: positive number
  cues: Cue[]
}

Cue {
  name: string
  fade_millis: integer
  delay_millis: integer
  trigger: go | follow | time(seconds)
}
```

## Existing implementation seams

- `apps/control-ui/src/windows/CuelistWindow.tsx` currently owns both Cuelist Pool and Cuelist View rendering. Split its right-side editor/settings concerns into focused components when the single-file view becomes difficult to test.
- `apps/control-ui/src/api/types.ts` currently exposes `CueList.mode`, numeric `priority`, boolean `looped`, `speed_group`, and Cue timing fields. Extend the types before wiring controls.
- `crates/playback/src/lib.rs` owns `Cue`, `CueTrigger`, `CueListMode`, `CueList`, chaser timing, wrap progression, and HTP/LTP arbitration. New behavior must be engine-owned rather than simulated by the browser.
- `crates/server/src/main.rs` and the show-object persistence path must accept revision-checked Cue/Cuelist updates and propagate them into the live playback engine without restarting the show.
- The server/show transaction seam owns Renumber Cues so the browser sends one starting number and expected revision rather than rewriting each Cue independently.
- Playback runtime needs a deleted-active navigation anchor that survives the Cuelist object mutation until GO, GO minus, Off, or release resolves it.
- The Stage preview thumbnail generation in `CuelistWindow.tsx` remains the source for the Preview column and must continue to reconstruct tracked Cue state rather than showing only each Cue's delta.

## Backward compatibility and migration

Existing show files must remain loadable:

- Missing `intensity_priority_mode` defaults to **HTP**, matching existing Cue intensity arbitration.
- Missing `restart_mode` defaults to **First Cue**.
- Missing `force_cue_timing` defaults to `false`.
- Missing `disable_cue_timing` defaults to `false`.
- Missing `speed_multiplier` defaults to `1×`.
- Missing Cuelist-level Chaser X-fade defaults to `0`.
- Existing `looped: false` migrates to Wrap Around **Off**.
- Existing `looped: true` migrates to Wrap Around **Tracking**, which most closely preserves continuous looping.
- Existing `mode`, numeric `priority`, `chaser_step_millis`, and valid `speed_group` data remain readable. `chaser_step_millis` is the fallback for legacy Chasers without a Speed Group.
- Existing Cue `name`, `fade_millis`, and `delay_millis` map directly to the selected-Cue editor.
- Existing manual triggers map to **GO**. Existing zero-delay Follow maps to **FOLLOW**. Existing delayed Wait/Follow data maps to **TIME** when it represented a wait after the preceding Cue. Legacy Timecode data remains loadable but is not exposed as one of the initial three editor choices until Timecode receives its own product contract.

Migration must be deterministic and covered by a persisted old-show fixture. Saving the migrated show writes the new fields without dropping Cue changes, per-value timing, preview reconstruction, or playback assignments.

## Implementation order

1. Extend Rust schema, validation, migration, and playback behavior.
2. Extend server/API types and revision-checked update operations.
3. Refactor the Cuelist View table and selected-Cue editor; remove Status and playback action buttons.
4. Add title-bar Cuelist Settings, all persisted controls, and the transactional Renumber Cues modal/action.
5. Implement Chaser rate/X-fade, Intensity HTP/LTP, the three Wrap Around modes, both Restart modes, and timing-disable precedence in the playback engine.
6. Add Rust/API tests for timing, arbitration, wrapping, and migration.
7. Implement the UI scenarios in CUE-011 and CUE-012 only after the named controls and engine behavior exist.
