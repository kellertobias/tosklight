# Return Home in the Position Special Dialog

## Status and scope

Add a **Return Home** action to the Position special dialog. This is a programmer action for the current ordered fixture selection; it is not a Stage-geometry move, fixture-profile edit, playback release, or direct DMX reset.

## Operator behavior

The Position special dialog shall provide a clearly labelled **Return Home** button beside the existing relative-position controls. Activating it writes each selected logical head's Position attributes to that head's profile-defined home/default values.

If a profile does not define a home value for Pan or Tilt, use `50%` for that missing attribute. Defaults are resolved per fixture and per attribute, so a mixed selection may return to different raw positions. Fixtures without Position attributes are skipped without blocking valid fixtures. An empty selection follows the special-dialog selection policy and must never silently affect every patched moving light.

The operation is one deliberate programmer gesture. It participates in normal programmer fade, undo, Blind, Preview, Preload, Record, and Update semantics. It must not modify the fixture profile's stored defaults.

## Acceptance criteria

1. A mixed ordered selection with explicit, partial, and missing home values receives the correct per-head Pan/Tilt programmer values.
2. Missing Pan or Tilt defaults independently fall back to `50%`.
3. Fixtures without Position attributes remain unchanged and do not prevent valid fixtures from returning home.
4. The action behaves identically from software-only and hardware-connected layouts and provides visible failure feedback.
5. Undo restores the previous programmer values, and Save/Reload changes nothing unless the operator records or updates the result.
