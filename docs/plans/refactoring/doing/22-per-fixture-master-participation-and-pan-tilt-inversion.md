# Per-Fixture Master Participation and Pan/Tilt Inversion

## Status

**DOING.** Claimed on 2026-07-30 after issue 21b completed.

## Goal

In the fixture page under **Show > Show Patch**, show and edit whether each fixture is controlled
by **Group Masters** and by the **Grand Master**, and allow the operator to configure **Invert
Pan** and **Invert Tilt** per fixture without editing its transferable fixture profile or changing
stored programmer and Cue values.

The four settings are independent:

- **Group Masters**: whether eligible intensity channels react to Group Master scaling and Group
  flashes;
- **Grand Master**: whether eligible intensity channels react to the Grand Master;
- **Invert Pan**: whether the physical Pan result is reversed; and
- **Invert Tilt**: whether the physical Tilt result is reversed.

Both master controls default to participating. Both axis inversions default to off. Changing one
setting must not imply or mutate any of the other three.

For an inverted axis, the normalized physical result is reversed:

```text
physical = 1 - programmed
```

Therefore `0%` produces the axis's highest physical value, `100%` produces its lowest physical value, and `50%` remains the midpoint.

## Operator workflow

Each fixture row exposes independent **Group Masters**, **Grand Master**, **Invert Pan**, and
**Invert Tilt** values. The fixture page must make the effective state visible without requiring
the operator to infer it from Group membership or current output.

An ordinary click continues to select the fixture or instance without changing show data. `[SET]`
followed by the cell opens the normal patch-cell editor. The master editors clearly show
**Controlled** or **Ignored**; the axis editors clearly show **Normal** or **Inverted**. Editing a
cell applies only that setting and saves as one mutation and one Undo step. Ignoring a master must
warn that the fixture can remain live when that master is reduced.

Pan/Tilt inversion is stored per physical fixture or multi-patch instance because physical units
sharing logical programming may be hung in opposite orientations. Master participation belongs
to the logical fixture: its master/shared head and logical heads follow the same policy even when
a Group contains the parent, one head, or several heads. Multi-patch instances therefore share
master participation unless a later physical-output requirement explicitly changes that contract.

Fixtures without an applicable axis or eligible intensity channels show unavailable cells rather
than accepting meaningless settings. Unpatched fixtures retain all configured policies so their
behavior resumes when they are patched again.

## Master participation behavior

Grand Master and Group Master participation affect only channels semantically configured to react
to the relevant master. They do not turn Color, Position, Beam, Media, Control, static, or raw-DMX
channels into intensity channels.

The per-fixture policies are additional instance-level opt-outs. Fixture-profile flags such as
`reacts_to_grand_master` and `reacts_to_group_master` remain the upper bound: a Show Patch setting
cannot make a profile channel react to a master that the profile marks as inapplicable.

Ignoring Group Masters bypasses every Group Master contribution and Group flash contribution for
that fixture, including overlapping Groups. It does not change ordinary Cuelist, Programmer,
Highlight, or other ownership rules.

Ignoring the Grand Master does not bypass Blackout, output-route disable, hazardous-fixture
safety, desk lock, or emergency/safety suppression. These safety boundaries remain authoritative.

Fixture Sheet master-source and limiting badges must use the resolved policy. A fixture that
ignores Group Masters must not be presented as limited merely because it belongs to a Group whose
Group Master is below full.

## Pan/Tilt resolution and profile interaction

Pan/Tilt inversion is a show-patch property of the physical instance. It is separate from:

- the fixture profile's channel-level raw-DMX inversion;
- the fixture's physical Pan/Tilt range;
- 3D mounting rotation;
- programmer values, Presets, Cues, and tracking; and
- encoder direction preferences.

The engine applies the patch inversion to the normalized Pan or Tilt request and then uses the
profile's authored range, resolution, fine-byte layout, and raw inversion to encode the result. A
profile-authored raw inversion and one enabled patch inversion therefore reverse the direction
twice and cancel physically; neither inversion may be accidentally skipped or applied more than
once.

Every projection of that physical instance must agree with its output. Normal output, Preload, Cue
transitions, Move in Black, Highlight if Position is ever included, DMX inspection, and 2D/3D
Stage motion all use the same resolved inverted axis.

## Ownership, persistence, and compatibility

All four policies are portable show-patch data because they describe the intended behavior and
physical orientation of fixtures in this show. Existing shows migrate with both master policies
enabled and both inversion flags off. Repatching universe or address changes none of them.

Copying a fixture copies its master policies. Copying a physical fixture or multi-patch instance
copies its inversion flags. Changing profile or mode retains all policies, applies master
participation only to channels that remain semantically eligible, and preserves an inversion as
dormant compatible data when its axis is temporarily absent. None of these settings becomes a
fixture-library revision.

Batch editing may be added only if it previews the exact fixture set and commits the policies and
audit information atomically. Venue or visual-only objects and fixtures without applicable
channels must not receive misleading effective settings.

## Acceptance coverage

1. The Show Patch fixture page visibly exposes independent Group Masters, Grand Master, Invert
   Pan, and Invert Tilt cells wherever each setting applies.
2. Existing and newly patched fixtures default to participating in both master families, with both
   axis inversions off.
3. Ignoring Grand Master leaves that fixture's eligible intensity unchanged as Grand Master moves.
4. Ignoring Group Masters leaves that fixture's eligible intensity unchanged under every
   overlapping Group Master and Group flash.
5. Disabling one master family does not disable the other or bypass Blackout, route disable,
   hazardous-fixture safety, desk lock, or emergency suppression.
6. Profile master-reaction flags remain an upper bound; patch settings are opt-outs, not opt-ins,
   and non-intensity attributes are not scaled or reclassified.
7. Parent/head membership, logical heads, and multi-patch output follow the documented policy
   ownership, and Fixture Sheet badges report effective limiting accurately.
8. Normal Pan/Tilt maps `0%` to the low endpoint and `100%` to the high endpoint; Inverted maps
   `0%` to the high endpoint and `100%` to the low endpoint.
9. The Pan/Tilt midpoint remains unchanged and 8-, 16-, 24-, and 32-bit channel encoding preserves
   the reversal without coarse/fine corruption.
10. Inverting one axis does not change the other axis, either master policy, or any stored
    Programmer, Preset, Cue, or tracking value.
11. Profile raw inversion and patch inversion compose exactly once, and normal output, Preload,
    transitions, Move in Black, DMX inspection, and Stage visualization agree.
12. Multi-patch instances can use different axis inversions while retaining one shared logical
    Programmer value and shared logical-fixture master policies.
13. Fixtures without the relevant attribute, eligible intensity, or physical output cannot receive
    a misleading effective setting.
14. Existing schema-v1 and current shows migrate to the documented defaults; unpatched fixtures
    retain their settings; repatching does not reset them.
