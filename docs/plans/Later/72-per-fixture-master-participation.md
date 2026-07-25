# Per-Fixture Master Participation

## Status

**Later — specification only.** This plan records a future per-fixture Show Patch policy. It does not implement master behavior, patch UI, playback behavior, engine scaling, persistence, migration, help changes, or executable tests.

## Goal

Allow selected fixtures to ignore the desk's intensity masters when their role requires stable output independent of normal master scaling.

Show Patch gains two independent per-fixture policies:

- **Grand Master**: whether the fixture reacts to Grand Master; and
- **Group Masters**: whether the fixture reacts to Group Master scaling.

Both are enabled by default. Disabling one affects only that master family for the selected fixture and does not imply disabling the other.

## Scope

The policy applies to the fixture's master/shared head and logical heads wherever their intensity is owned by that physical fixture. It affects only channels semantically configured to react to the relevant master. It does not turn arbitrary Color, Position, Beam, Media, Control, static, or raw-DMX channels into intensity channels.

Grand Master bypass does not bypass Blackout, output-route disable, hazardous-fixture safety, desk lock, or emergency/safety suppression. Group Master bypass ignores every Group Master contribution and Group flash contribution for that fixture, including overlapping Groups, while preserving ordinary Cuelist, programmer, Highlight, and other ownership rules.

The plan must define interaction with fixture-profile channel flags such as `reacts_to_grand_master` and `reacts_to_group_master`. The per-fixture Show Patch policy is an additional instance-level opt-out; it cannot make a profile channel react to a master that the profile marks as inapplicable.

## Operator workflow

The two Show Patch columns use the standard `[SET]`-then-cell editing path. An ordinary click selects without mutating. The editor clearly labels **Enabled** and **Ignored** and warns that bypassing Grand Master or Group Masters can leave output live when a master is reduced.

Batch editing may be added only if it previews the exact fixture set and commits both policy and audit information atomically. Venue/visual-only objects and fixtures without applicable intensity channels show unavailable cells.

## Ownership and compatibility

These policies are portable show-patch data because they describe the intended behavior of specific show fixtures. Repatching universe/address does not change them. Existing shows migrate with both policies enabled.

Copying a fixture copies its policies. Changing profile/mode retains the policies but applies them only to channels that remain semantically eligible. Logical-head output follows the owning fixture's policy even when a Group contains the parent, one head, or several heads. Multi-patch instances share the logical fixture policy unless a later physical-output requirement explicitly introduces instance-specific master behavior.

Fixture Sheet master-source and limiting badges must use the resolved policy. A fixture that ignores Group Masters must not be presented as limited merely because it belongs to a Group with a Group Master below full.

## Acceptance coverage

1. Show Patch exposes independent Grand Master and Group Masters participation for applicable fixtures.
2. Existing and newly patched fixtures default to participating in both.
3. Ignoring Grand Master leaves that fixture's eligible intensity unchanged as Grand Master moves.
4. Ignoring Group Masters leaves that fixture's eligible intensity unchanged under every overlapping Group Master.
5. Group flash obeys the same Group Masters policy, including parent/head membership and overlapping Groups.
6. Disabling one master family does not disable the other.
7. Profile channel reaction flags remain an upper bound; patch settings are opt-outs, not opt-ins.
8. Non-intensity attributes are not scaled or reclassified.
9. Blackout, route disable, hazardous-fixture safety, and emergency suppression remain authoritative.
10. Fixture Sheet badges report effective limiting and do not claim a bypassed Group Master is active.
11. Venue/visual-only and inapplicable fixtures cannot receive misleading settings.
12. Existing schema-v1 and current shows migrate with both policies enabled, and repatching does not reset them.
