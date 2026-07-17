# Programming Windows

Use the window that gives the clearest view of the current task; all of them operate on the same user programmer.

| Window | Primary use |
| --- | --- |
| Stage | Spatial selection, 2D/3D visualization, and Preload following. |
| Fixtures | Fixture/head rows, attributes, source ownership, ordering, and active/Cuelist filters. |
| Channels | Channel-oriented value and source inspection. |
| Groups | Reusable ordered selections and group masters. |
| Presets | All, Intensity, Color, Position, and Beam pools. |
| Dynamics | Placeholder for the planned effect editor; it currently contains no operator controls. |
| Cuelists / Cues | Cue content, order, timing, triggers, tracking, and execution. |
| DMX | Final universe output and diagnostic overrides. |

Selections made in Stage, Fixtures, Groups, Presets, or the command line are the same actual programmer selection. While using [Highlight and Step Through](02-selecting-and-setting-values.md#highlight-and-step-through), PREV and NEXT replace that actual selection with one item, ALL restores the current membership of the remembered live source, and any ordinary selection from one of these windows becomes the new complete step basis. Preset, encoder, dialog, and other value changes use the actual selection but do not reset the basis. HIGH remains independent and follows whatever is actually selected.

The Fixture Sheet is also the on-desk step-state view: remembered-base rows remain subtly selected while the actual step is prominent, including on multi-head rows and collapsed parents. The indication remains with HIGH off. The command bar and hardware simulator do not add separate Highlight status panels.

Pane settings are local to that pane. A Stage pane can follow Preload while another shows live output; a Preset pane can remain on Position while another shows Color.

See [Channel Faders](05-channel-faders.md) for the current Channels workflow. Dynamics remains a future feature documented under [Open Questions](../99-Development/01-open-questions.md); do not depend on it for show programming yet.
