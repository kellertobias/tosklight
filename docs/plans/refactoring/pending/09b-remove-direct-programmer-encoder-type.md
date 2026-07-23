# 09b — Remove the direct programmer encoder type (adopts one Next/64 item)

## Context

Adopted from [`../../Next/64-minor-operator-polish.minor.md`](../../Next/64-minor-operator-polish.minor.md)
§"Remove direct programmer encoder type": fixed actions such as Lamp On / Lamp Off /
Reset belong in special dialogs or the DMX/timecode/master-macro surface, not in a
direct encoder type. Adopted into the refactoring queue because chunk 10 migrates the
programmer-values/encoder paths onto WS frames — deleting this encoder type **first**
means less surface to migrate and no dead direct-mode branch in the new transport.

Verified 2026-07-23 in `apps/control-ui/src/components/control/parameterControls/`:

- `DirectProgrammerPicker.tsx` (rendered from `ParameterControlView.tsx`);
- `model.ts:222` `directProgrammerChoices`;
- `useParameterController.ts:77` `directMode` state, `:62-63`
  `directChoiceActive`/`directParameterChoiceActive`, `:91` passes `directMode` into
  `useHardwareParameterEncoders`;
- further `direct` references in `parameterValueMutations.ts` (`directValueMutations`,
  `:107-123` — note this name also serves plain absolute entry; only remove what is
  exclusively the direct *encoder type*, not direct value entry),
  `parameterProgrammerState.ts`, `ParameterFamilyTabs.tsx`, `EncoderSurfaces.tsx`,
  `useParameterProjection.ts`.

The replacement surfaces already exist: the control special dialog
(`specialDialogs/control`) and programmer control actions (`features/programmerActions`)
provide Lamp On/Off/Reset-style actions.

## Work

1. Confirm every action reachable through the direct encoder type is reachable through
   the special/control dialog (compare `directProgrammerChoices` output against the
   control-dialog action set); if one is missing, add it to the dialog in this chunk.
2. Remove the direct encoder type: `DirectProgrammerPicker`, `directMode` state and
   plumbing, `directProgrammerChoices`, the family-tab/encoder-surface branches, and the
   hardware-encoder direct-mode path. Keep absolute direct *value entry* intact.
3. Check `docs/help/30-Programmer/**` for mentions of the direct encoder type; update the
   help if it is documented. `npm run manual` if help changed.

## Definition of done

- No operator-facing way to select a direct encoder type; all its actions available via
  the special/control dialog; hardware-connected and software-only layouts both verified
  (parity rule).

## Verification

```sh
npm run test:unit
npm run test:e2e-ui
npm run test:e2e   # full suite gate
```

Manual: `npm run open`, check encoder tabs in both software-only and hardware-connected
layouts, run Lamp On via the control dialog.

## Decisions

None — Next/64 states the removal plainly. The rest of Next/64 (history fold-up, dialog
polish, macro-surface redesign, icon, clock) stays feature work and is NOT adopted.

Sequence: before chunk 10 (less encoder surface to migrate to WS).
