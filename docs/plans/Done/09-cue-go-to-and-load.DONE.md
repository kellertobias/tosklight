# Cue Go To and Load Command Grammar

## Status

Implemented across the command grammar, desk-local selected-playback state, concrete playback runtime, UI/API/OSC feedback, operator documentation, Rust tests, and visible Playwright coverage in `tests/09-cue-go-to-load.spec.ts`.

## Operator intent

Give `[CUE]` a direct playback-execution meaning:

- one initial `[CUE]` is **Go To**;
- two consecutive initial `[CUE]` presses are **Load**; and
- the later `[CUE]` inside an explicit address continues to separate the playback/Cuelist address from the Cue number.

This replaces the current failure in which a bare `CUE <number>` command falls through to fixture-selection parsing.

## Command grammar

When a playback is selected:

| Command | Result |
| --- | --- |
| `[CUE] 8 [ENTER]` | Go To Cue 8 on the selected playback. |
| `[CUE] [CUE] 8 [ENTER]` | Load Cue 8 as the selected playback's next Cue. |

Without a selected playback, use the existing explicit `[SET]` address grammar:

| Command | Result |
| --- | --- |
| `[CUE] [SET] 5 [CUE] 8 [ENTER]` | Go To Cue 8 on explicitly addressed Cuelist/playback 5. |
| `[CUE] [CUE] [SET] 5 [CUE] 8 [ENTER]` | Load Cue 8 on explicitly addressed Cuelist/playback 5. |
| `[CUE] [SET] 4 [.] 7 [CUE] 8 [ENTER]` | Go To Cue 8 through playback 7 on page 4. |
| `[CUE] [CUE] [SET] 4 [.] 7 [CUE] 8 [ENTER]` | Load Cue 8 through playback 7 on page 4. |

The two initial `[CUE]` presses form the operation and must remain consecutive. The `[CUE]` after `[SET] ...` is part of the address and does not change Go To into Load.

If neither a selected playback nor a complete explicit address is available, reject the command without changing playback state. Do not guess a playback from whichever Cuelist or window was most recently visible.

## Go To behavior

Go To immediately makes the addressed Cue current rather than waiting for another GO press.

- If the playback is Off, activate it.
- Set that playback's fader to `100%`.
- Execute the addressed Cue using its normal effective Cue and per-value timing.
- Respect Grand Master, Blackout, playback priority, HTP/LTP arbitration, and other normal output controls; setting the playback fader to `100%` must not bypass them.
- Reconstruct the Cue's complete tracked state before execution. Do not apply only the delta stored directly in the target Cue.
- Clear any previously loaded-next-Cue override on that playback after a successful Go To.
- If the Cue or address does not exist, reject the command atomically: do not activate the playback, move its fader, change its current/next Cue, or alter output.

Go To changes only the addressed playback. It does not select fixtures, load values into the programmer, or modify Cue data.

## Load behavior

Load makes the addressed Cue the playback's pending next Cue without executing it.

- Do not activate an Off playback.
- Do not move the playback fader.
- Do not change current output or the current Cue.
- Expose the loaded Cue as the playback's next Cue in every relevant playback surface and API/OSC feedback.
- The next forward GO executes the loaded Cue, consumes the loaded override, and then resumes normal sequence progression from that Cue.
- Loading the currently active Cue is valid; the next GO re-executes it.
- Loading another Cue replaces the previous loaded override atomically.
- Off/release clears the loaded override. Command-line cancellation changes nothing. The final implementation must explicitly test whether GO minus preserves or clears a loaded override; do not infer this accidentally from vector indexes.

For an Off playback, Load remains silent until the operator presses GO or otherwise turns the playback on. GO then activates the playback and executes the loaded Cue under the playback's normal activation behavior; Load itself must never produce output.

## Address ownership and fader safety

Go To's automatic fader movement requires a concrete playback instance. A Cuelist Pool address that resolves to an unassigned Cuelist must be rejected with a visible error such as `Cuelist 5 is not assigned to a playback`. Do not silently assign it, choose an arbitrary playback, or create a new playback.

If one Cuelist is assigned to multiple playbacks, a page/playback address or explicit selected playback is required so the application knows which fader to move and which runtime state to change. A bare pool address must not affect all assignments.

Selected playback is runtime operator context, distinct from the Cuelist currently open in an editor. Selecting and retaining a playback must work consistently from software playback controls, hardware-connected controls, OSC, and restored desk/session state before the short forms are enabled.

## Runtime model

Playback runtime needs a transient loaded-Cue override separate from persisted Cuelist data. Its state should be equivalent to:

```text
PlaybackRuntime {
  current_cue: Cue identity or number
  normal_next_cue: Cue identity or number
  loaded_next_cue: Cue identity or number or null
}
```

The effective next Cue is `loaded_next_cue` when present and `normal_next_cue` otherwise. Do not reorder the persisted Cuelist or mutate Cue numbers to implement Load.

Cue edits, deletion, renumbering, show reload, playback reassignment, and application recovery must explicitly reconcile the transient loaded target. Prefer stable Cue identity internally so renumbering does not accidentally load a different Cue. If the loaded Cue is deleted, clear the override and visibly return to the normal next Cue.

## UI and feedback

- The command line must render the complete command before Enter and show a specific error for a missing selected playback, missing Cue, ambiguous assignment, or invalid address.
- A successful Go To updates current/next indicators, fader position, active state, playback feedback, OSC feedback, and relevant hardware surfaces from authoritative server runtime state.
- A successful Load visibly marks the loaded Cue as next without marking it current or active.
- Loaded state must be distinguishable from ordinary sequential next-Cue state for diagnostics and tests.
- The Cuelist View remains an editor: selecting a Cue row alone must not invoke Go To or Load.

## Existing implementation seams

- `apps/control-ui/src/components/control/softwareKeypad.ts` owns consecutive `[CUE]` command construction and must preserve the distinction between the initial operation tokens and the address separator.
- `apps/control-ui/src/components/control/CommandLineBar.tsx` owns software-key routing, command completion, and visible command errors.
- `crates/server/src/main.rs` owns `execute_programmer_command`, explicit playback-address parsing, validation, and authoritative command dispatch.
- `crates/playback/src/lib.rs` owns playback activation, jumping, tracked-state reconstruction, sequence progression, and the transient loaded-next-Cue override.
- The server playback state and `crates/control` feedback paths must expose current, normal-next, effective-next, loaded state, playback activation, and fader changes consistently to the main desk and hardware controller.

## Required tests

Add contract, Rust, API, and visible keypad coverage for at least:

1. `[CUE] 8 [ENTER]` on a selected active playback.
2. Go To on an Off playback activates it and sets only its fader to `100%`.
3. Go To reconstructs tracking and uses effective Cue/value timing.
4. `[CUE] [CUE] 8 [ENTER]` changes next Cue without output, activation, or fader movement.
5. GO consumes the loaded Cue and resumes sequence progression from it.
6. Both pool and page/playback explicit address forms.
7. Missing selection, missing Cue, unassigned Cuelist, and multiply assigned Cuelist errors are atomic.
8. Grand Master and Blackout remain authoritative after Go To.
9. Loaded-state replacement, Off/release clearing, Cue deletion, and Cue renumbering.
10. Software keypad, physical-key shortcuts, OSC/hardware input, and authoritative feedback parity.
11. Save/reload does not accidentally persist a transient Load unless playback-recovery policy explicitly requires it.

Update `docs/help/02-Programming/01-command-line.md`, its mirrored command-line documentation, and the Cue/playback testing scenarios in the same implementation change. Until then, this file is the planning contract only.
