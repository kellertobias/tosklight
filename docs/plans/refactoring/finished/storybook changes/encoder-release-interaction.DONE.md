# Encoder Release Interaction

## Status

Done.

## Operator contract

1. Do not show a **Release** button on the encoder face.
2. A normal physical encoder push opens that encoder's **Set Value** modal.
3. `[DEL]` followed by a physical encoder push releases that encoder's parameter for the current
   selection.
4. Keep **Release** as an explicit action inside the **Set Value** modal.
5. Releasing removes scoped programmer ownership. It must not write zero or another replacement
   value.
6. Only offer or execute Release when the encoder target has a scoped programmer value and writes
   are currently allowed.
7. After a successful `[DEL]` + encoder-push release, reset the shared command line so Delete is no
   longer armed.
8. A normal encoder push without Delete armed must continue opening Set Value. It must not release
   the parameter.

## Package and application boundaries

- `apps/ui-library` may expose an optional typed `onRelease` callback and may render that action inside
  the Set Value modal.
- `apps/ui-library` must not import command-line state or interpret `[DEL]`.
- Remove the inline release action from `TouchEncoderZones` in
  `apps/ui-library/src/encoders/TouchEncoder.tsx`.
- Keep the modal release action in `TouchEncoderEditor` and
  `HardwareEncoderDisplayView`.
- The `apps/light-desktop` encoder adapter owns the combination of the exact shared `DELETE`
  command state, physical encoder press, scoped programmer-value availability, release mutation,
  and command-line reset.
- Preserve ordinary encoder turns, relative coarse/fine behavior, absolute Set Value entry,
  ordered THRU spreads, indexed-value constraints, and hardware/software presentation.

## Storybook correction

- The ordinary individual encoder and encoder-family stories must not present Release as a
  permanent face button.
- Do not model release as `setValue(0)`: zero is still an owned programmer value.
- If a story demonstrates Release, give it explicit programmer-ownership state, expose Release
  inside Set Value, and transition the mock to an observable released/unowned state.
- Cover both a value with programmer ownership and a value that has no releasable programmer
  ownership.

## Verification

- Package test: the encoder face contains no Release button.
- Package test: Set Value shows Release only when `canRelease` and `onRelease` are both present.
- Package test: modal Release calls `onRelease` once and closes the modal without calling `onSet`.
- Desktop test: ordinary hardware encoder press opens Set Value.
- Desktop test: exact shared `DELETE` plus an assigned encoder press releases the matching scoped
  parameter, does not open Set Value, and resets the command line.
- Desktop test: Delete plus an encoder without a scoped programmer value performs no release
  mutation and does not create a zero value.
- Storybook focused render/interaction checks pass without REST or WebSocket traffic.
- Update `docs/help/30-Programmer/01-command-line.md` to document the two release paths.

## Result

- Removed the permanent Release action from the touch encoder face while retaining conditional
  Release actions inside both touch and hardware Set Value modals.
- Added application-owned routing for exact shared `DELETE` plus a physical encoder push. A
  releasable scoped value is released and the command line resets after the write succeeds; an
  encoder without scoped ownership performs no mutation, does not open Set Value, and leaves
  Delete armed.
- Corrected Storybook ownership mocks so release produces an observable unowned state rather than
  writing zero, and added a dedicated released/unowned encoder story.
- Documented modal Release and `[DEL]` plus encoder-push Release in the command-line reference.
- Verified with package encoder tests (11 passing), the focused desktop Parameter Controls tests
  (19 passing), package and desktop typechecks, and the complete Storybook gate (71 passing).
- The current Storybook browser interaction opens the owned touch encoder's Set Value modal,
  performs absolute entry, invokes Release, observes the released/unowned value, confirms
  disabled/indexed encoders cannot open the editor, and repeats Release through the hardware
  encoder editor. The ordinary encoder face remains free of a Release action.
- Included in the completed shared frontend and Storybook lane commit recorded by plan 02.
