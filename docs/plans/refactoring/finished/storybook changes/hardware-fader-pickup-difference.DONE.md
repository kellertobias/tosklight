# Hardware Fader Pickup Difference

## Status

Completed on 2026-07-26. The authoritative runtime projection, hardware-only presentation, and
integrated Storybook migration gates pass.

## Goal

Represent fader pickup only where a real non-motorized hardware fader can be physically out of
sync with the value the desk requires. Replace the dashed card outline with a direct visualization
inside the hardware fader:

- normal playback-color fill from zero to the physical hardware position; and
- a red difference segment connecting the physical position to the required pickup target.

For example, if the hardware fader is physically at 50% and must reach 75%, render the regular
fill from 0–50% and a red segment from 50–75%. The operator can immediately see both where the
hardware control is and how far, and in which direction, it must move.

## Operator meaning

Pickup means that a non-motorized hardware fader's physical position does not match the value at
which the desk can safely hand control back to it. It does not mean that a touch fader is at the
wrong position: a touch fader has no independent physical position and always displays the desk's
authoritative value.

Therefore:

1. Never show pickup-required state on a touch/software fader.
2. Never show pickup-required state on a faderless playback.
3. Show pickup only on the hardware playback surface that corresponds to a real physical fader.
4. Remove dashed pickup outlines from the complete playback card in touch and hardware modes.
5. Do not use a card border, warning dot, or anonymous amber/yellow marker as the primary pickup
   indication.

## Required data contract

The hardware presentation needs two distinct normalized values:

- `physicalPosition`: the latest real hardware fader position; and
- `pickupTarget`: the position the hardware fader must reach before it takes control.

Do not derive `pickupTarget` from the effective playback `master`, displayed output, or another
coincidentally similar value. Those values can diverge by design.

Current evidence:

- cue-list runtime exposes `fader_position` and `fader_pickup_required`;
- the current domain behavior created by Off latches pickup until the physical position reaches
  zero;
- the current wire projection does not expose a general pickup-target field; and
- `playbackFaderModeFeedback` currently hard-codes **Pickup: lower to zero**.

To support targets such as 75% truthfully, add an explicit typed pickup target at the authoritative
playback/control-surface boundary and project it through the existing wire/client/view-model path.
For today's lower-to-zero behavior, the authoritative target is `0`, not a value inferred by the
UI. If general pickup is not yet supported by the engine or hardware input path, keep the new view
model capable of representing it but never fabricate a non-zero target in production.

The UI-library component receives both values as typed presentation data. It must not calculate
hardware synchronization rules or import playback runtime services.

## Stacked hardware-fader visualization

### Target above the physical position

For a physical position of 50% and pickup target of 75%:

- 0–50% is the normal playback-color gradient;
- 50–75% is solid/gradient red difference fill;
- 75–100% remains the ordinary unfilled fader background;
- the physical-position boundary at 50% remains visually identifiable; and
- the pickup-target boundary at 75% remains visually identifiable.

The red segment is stacked directly above the normal fill. It must not make the fader appear to be
currently at 75%.

### Target below the physical position

For a physical position of 75% and pickup target of 50%:

- the normal fill still communicates the physical hardware position at 75%;
- a red difference segment spans 50–75%;
- the segment is below the physical-position edge, showing that the operator must move downward;
- the target boundary at 50% remains identifiable; and
- the graphic must not imply that the physical fader has already moved to 50%.

The component may layer the red segment over the corresponding portion of the normal fill, but it
must preserve a distinct physical-position marker at 75%. The same model must work in both
directions without swapping the meanings of the two positions.

### Equal and released states

- When physical position equals pickup target, render no red difference segment.
- When pickup is released, immediately return to the normal single-value hardware fader.
- A zero-width numerical rounding difference must not leave a one-pixel red artifact.
- Clamp rendering safely to the supported fader range while retaining the authoritative values for
  diagnostics.

## Labels and feedback

1. Keep the ordinary hardware fader value readable.
2. While pickup is required, show both values with explicit roles, for example:
   **Physical 50% · Target 75%**.
3. Do not label the target as the current fader position or current playback output.
4. If horizontal space is constrained, use a compact two-line label rather than hiding one value.
5. The red segment must be accompanied by accessible text; color alone is not sufficient.
6. Avoid the term **wrong position** in operator copy. Prefer **Move to 75%** or
   **Physical 50% · Target 75%**.
7. Directional wording must follow the actual relationship:
   - target above physical: **Raise to 75%**;
   - target below physical: **Lower to 50%**.

## Interaction and release

1. Moving the physical fader updates only `physicalPosition` and the size/direction of the red
   difference segment until the pickup rule is satisfied.
2. While pickup is required, the hardware movement must not jump the authoritative playback value
   merely because the UI has redrawn.
3. Reaching/crossing the authoritative target releases pickup according to the engine's exact
   threshold rule.
4. On release, the red segment disappears and ordinary hardware-fader control resumes without a
   visual jump.
5. Touch interaction cannot simulate or clear hardware pickup. A touch fader must not display the
   hardware physical position as though it were its own value.
6. Page changes, desk/session replacement, hardware disconnect, and reconnection must either
   restore an authoritative pickup pair or clear the visualization; never retain stale red
   difference state.

## Storybook states

The later playback-bank consolidation intentionally keeps only three playback stories. Pickup
coverage therefore lives in:

- **Configurable Playback**, whose controls expose hardware/touch mode, fader presence,
  `physicalPosition`, and `pickupTarget`, including equal, boundary, lower-to-zero, and released
  configurations without adding one story per permutation;
- **Eight By Two Hardware Bank**, which includes physical 50% / target 75% and physical 75% /
  target 50% examples in the same representative bank; and
- **Eight By Two Touch Bank**, which proves that the corresponding software surface renders no
  pickup visualization.

Authority replacement, hardware disconnect/reconnect, gradual approach, threshold release, and
one-of-many ownership remain runtime/package interaction tests rather than separate catalog
stories.

## Verification

- Domain/wire contract test: an authoritative pickup target is projected explicitly and is not
  inferred from `master`.
- Package test: the hardware fader view accepts separate physical and target values.
- Package geometry test: physical 50% / target 75% produces normal fill through 50% and a red
  segment spanning exactly 50–75%.
- Package geometry test: physical 75% / target 50% produces a red segment spanning exactly
  50–75% while the physical marker remains at 75%.
- Package test: equal values and released pickup render no red difference segment.
- Package test: clamped/boundary values do not invert or overflow the fader.
- Package accessibility test: physical value, target value, and direction are available without
  relying on red.
- Storybook visual test: no dashed pickup outline or anonymous yellow pickup marker remains.
- Storybook interaction test: the red segment shrinks as the mock hardware fader approaches the
  target and disappears at release.
- Cross-mode test: touch/software playback cards never render pickup-required presentation.
- Desktop adapter test: hardware pickup uses the real physical position and authoritative target,
  survives ordinary feedback updates, and clears on authority replacement.
- Existing playback runtime tests continue proving that movement before pickup does not jump the
  controlled value and reaching the target releases pickup.

## Acceptance

- Pickup is shown only for real hardware faders.
- No dashed playback-card outline represents pickup.
- The normal fill shows the physical hardware position.
- The red segment shows only the difference between physical position and pickup target.
- The target can be above or below the physical position.
- The display clearly distinguishes **Physical** from **Target** and never presents the target as
  the current hardware position.
- Touch faders remain ordinary authoritative value controls with no pickup warning.

## Non-goals

- Do not change the ordinary touch-fader gradient or touch interaction.
- Do not infer a target from `master` merely to avoid extending the contract.
- Do not make the red segment an output-level meter.
- Do not add motorized-fader behavior.
- Do not change playback activation/Off semantics unless a separately approved runtime plan
  expands pickup beyond today's lower-to-zero behavior.

## Result

Implemented the hardware-only pickup presentation end to end:

- added an optional authoritative `fader_pickup_target` to persisted playback runtime state,
  application projection, v2 wire projection/schema, generated TypeScript, client decoding, and
  legacy view projection;
- the existing Off behavior now explicitly publishes target `0` while pickup is latched and clears
  the target on release, without deriving it from `master`;
- added reusable clamped pickup geometry and a hardware fader view that preserves the normal
  physical-position fill, overlays only the red physical-to-target difference, keeps distinct
  physical/target markers, and exposes Physical, Target, and Raise/Lower text;
- removed the dashed playback-card pickup class, card status, and touch-fader
  `Pickup: lower to zero` feedback;
- touch and faderless cards ignore pickup presentation even if the same model/runtime data is
  present;
- added configurable Storybook coverage for equal/released, lower-to-zero, boundaries, and
  faderless/touch exclusion, plus exact 50→75 and 75→50 examples in the representative hardware
  bank; authority replacement and release remain covered at the runtime adapter boundary; and
- added domain, wire decoder, runtime-store authority, desktop adapter, package geometry,
  accessibility, cross-mode, and focused Storybook browser coverage.

Verification completed:

- `cargo test -p light-playback off_requires_zero_pickup_without_moving_the_recorded_fader`
- `cargo test -p light-application -p light-headless-runtime -p light-wire --lib --no-run`
- `npm run test --workspace @tosklight/ui -- PlaybackCards.test.tsx`
- `npm run test --workspace @tosklight/light-desktop -- PlaybackFaderBank.test.tsx PlaybackRuntimeStore.test.ts playbackWire.test.ts`
- `npm run typecheck --workspace @tosklight/ui`
- `npm run typecheck --workspace @tosklight/light-desktop`
- `npm run storybook:build --workspace @tosklight/ui`
- `npx playwright test --config apps/ui-library/storybook/playwright.config.ts apps/ui-library/storybook/tests/ui-stories.spec.ts --grep 'hardware pickup uses explicit'`

The shared integration failures were resolved. The complete UI suite passed 105 tests, the
complete desktop suite passed 1,993 tests, both TypeScript gates passed, and the integrated
Storybook run passed all 217 checks, including pickup authority, hardware-only rendering, and
touch/faderless exclusion. The complete `light-playback` and `light-wire` Rust suites also passed.
