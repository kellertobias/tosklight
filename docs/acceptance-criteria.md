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

- **Unified Playback contract 1 (TL-714).** Pages 1–99 own sparse assignments at positions
  1–1000. The regular pool, virtual number banks and page-slot maps are removed without
  migration or legacy aliases. Portable writers require schema floor 10 and
  `light.playback_contract=1`; desk layouts require schema 17. Incompatible files, runtime,
  named revisions and Undo/Redo content are refused before writable use. Originals remain
  unchanged, startup offers visible recovery and a separate fresh show/desk subtree, and
  affected owned shows must be regenerated. Implementation and combined acceptance are
  tracked separately in [the feature plan](plans/unified-playbacks.md).

- **Semantic programming contract 1 (TL-552).** ToskLight now supports only programming
  semantic values from contract 1. The additive live Preset reference feature below does not
  restore support for legacy values. A show, desk session, Playback or Output runtime that still holds legacy
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

## Additive programming features

- **Live Cue Preset references (TL-685), feature contract 2.** Supported contract-1 Shows
  retain literal Cue behavior. New references are authored only by an actual Preset recall;
  matching literal values never acquire links implicitly. Existing Presets receive a stable
  instance identity once, in the portable migration transaction. Literal-only Shows keep marker
  1, while references require marker 2 and older readers refuse them. Writers preserve an
  existing higher marker. Missing sources retain recorded fallback values and show local Cue
  guidance. Save/reopen, one-time legacy identity migration, literal isolation and active-Cue
  refresh have source regressions; packaged operator acceptance is recorded separately.
  See [the feature plan](plans/live-cue-preset-references.md).

- **Independent Cuelist addresses (TL-683), feature contract 3.** Cuelist and physical
  Playback numbers are separate namespaces. An independent-numbering mutation preserves
  meaningful legacy addresses as explicit canonical numbers and aliases, without rewriting
  physical assignments or Cue programming. Only authored Cuelist address metadata requires
  marker 3; existing contract-1/2 files retain their marker until this feature is used. Readers
  supporting at most 2 refuse marker 3. Ambiguous legacy addresses are rejected. See
  [the compatibility plan](plans/independent-cuelist-addresses.md). Implementation and operator
  acceptance remain separately tracked in the release evidence.


- **Phase-local Dynamic ordering (TL-807), feature contract 5.** Explicit Linear uses
  source order; a phase-local spatial choice uses its own inherited or overridden shape and
  the Dynamic's Projection. Each lane can retain a separate choice. Existing Selection and
  legacy spatial ordering variants retain their previous meaning and are not rewritten when
  opened. Only newly authored `source_order` or `spatial` ordering requires marker 5,
  including embedded Dynamic fallbacks; older readers refuse it before writable use.
  The SQLite schema is unchanged. Native and release acceptance remain separately tracked.

- **Venue architecture (TL-843), reader capability schema 11.** Existing shows remain at
  schema 10 and retain their scenery behavior. The first authored venue construction object
  promotes the portable reader floor to 11 in the same transaction; removing that object never
  lowers it. Schema-10 readers refuse the file before writable use, preserving walls, openings,
  slab outlines and furniture details. No SQL table migration is required. Save/reopen and
  Undo/Redo preserve the typed integer-millimetre construction contract.
