# Scripted video takes

Replay a YAML list of visible actions in the running macOS app, with timed holds
and manual pauses. Start the script yourself, then record the desk and Visualizer
as separate video tracks. The runner sends pointer and keyboard input to the real
windows, so the recording includes the actual dialogs, selection and transitions.

## Run the starter take

Requires macOS, the repository's Node dependencies, and Xcode Command Line Tools
(`swiftc`). The native helper builds automatically on its first run.

1. Open ToskLight with a disposable copy of the Demo Show. Start with an empty
   fixture selection. The script changes the loaded show through normal UI actions;
   use a saved starting checkpoint for each take.
2. Open and frame the separate Visualizer. Set the window sizes and positions for
   recording, and keep the controls used by the take visible.
3. From the repository root, validate the script without sending any UI input:

   ```sh
   npm run demo:script -- docs/marketing/video/preset-demo.yaml --dry-run
   ```

4. Start the take:

   ```sh
   npm run demo:script -- docs/marketing/video/preset-demo.yaml --countdown 10
   ```

5. Start your recorder during the countdown. Leave the mouse and keyboard alone
   during replay. The runner focuses the specified app before each interaction.

The starter selects Beam Show, sets its dimmer to 70, applies Yellow and Fan Out,
then Dark Blue and Cross 1, with holds at 120 BPM. It was replayed successfully
against the native ToskLight window on an isolated demo show: the final UI reported
Dark Blue on 42 fixtures and Cross 1 on 34. This is a preset demonstration; the
complete patching, Architect, dynamics and cue sequence still needs authoring.

If Accessibility access is missing, the command stops with instructions for
System Settings > Privacy & Security > Accessibility. Enable the terminal or app
that launches the command, then rerun. The runner does not change permissions.

## Write a take

Each step contains one action. Use accessible control names from `--inspect`:

```yaml
version: 1
name: Preset rehearsal
bpm: 120
apps:
  control:
    bundleId: de.tokenet.light
    window: ToskLight
steps:
  - app: control
  - click: Busking
  - wait: 1s
  - click:
      name: "4 Beam Show "
      match: prefix
  - wait: 8 beats
  - pause: Place the remaining fixtures, then return here.
  - marker: final-fixtures
```

`pause` waits for Enter in the launching terminal. It pauses the script only;
pause the recorder separately if the off-screen work should be omitted. After
resuming, the next UI interaction focuses its app again.

| Action | Value and behavior |
| --- | --- |
| `app` | Focus a running app alias declared under `apps`. |
| `click` | A control name, or a selector object. Sends a pointer click at its accessible bounds. |
| `fill` | `{name: "Field label", text: "New value"}` replaces a text field's contents. |
| `key` | A letter, digit, Return, Tab, Space, Backspace, Escape, arrow, or chord such as `cmd+a`. |
| `wait` | Seconds as a number, or `500ms`, `2s`, `8 beats`, `4 bars`. Bars assume 4/4. |
| `waitFor` | A selector; waits up to ten seconds for one enabled matching control. |
| `pause` | A message; waits for Enter in the launching terminal. |
| `marker` | A named point in the timeline log. |
| `clickAt` | `{x: 400, y: 300}` in points from the target window's top-left corner. |
| `drag` | `{from: {x: 400, y: 300}, to: {x: 600, y: 300}, duration: 1s}`. |

For another window, add its bundle identifier and exact window title under `apps`,
then insert an `app` step with that alias. Optional `path` selects one exact `.app`
bundle when several builds are running. The runner refuses ambiguous apps, windows
or controls rather than choosing the first match. Apps must already be running.

Use named controls where available. `clickAt` and `drag` support canvas work but
depend on the rehearsed window size, zoom and camera state. Their execution order
and timing are unit tested; fixture placement in Architect still needs a native
rehearsal before it is used for the final recording.

### Find control names

```sh
npm run --silent demo:script -- docs/marketing/video/preset-demo.yaml --inspect control
```

This prints the window's accessible names and roles without clicking or focusing
it. For labels with changing counts, use `match: prefix` or `match: contains`.
Selectors also accept `role` and an exact accessible container name as `within`:

```yaml
- click:
    name: Settings
    role: button
    within: Groups pane
```

The `button` role includes buttons, checkboxes and radio buttons. `field` includes
text fields, text areas and combo boxes. Exact macOS roles such as `AXButton` are
also accepted. A click waits for a unique enabled match before sending input.
Controls outside the window bounds cause an error; scroll them into view during
rehearsal or add the required navigation steps.

## Rehearse, stop and recover

Use `--step` to press Enter before each step while rehearsing:

```sh
npm run demo:script -- docs/marketing/video/preset-demo.yaml --countdown 0 --step
```

To stop, press Ctrl+C in the launching terminal, or run this from a second terminal:

```sh
npm run demo:script -- stop
```

An input interaction already in progress finishes before cancellation. A wait or
manual pause is cancelled promptly. Later steps do not run. Stopping does not undo
show edits; restore the starting checkpoint and clear the selection before retrying.
In particular, replaying a group toggle from a different selection changes its result.

Each run prints its log directory under
`.artifacts/test/visual-inspection/demo-scripts/`. `timeline.json` records completed,
failed or aborted steps and measured timing; `script.yaml` preserves the take.
Canonical artifact environment overrides are respected. A failed action stops the
take and reports the step and reason. Missing or ambiguous names usually require
another inspection after opening the relevant desktop or dialog.

At 120 BPM, one beat is 0.5 seconds and one bar is 2 seconds. Holds begin after the
preceding UI action finishes. Window activation, readiness checks and UI response
add time, so beat-based waits do not provide frame-accurate musical synchronization.
Use the desk's cue/timecode playback for the final synchronized performance, and
use these scripts for the recorded setup and programming sequence.

Recording and full-quality rendering remain separate concerns. See
[Visualizer capture investigation](visualizer-product-demo-capture.md) for the
capture requirements. This runner does not start recorders or implement render export.

The implementation is in [the CLI](../../tools/demo-script/cli.mjs),
[the script runtime](../../tools/demo-script/runtime.mjs), and
[the native helper](../../tools/demo-script/NativeUI.swift). Run its focused checks:

```sh
node --test tools/demo-script/*.test.mjs
```
