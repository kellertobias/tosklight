# Highlight and step through

Highlight is a temporary highest-priority identification look. Select fixtures from Stage, Fixtures, a Group, or the command line, then press **HIGH** or `Alt+H`. The current valid ordered selection becomes the frozen original set: every member uses the default Highlight look of 100% intensity and white. The look is not a Programmer value and is never included by Record, Update, Merge, a Group, a Preset, or a Cue.

- **NEXT** or `Alt+Right` singles out the first member, then advances through the original order. The singled-out fixture stays in Highlight; every other original member uses Low Light at 10% intensity and blue.
- **PREV** or `Alt+Left` singles out the last member, then moves backward through the same order.
- **ALL** or `Alt+A` restores the complete frozen original set as the actual selection and returns every surviving member to Highlight.
- **HIGH** again removes the temporary Highlight and Low Light layers without clearing selection or Programmer values.

NEXT from the last valid item wraps to the first; PREV from the first wraps to the last. Fixtures outside the original set never receive either look. Editing a Group or making another selection while HIGH is active does not silently replace or grow the frozen set. Deleted fixtures are skipped without reordering the remaining members. Multi-head selection has already expanded into selectable heads; multipatched physical copies still share one logical step item.

The actual Programmer selection follows the Highlight focus. In All mode, encoders, Presets, special dialogs, and other value actions write every fixture in the original set. In Step mode they write only the singled-out fixture. The explicit value is retained in the Programmer. For each touched attribute, the real value immediately replaces that part of the temporary look, so it can be seen and tuned; untouched Highlight or Low Light attributes remain temporary. Moving away and back does not discard the explicit value.

The Fixture Sheet keeps the complete original set visible with a subdued treatment and marks the active step prominently. ALL returns every member to complete-selection styling. OSC and attached-hardware feedback report active state, All-versus-Step mode, index, total, and active fixture identity. Software, keyboard, OSC, attached hardware, and native extensions all call the same authoritative Highlight actions and share the repeat guard.

The transient layer is applied after ordinary Programmer, playback, and output resolution. Turning HIGH off therefore reveals the exact underlying winners in the next frame without an intervening default frame. Profile resolution, inversion, physical limits, Grand Master, Blackout, disabled routes, and hazardous safe values remain authoritative. Blind and Preload suppress live Highlight output while leaving its state inspectable.

The desk owns live Highlight output. Every surface shares the authoritative original set, position,
and touched-attribute state, because there is one of each. Saving or reopening a show never
restores live Highlight output or a stale step position.
