# Replace a physical fixture

In Control, arm **SET**, open the fixture's **Fixture / mode** cell, and choose
**Replace fixture with another product**. Search by manufacturer, product or mode, then
choose the replacement personality. This changes the existing fixture; its number, root
identity, placement and stored programming stay attached to it.

Choose a target for every existing logical head. Correspondences preserve those logical
programming identities even when the replacement mode orders its heads differently.
**Leave unmatched — keep dormant programming** is an explicit decision: existing references
remain stored, but cannot drive a missing head. New unmatched heads receive new identities.
No correspondence is inferred from head names or indices.

For an old shared physical head, choose the new destination owners separately for each
programming family. Several destination heads may receive that existing source, or choose
**Leave unmatched — keep dormant programming**. This correspondence applies to existing
Preset, Cue, Group and held Programmer sources captured by this replacement. Group membership
and its spread order remain unchanged; later membership edits still affect the Group.
Fresh explicit master programming such as `FIXTURE 1.0` retains its ordinary master behavior.
A saved show with these correspondences requires programming contract 4; an older engine
must refuse to load or edit it rather than silently ignore the destinations.

Review the new footprint before pressing **Set**. Root and multi-patch addresses are retained
by split number; new splits start unpatched, and removed splits stop output. Overlaps or an
invalid footprint reject the entire replacement. If the show changed while the dialog was
open, reopen the editor and review the replacement again. Cancelling before **Set** writes nothing.
After **Set**, the dialog shows **Replacing fixture…** and waits for the atomic result.

Identity-bound installed calibration remains stored with its original source identity and
becomes inactive when the replacement is incompatible; revalidate or explicitly author a
new calibration. Semantic color and aim programming resolve through the replacement's
capabilities. A color wheel may approximate a requested color, unsupported attributes remain
passive, and Direct colors retain their original recipe with best-effort translation on a
foreign model. Save/reopen and inspect emitted DMX after a replacement.

This replacement confirmation is currently available in Control. Architect's existing
same-product mode editor remains separate.
