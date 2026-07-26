# Touch Encoder Continuous Surface

## Status

Completed on 2026-07-26 as the touch/software encoder follow-up to
[`encoder-release-interaction.DONE.md`](encoder-release-interaction.DONE.md).

## Goal

Replace the visible five-button touch encoder face with one calm, continuous relative-control
surface. Preserve the accepted information hierarchy:

- encoder type/name at the top left;
- the **Coarse** / **Fine** mode indication;
- the current formatted value at the top right; and
- an explicit, larger **Set Value** target for absolute entry.

The touch encoder must continue to behave like a relative encoder, not an absolute fader. The
coarse/fine step controls remain interaction semantics, but must no longer appear as individual
buttons.

## Current evidence

- `TouchEncoderZones` in `apps/ui-library/src/encoders/TouchEncoder.tsx` renders five visible
  buttons: `+10`, `+1`, `Set Value`, `−1`, and `−10`.
- `.touch-encoder-zones` in `apps/ui-library/src/styles/operator-surfaces.css` presents those
  buttons as five bordered rows.
- Pointer dragging is already handled at the outer `TouchEncoder` region. Vertical displacement
  selects progressively larger deltas and a timer repeats the active delta under one undo group.
- `TouchEncoder.test.tsx` currently treats the five visible buttons as the public interaction
  contract and therefore needs replacement coverage.

## Visual contract

1. Keep the current header structure and accepted labels. Do not remove or demote the encoder
   type/name, **Coarse** / **Fine** indication, or current value.
2. Remove all visible `+10`, `+1`, `−1`, and `−10` buttons, labels, borders, backgrounds, row gaps,
   and pressed-button chrome.
3. Do not replace them with differently styled plus/minus buttons, segmented rows, a fader track,
   or permanent numeric step labels.
4. Keep **Set Value** visible and make its touch target materially larger than the current middle
   row. It must be easy to hit without occupying the full encoder face.
5. Present the remaining body as one continuous surface. The Set Value target may be visually
   distinct, but it must not recreate a five-button stack.
6. Show that the continuous surface is vertically draggable without showing buttons. Use a subtle
   non-button affordance such as a vertical grip/motion mark, surface texture, or restrained
   directional treatment. The affordance must remain legible on touch screens without competing
   with the header or current value.
7. Do not rely on hover as the only drag indication. A grab/grabbing cursor may supplement, but
   not replace, the always-visible touch affordance.
8. Disabled and indexed-value presentations must retain the same uncluttered geometry and clearly
   communicate that relative interaction is unavailable.

The exact drag-affordance artwork remains subject to operator review. Its acceptance boundary is
that it communicates vertical dragging while no plus/minus or coarse/fine step button is visible.

## Tap zones

Treat the encoder body as a single composite interaction surface with three semantic tap zones.
These zones have no visible button borders, backgrounds, or step labels:

1. Tapping above the Set Value target applies exactly one positive fine step: `+1`.
2. Tapping the Set Value target opens the existing Set Value modal and performs no relative step.
3. Tapping below the Set Value target applies exactly one negative fine step: `−1`.
4. A stationary tap never applies `+10` or `−10`; coarse movement is drag-only.
5. The upper and lower tap zones should fill the available space around the enlarged Set Value
   target so there are no small dead strips or hidden coarse-tap zones.
6. Resolve the tap zone from the pointer-up interaction that began on the encoder. A small
   movement within the drag dead zone remains a tap; a recognized drag suppresses every tap
   action.
7. One gesture produces only one result: positive step, negative step, Set Value, or drag.

The `+1` and `−1` names describe the existing fine relative step, not an absolute normalized value
or a write of literal one.

## Drag behavior

1. A vertical drag may start anywhere on the encoder body, including the upper zone, lower zone,
   drag affordance, and Set Value target.
2. Dragging upward applies repeated positive relative changes.
3. Dragging downward applies repeated negative relative changes.
4. Close displacement uses the fine step (`+1` or `−1`).
5. Once displacement crosses the documented coarse threshold, the same uninterrupted gesture
   accelerates to the coarse step (`+10` or `−10`).
6. Moving back toward the start during the same gesture returns from coarse to fine when the
   displacement falls below the threshold. Crossing the origin changes direction without
   requiring a new pointer-down.
7. Keep a small dead zone so ordinary taps do not become accidental drags. Once the dead zone is
   crossed, suppress the tap that would otherwise occur on release.
8. Use one undo-group identity for the complete drag, including fine/coarse transitions and
   direction changes. Release/cancel ends that group and stops every repeat timer.
9. Pointer capture must keep the gesture active when the finger leaves the visible encoder
   bounds. A cancelled pointer must never leave continuous stepping running.
10. Drag speed/step feedback may appear transiently while dragging, but it must not become
    permanent button chrome. If shown, communicate direction and fine/coarse state without
    covering the current value.

The acceleration threshold and repeat cadence must be centralized named interaction constants and
covered at their boundaries. Do not derive coarse mode from which invisible tap zone the drag
started in.

## Other interaction paths

- Preserve wheel/trackpad relative changes and their documented fine/coarse modifier behavior.
- Preserve the existing Set Value modal, range entry, conditional Release action, and absolute
  entry semantics.
- Preserve application callback boundaries, programmer-relative mutation behavior, undo, and
  formatted live value feedback.
- Keep indexed values constrained rather than applying normalized relative steps that do not
  match their discrete option model.
- This correction applies to the touch/software encoder presentation only. Do not change the
  attached-hardware encoder face or physical encoder-push contract.

## Accessibility

Model the touch encoder as one composite control rather than five hidden HTML buttons:

- expose the encoder label, current value, active fine/coarse mode, and disabled/indexed state;
- provide keyboard-operable positive fine step, negative fine step, and Set Value actions;
- expose concise instructions that the upper/lower surface taps step and vertical dragging
  accelerates;
- retain visible focus for the composite control and Set Value action; and
- do not leave removed plus/minus controls in the accessibility tree as invisible buttons.

Touch, pointer, keyboard, and assistive interaction must invoke the same typed callbacks and
produce the same relative-versus-absolute distinction.

## Storybook states

Update `Encoders/Production encoder surfaces` and the application-owned touch-encoder composition
to show:

- assigned encoder with accepted header information and the uncluttered continuous surface;
- fine and coarse mode labels;
- transient upward fine, upward coarse, downward fine, and downward coarse drag feedback if such
  feedback is part of the final design;
- enlarged Set Value target and open Set Value modal;
- disabled encoder;
- indexed/constrained encoder; and
- released/unowned state without reintroducing a face-level Release button.

Stories must use the production `TouchEncoder` implementation and deterministic callbacks. They
must not contact REST or WebSocket services.

## Verification

- Package test: the encoder face has no visible or accessibility-tree buttons named `+10`, `+1`,
  `−1`, or `−10`.
- Package test: a stationary tap above Set Value emits one positive fine step.
- Package test: a stationary tap below Set Value emits one negative fine step.
- Package test: tapping Set Value opens the modal, emits no relative step, and has a larger
  computed hit target than the previous middle row.
- Package test: tapping anywhere outside Set Value never emits a coarse step.
- Package test: upward and downward drags may start in each of the three semantic zones.
- Package test: displacement on either side of the coarse threshold selects the expected fine or
  coarse delta.
- Package test: one drag can transition fine → coarse → fine, reverse direction, and retain one
  undo-group identity.
- Package test: pointer up and pointer cancel stop continuous stepping and suppress the trailing
  tap after a recognized drag.
- Package test: disabled and indexed encoders do not step or open Set Value.
- Accessibility test: the composite control exposes its label/value/instructions, is keyboard
  operable, and contains no invisible plus/minus buttons.
- Storybook visual check: no step-button chrome is visible at desktop and supported touch
  viewports; the drag affordance and enlarged Set Value target remain clear.
- Storybook interaction check: upper/lower taps, Set Value, fine/coarse dragging, modal entry,
  wheel behavior, and constrained states work without network traffic.
- Focused desktop adapter coverage confirms that relative steps still reach the authoritative
  programmer operation and Set Value remains the explicit absolute-entry path.

## Acceptance

- The header still shows the encoder type/name at top left, Coarse/Fine information, and the
  current value at top right.
- The encoder face no longer looks like a stack of five buttons.
- Upper tap means one positive fine step; lower tap means one negative fine step; Set Value opens
  absolute entry.
- Dragging anywhere vertically steps continuously and accelerates from fine to coarse with
  distance.
- Set Value is easier to hit, and the surface visibly suggests dragging without displaying
  plus/minus buttons.
- Hardware-only behavior and all unrelated encoder semantics remain unchanged.

## Non-goals

- Do not turn the software encoder into a fader or absolute vertical slider.
- Do not remove Coarse/Fine or current-value feedback from the header.
- Do not redesign the Set Value modal in this plan.
- Do not change physical encoder interaction, command-line release, OSC, or programmer merge
  semantics.
- Do not restore a permanent Release action to the touch encoder face.

## Result

Implemented and accepted on 2026-07-26.

- Replaced the five visible step buttons with one continuous touch surface, an always-visible
  vertical-motion affordance, and a materially larger central **Set Value** target.
- Upper and lower stationary taps emit exactly one positive or negative fine relative step.
  Vertical drag may start in all three semantic zones, uses centralized dead-zone, coarse-threshold,
  cadence, fine-step, and coarse-step constants, and retains one undo group across acceleration and
  direction changes.
- Pointer cancellation and release stop repetition and suppress a trailing tap. Wheel/Shift-wheel,
  explicit absolute entry, conditional modal Release, disabled, indexed, and released states remain
  distinct.
- The encoder is one focusable composite control with accessible instructions; arrow keys provide
  fine relative changes and Enter/Space opens **Set Value**. Removed step buttons are absent from
  both the visual surface and accessibility tree.
- Added deterministic disabled and indexed stories alongside the existing assigned and released
  states.

Focused verification completed:

- UI-library encoder tests: 8 passed;
- UI-library playback/encoder combined tests: 13 passed;
- desktop Playback Fader Bank regression tests: 42 passed; and
- repository architecture/source-size gate: passed.

The complete UI and desktop unit suites, both TypeScript gates, production Storybook build, and
all 209 integrated Storybook Playwright checks subsequently passed.
