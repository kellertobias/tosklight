# Acceptance Criteria

Until ToskLight reaches v1 (which will be stated in this document), a feature may
deliberately break persisted-show compatibility. Such a break must be called out
explicitly in the feature plan and result, and every repository-owned demo,
benchmark, test, and example show affected by it must be regenerated.

For a change that does not explicitly declare such a pre-v1 break:

- Existing valid files from supported earlier versions must continue to load, using an explicit migration or backward-compatible reader where necessary.
- A change that cannot safely infer a migration must stop and ask whether old files need to remain supported before the persisted schema is changed.
- Migration behavior must have a regression test containing representative legacy data.
- A failed file migration or invalid active show must not prevent the application from starting.
- Recovery errors must be visible and actionable. The application must preserve the original file and offer creation of a separate empty show instead of silently overwriting or deleting data.
- New-file initialization and successful migration must both be verified through the real server startup path.

A clean-install happy path is never sufficient evidence for a persisted-data
change: verify either the declared pre-v1 break and regenerated owned data, or
the applicable compatibility and recovery path above.

## Declared pre-v1 breaks

- **Semantic programming contract 1 (TL-552).** ToskLight now supports only programming
  contract 1. A show, desk session, Playback or Output runtime that still holds legacy
  normalized Position (`pan`/`tilt`), legacy Color component values (`color.red`, …) or a
  percentage Zoom is not loaded. A percentage is never reinterpreted as degrees or as a colour.
  - The rejection is visible and actionable. The original file or stored runtime JSON is kept
    unchanged, and runtime payloads are also copied to `backups/runtime-recovery-*.json`.
  - Desk settings are unaffected, and a separate show can be created or opened.
  - An Undo or Redo that would bring back such a pre-cutover version is refused and changes
    nothing.
  - Shows written by contract 1 carry the `light.programming_contract` metadata marker. An older
    contract-0 build refuses them.
  - Regenerated repository-owned data: `assets/demo.show`. `tests/fixtures/compact-rig.show`
    and `default-stage.show` hold no programming and were audited unchanged.
  - The plan is `docs/plans/fixture-independent-programming.md` §13. The checklist and result are
    in `docs/engineering/semantic-programming-cutover.md`.
