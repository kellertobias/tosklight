# 10b2 — Command, Programmer, Group, and Preset representative migration

## Outcome

Migrate one existing root UI workflow for command/selection,
Programmer/encoder, Group, and Preset helper families.

## Done gate

- Four representative workflows use only public semantic intent.
- Ordered selection, relative/absolute Programmer behavior, empty/live Groups,
  and family-local Presets retain their exact contracts.
- Focused API/UI/OSC cases, architecture, inventory, parallel stress, and full
  E2E pass.

## Result

Completed on 2026-07-25.

- Promoted four existing semantic scenarios into one enforced root catalog spec:
  ordered command/selection state with Highlight isolation, absolute and relative
  Dimmer intent, empty live Group versus dereferenced selection behavior, and
  family-local Preset storage/recall through pool, keypad, API, and OSC.
- Preserved the scenario IDs and bodies while removing their former registrations
  from the test-bench subcatalog.
- The generated inventory now records 305 active root cases across 39 files and
  identifies all four promoted cases as `migrated-semantic-world`.

Verification:

- Control UI typecheck, architecture checks, inventory generation, and diff check:
  passed;
- promoted cases plus five-bench isolation stress: 6 passed using four workers;
- full browser E2E: 327 passed and 9 intentionally skipped.
