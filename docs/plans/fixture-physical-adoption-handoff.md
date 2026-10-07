# Fixture physical data: TL-555 to TL-545 handoff

This is the implementation contract for [TL-545](https://plans.tokenet.de/TL/545), after
[TL-555](https://plans.tokenet.de/TL/555). Work in the existing `codex/intention-based-programming`
checkout. This handoff supplies configuration and reference mathematics; live fitting, DMX changes,
Stage activation and semantic preset/cue storage remain downstream issues. Software verification
and manufacturer metadata do not establish measured lamp performance.

## Sources and owned evidence

- [Architecture and dependency plan](fixture-independent-programming.md).
- [Numbered fixture-data acceptance scenarios](../testing/31-fixture-physical-mapping.md).
- [Current operator authoring controls](../help/10-Desk/10-Show-Setup/11-fixture-types-and-gdtf.md).
- [Repository fixture-authoring contract](../../.agents/skills/build-light-fixtures/references/tosklight-fixture-contract.md).
- [Representative archive audit](../../.artifacts/test/visual-inspection/intention-programming/representative-fixture-package-audit.md): 128 packages / 263 modes, existing identifiers, native binding examples and source gaps. Its recorded manufacturer links require revalidation during adoption.
- [TL-555 verification record](../../.artifacts/test/visual-inspection/intention-programming/tl555-gdtf-source-verification.md).

## Adoption order and gates

| Representative | First useful adoption | Gate before claiming physical correctness |
| --- | --- | --- |
| Cameo ROOT PAR 6 | Exact six-emitter RGBWAUV ownership; retain existing nominal XYZ as estimated | Explicit UV band; no measured UV/white output or spectra invented from swatches |
| Cameo AURO SPOT Z300 | Exact steady wheel states, directed Pan/Tilt and Zoom; Focus raw 6–255 | Use the real nine-slot chart, retain 0–5 Focus as unknown; opening convention and filter/source appearance need evidence |
| JBLED A7 | Exact RGB/CTC/macro ownership and all four modes | Separate balance from Reset functions; resolve 430°/300° native travel versus old 540°/270° geometry without changing the manufacturer's travel |
| Martin ELP CL / WW | RGBAL versus fixed-white source; retain distinct profiles | Color Scene/temperature interactions require explicit ownership; CL has no fabricated W/UV emitter; CCT alone does not define measured XYZ/SPD |
| Claypaky Stage Zoom 1200 / SV | CMY plus wheel, exact native states and optics | Verify/correct U16 functions authored as 0–255 with 32768 defaults; retain distinct identities and historical wheel repair provenance; optical order is not slot order |
| GLP JDC1 / ROBE zone modes | Multi-head topology and shared controls | Branch-specific dependencies: shared RGB plate controls are not white-beam controls; real emitter nodes and exact source references required |
| ETC Lustr Direct | Seven independent emitter bindings | Cyan is an additive emitter despite old canonical aliasing; HSI/Plus Seven behavior remains unknown until sourced |
| Two independent color wheels / velocity-only pan | Explicit synthetic contract/reference fixtures until a real source exists | Label synthetic data; never present it as a shipped manufacturer's measured behavior |

Missing data is a supported configuration result. Keep appearance, pivots, response and filter
transmission unknown where no reliable evidence exists. Nominal engineering models may be saved
as estimated with their derivation; they must not acquire manufacturer/measured quality by copying.

## Required profile fields

1. Preserve profile/mode/head/channel/function UUIDs and increment immutable profile revisions.
   Native function default/Highlight/service behavior stays explicit; routine Color ownership must
   not activate reset/lamp commands. Audit all modes, fine bytes, splits and independent heads.
2. Physical functions retain integer raw endpoints at 8/16/24/32-bit resolution, direction, units,
   optional monotone samples, evidence and calibration revision. Pan/Tilt use degrees or deg/s;
   Zoom uses full opening degrees and a declared beam/field convention; Focus remains percent
   or normalized lens setting. Do not infer metres of focus distance.
3. Declare `geometry.physical_contract` only after reviewing local mm, right-handed Y-up, neutral
   beam −Y and rigid ancestry. Rotations compose as `Rx * Ry * Rz`; use the shared basis helpers
   for desk XYZ. Explicit bracket: unknown, fixed or parent-frame hinge. A nonzero bracket without
   a hinge is unsupported. GLB artwork is not the optical pose contract.
4. Bind `mode.position_physical` to exact moving node/channel/function UUIDs and Pan/Tilt role.
   Keep multi-turn values unwrapped. Velocity-only travel cannot supply absolute Angle/Target.
   Absolute and velocity drivers on one axis require later runtime arbitration, not two positions.
5. Author `mode.color_physical` paths per head with complete native Color ownership, exact emitter
   and filter bindings, source type, physical filter order, active native function states and evidence.
   Alternate functions on one wheel are alternatives, not simultaneous serial filters. Preserve
   unknown appearance and explicit UV control purpose; never multiply XYZ as filter transmission.
6. Keep installation fields out of packages: family offsets/inversion and per-axis overrides,
   emitter gains and full-path observations belong to each actual root/copy. New duplicated
   hardware starts without installation measurements. Unchanged stale data remains portable and
   visibly inactive after replacement; a new edit requires current authoritative source identity.

## Authoring examples and reference values

The following fragments describe synthetic reference data, not real lamp measurements. Fill UUID
references from the selected profile; use the executable builders below for complete valid graphs.

```json
{
  "physical_mapping": {
    "quality": "estimated",
    "source": "Synthetic monotone descending zoom example; not manufacturer data",
    "revision": 1,
    "samples": [{"raw": 0, "physical": 44}, {"raw": 128, "physical": 30}, {"raw": 255, "physical": 8}],
    "opening_convention": "beam"
  }
}
```

This fragment belongs to a continuous Zoom function with raw 0–255, physical 44→8 and explicit
degree unit. Samples include authoritative endpoints. Dropping the samples restores linear mapping
without discarding evidence. Removing mapping metadata does not change raw/default/function values.

```json
{
  "physical_contract": {
    "version": 1,
    "provenance": {"quality": "unknown", "revision": 0},
    "bracket": {"kind": "unknown"}
  }
}
```

This is a declaration of coordinate interpretation, not certification of unknown dimensions.
For a sourced hinge, use its real graph node UUID and measured/manufacturer pivot/axis in the
parent frame. Build ancestry once later; do not call the allocating reference traversal per frame.

Executable examples (authoritative complete constructors):

- `crates/shared/fixture/src/profile/tests/physical_mapping.rs`: signed/descending U16 and U32,
  monotone curves, quantization, bounded versus angular velocity.
- `crates/shared/fixture/src/profile/tests/position_physical.rs`: exact graph/function ownership,
  multi-turn angle, per-axis source identity and stale replacement. Synthetic inverted axis with
  zero +30° maps calibrated +390° to native −360°; native U16 output rounds to 49151 and achieved
  angle is approximately +389.9945°. The family +999° offset is not added again.
- `crates/shared/core/src/spatial.rs`: compound moving mount, bracket/lens reference poses and
  desk-to-profile basis conjugation without artwork.
- `crates/shared/fixture/src/profile/tests/color_physical.rs` and `color_calibration.rs`: mixed
  emitter/serial path ownership, exact parked native recipes, unknown appearance and identity.
- `crates/viz/document/src/tests/mvr_sources.rs`: exact embedded archives, independent revisions,
  native self-reimport, library fallback, invalid source retention and atomic retry.

## TL-545 reviewable acceptance checklist

Delivery evidence is now recorded in [the TL-545 review report](../../.artifacts/test/visual-inspection/intention-programming/tl545-adoption-verification.md). The checklist below remains the original review checklist; use that report to distinguish automated, actual UI and unperformed physical/hardware checks.

- [ ] Revalidate each adopted manufacturer chart/manual and retain URL/revision/page evidence.
- [ ] Update representative packages through the fixture skill workflow; validate all shipped
  packages and selected-mode projections. Show unknown/estimated/manufacturer/measured correctly.
- [ ] Exercise actual existing Fixture Library Geometry, Emitters & Motion, Color, Mapping and
  Details tabs; save/reopen, mode switch, package export/import and duplicate profile.
- [ ] Exercise root and physical-copy Patch calibration modals. Confirm source changes while open
  retain drafts/errors, stale values survive unrelated edits, and clearing enables new authoring.
- [ ] Verify portable lean and inline saves, reopen on a machine without the source library,
  immutable revision replacement, native MVR and standard GDTF diagnostics. Unsupported executable
  semantics must fail/report explicitly with original evidence retained.
- [ ] In Architect, preview exact GDTF source and apply it after changing/deleting the source file;
  the staged meaning stays fixed. Cancel/new preview/new document invalidates tokens. Invalid
  address corrections can retry. Reimport does not conflict with the same existing fixture.
- [ ] Check warnings before import and after Apply, including unknown profiles, additional breaks
  and unsupported scenery. Keep unresolved/Skip are distinct from assigning an address.
- [ ] Run desktop/shared/Architect types and focused tests, build through the repository wrapper,
  verify the correct running bundle/readiness, and inspect required dialogs at 1496×761 and
  1024×768 with software and hardware-connected layouts where applicable.
- [ ] Save screenshots/reports under canonical `.artifacts/` paths and link exact evidence in
  PLAINER. Do not assert physical matching, live runtime activation or human acceptance.

## Downstream constraints that must survive adoption

- TL-546 owns one shared compiled forward simulation model; physical pose is separate from GLB.
- TL-547/TL-548 own semantic family ownership and coherent sampled output frames. Dynamic PSN
  updates must not write show history, rehash source archives or rebuild profile graphs per tick.
- TL-556/TL-557/TL-558 own Position, Color and Focus fitting. Consumers keep requested versus
  achieved values separate, including clipped/unknown/fallback diagnostics.
- TL-559/TL-560 and delivery tasks own cue/preset/Dynamics/import persistence. Normal Color
  recordings retain intent through RGB→RGBW→CMY replacement; derived native channels are never
  the substitute record. Explicit Direct Color recipes retain source identity and best-effort
  fallback rather than claiming fixture independence.
