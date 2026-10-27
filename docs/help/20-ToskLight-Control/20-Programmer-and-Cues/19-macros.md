# Macros

The Macro Pool stores portable, show-owned command sequences. Tap to run; right-click or use
**SET** to open the editor. The editor validates command text, numbers lines, and provides
**Run Macro**, **Run line**, and the available safe **Undo last run** action. Use the
[Command Line Reference](01-command-line.md) for the grammar and
[Triggers, Chasers, and Speed Groups](13-triggers-chasers-and-speed.md)
for playback use.

## Run and edit a Macro

Enter `[^PBK] <macro-number> [ENT]` to run a Macro. Use `[SET][^PBK] <macro-number> [ENT]` to open its editor instead. Macros act on the current Programmer and selection.

A running Macro leaves any half-entered command unchanged. Its lines execute directly against the Programmer, so another control can start a Macro while you are typing.

Put `DELAY <seconds>` on its own source line to wait before the following line. The duration is non-negative and accepts up to three decimal places. `DELAY 1.5` waits 1.5 seconds; `DELAY 0.025` waits 25 milliseconds. Cancel the active wait from the execution display. Output, fades, and Dynamics keep running during the wait.

The desk validates the complete Macro before its first line runs. While it waits, other commands cannot interleave into its execution sequence. See [Macro command behavior](01-command-line.md#timecodes-and-macros) for the exact contract.
