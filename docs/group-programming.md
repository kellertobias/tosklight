# Group-relative programming semantics

Groups retain ordered fixture membership. Empty groups are valid show objects and keep their
programming, presets, cue references, dynamics, master and derivation metadata. Unsupported
attributes are preserved and ignored for fixtures that cannot encode them.

## References and selection

- A normal group selection is live. Odd, even and every-Nth rules use one-based display order;
  every-Nth uses a zero-based offset into that order.
- Derived groups retain their source and rule. Resolution is recursive and cyclic references are
  rejected during show validation.
- A frozen group stores the exact ordered members plus source revision and capture time. Repeating
  `GROUP <id> GROUP <id>` or choosing **Select frozen group** creates a static selection.
- Manual fixture selection is static. Storing a live selection creates a derived group; storing a
  frozen selection retains frozen metadata.
- Manually replacing a derived group's membership is disallowed until the operator explicitly
  chooses **Detach derived group**. Chained derivations are supported; cycles are rejected.
- Frozen membership can be temporary or stored as a named group. Re-freezing is always explicit.
  Missing fixture UUIDs remain in stored order and are shown as warnings.

## Masters and arbitration

Group masters affect intensity only. The highest applicable group master is used when a fixture is
in multiple mastered groups. Scaling occurs after programmer/playback HTP/LTP resolution, while
values are normalized, and before fixture encoding. Grand master and blackout are applied after
group scaling. A zero master in one group therefore cannot extinguish a fixture that is also in a
contributing group with a higher master.

Group intent is resolved against current membership. Adding fixtures immediately makes group
values, group presets, group cue changes and group dynamics applicable without copying calibrated
fixture-native values. Removing a fixture stops live group resolution but does not delete manual
fixture values. Membership edits use object revisions and can be undone from show object history.
Validated show-object recompilation preserves running cue index, fade timing and pause state, so a
new member joins an already active group cue or dynamic immediately. Opening a different show uses
the selected transition policy and deliberately releases the previous show's playback identities.
Manual fixture overrides survive both addition and removal. Removing a fixture only stops future
live group resolution; it never deletes fixture-scoped programmer or stored data.

A frozen reference freezes membership only. Masters remain relationships of named live groups;
using a frozen selection does not copy or create a master. Fixtures still receive any master that
currently applies through their live group membership.

## Alignment and preload

Alignment operates over ordered selections in normalized space before fixture-native conversion.
Left and right interpolate in opposite directions; center mirrors from the center toward the edges;
out mirrors from the edges toward the center. Unsupported/discrete attributes are not interpolated.
The current selection order is authoritative. One fixture receives the first endpoint; two fixtures
receive both endpoints. Physical ranges are normalized per fixture, encoder inversion and curves
remain authoritative, and attributes marked as wrapping use the shortest normalized path.

Pending preload values are separate from the active preload scene. **Clear** removes only pending
values. Preload GO merges pending values into the active preload scene. Storage targets an explicit
preset slot or cue list/cue number and uses the normal target revision check.
Repeated pending Clear is idempotent and never changes the active preload scene or stored objects.
