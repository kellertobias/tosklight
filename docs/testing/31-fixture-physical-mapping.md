# Fixture physical mapping calibration

These scenarios cover the TL-545 and TL-555 fixture-data implementation increments and their later live
activation. Calibration now feeds live output: installed Position calibration and per-axis overrides are
applied by the Position fitter
(`crates/light/adapters/headless/src/runtime/output_scheduler/dynamic_projection/physical_adapter/position.rs`),
sampled Zoom/Focus mapping curves by the optics fitter (`CompiledOpticsFitting`), and installed Color
calibration compiles each instance's Color fitter (`physical_adapter/color.rs`). Stage simulation of
these values is not covered here.

## Purpose and status

Root Playwright coverage: `tests/122-fixture-physical-mapping.spec.ts` (helpers in
`tests/bench/show-setup/{installedCalibrationScenario,fixtureMappingEditorScenario,gdtfTransferScenario}.ts`).
Production reports programming contract 1, so the semantic-output tests run under `npm run test:e2e`.

- Playwright, desk UI (Setup → Fixture Library editor and Show Patch): FIXTURE-MAPPING-001 steps 1–5 and 7;
  FIXTURE-MAPPING-002 steps 1–2; FIXTURE-MAPPING-003 step 4 (1496×761 and 1024×768); FIXTURE-OPTICS-001
  step 1; FIXTURE-INSTALLATION-001 steps 1–3 (Pan / Tilt → Position calibration… for root and copy).
- Playwright, API: FIXTURE-MAPPING-001 step 6 (.toskfixture export/delete/import) and live Zoom fitting
  through a sampled curve; FIXTURE-MAPPING-002 step 3 and the step-5 percent/Beam refusal;
  FIXTURE-MAPPING-003 step 3; FIXTURE-INSTALLATION-001 steps 2 (inversion/placement edits), 3, 4 (portable
  round trip only) and 5 (emitted Pan/Tilt); FIXTURE-INSTALLATION-002 steps 1, 2, 5, 6
  (zero gain, root/copy, refusals, exact replay, clear); FIXTURE-GDTF-001 steps 1–4 and 5 (exact-byte
  reuse only); FIXTURE-GDTF-002 steps 1–3, 4 (reused request ID, stale revision) and 6 (package source
  retention); FIXTURE-GDTF-003 steps 1–2 (300 fixtures);
  FIXTURE-GDTF-004 step 1 (preview writes nothing); FIXTURE-PHYSICAL-MOTION-001 step 4 (root/copy axis
  overrides, no stacking, emitted words, portable round trip); FIXTURE-OPTICAL-UV-001 step 4.
- A nonzero patch bracket angle on a profile without an authored Hinge bracket turns the whole
  fixture about its own transverse axis, as the Stage draws it; programmed Angles still reach the wire.
- Rust/Vitest evidence for the rest: MAPPING/OPTICS editor rules in
  `apps/patch-library/src/library/physicalMappingEditor.test.tsx`, `colorPhysicalEditor.test.tsx` and
  `crates/shared/fixture/src/profile/tests/{physical_mapping,color_physical}.rs`; GDTF/MVR in
  `crates/shared/fixture/src/gdtf/{read/tests.rs,profile_tests.rs}`, `crates/light/src/mvr_export_tests.rs`,
  `crates/light/adapters/headless/src/runtime/tests/{fixture_profile_api_tests,mvr_import_route_tests}.rs`
  and `crates/light/src/mvr_import/tests/mod.rs`; GDTF-005 (Architect) in
  `apps/viz-editor/src-tauri/src/session/mvr_preview.rs`, `crates/viz/document/src/tests/mvr_sources.rs`
  and `apps/viz-editor/src/MvrImport.test.tsx`; PHYSICAL-MOTION-001 steps 1–3 and 5–8 in
  `apps/patch-library/src/library/positionPhysical.test.tsx`,
  `apps/light-desktop/src/components/setup/fixturePatch/PositionCalibration.test.tsx` and
  `crates/shared/fixture/src/profile/tests/position_physical*.rs`; NATIVE-PRELOAD-001 in
  `crates/light/domain/engine/src/tests/preview_ownership.rs`,
  `crates/light/adapters/headless/src/runtime/tests/native_output_route_tests.rs` and
  `crates/viz/project/src/physical/tests.rs`; NATIVE-UV-002 in
  `apps/viz-renderer/src/ui/status/fixture_label_tests.rs` and `crates/viz/scene/src/values/tests.rs`.
- Remaining manual: NATIVE-UV-002 renderer captures (the GPU capture test is ignored by default) and
  physical rig/meter accuracy for every scenario.

## FIXTURE-MAPPING-001 — Author and preserve a response curve

1. Open the Fixture Library, create a test profile and open a mode's Channels tab.
2. Add a Zoom channel. Open Mapping, set its Continuous function to raw 0–255,
   physical 44–8 and explicit degrees. Open Details.
3. Verify Mapping quality is Unknown, the preview descends, and opening Details has not attached
   calibration or changed the channel value.
4. Choose Use sampled mapping, Add intermediate sample, then set the middle physical value to 30.
   The midpoint preview must change and both endpoint samples must stay 44 and 8.
5. Select Measured. Save must be blocked until Mapping source is supplied. Enter a bench/source
   description, revision 2 and Beam angle. Save and reopen; every value must survive.
6. Export/import the profile as a .toskfixture package. Verify source identity, resolution,
   samples, quality, source, revision and opening convention are unchanged.
7. Use linear mapping. Verify only samples are removed; provenance remains. Clear mapping
   calibration removes the optional data without changing function endpoints or raw defaults.

## FIXTURE-MAPPING-002 — Invalid samples stay visible

1. Add an interior sample outside the endpoint direction. Verify a precise error, no misleading
   curve preview and a blocked save; the entered value remains available to correct.
2. Repair it, then edit a function endpoint. Verify the existing sample is retained and the endpoint
   mismatch is reported. Use function endpoints explicitly to repair endpoint samples.
3. Verify duplicate/noninteger raw knots, nonfinite or f32-collapsed physical knots, incomplete
   samples and out-of-resolution raw values cannot be saved.
4. A raw interval with no available integer/f32 interior point disables Add intermediate sample
   and explains why. A different valid gap remains eligible.
5. Beam/Field convention requires a Zoom function with its own explicit degree unit. Percent Zoom
   cannot be treated as degrees. Incompatible behavior changes remove calibration deliberately.

## FIXTURE-MAPPING-003 — Resolution and existing profiles

1. Convert a calibrated channel between 8, 16, 24 and 32 bits. Raw knots rescale with the existing
   default/Highlight/function values; physical samples and their direction stay unchanged.
2. A resolution reduction that would collapse sample knots is refused visibly with the original
   mapping intact. Reselecting Continuous retains calibration and existing priority-reset behavior.
3. Load shipped profiles without the optional metadata. Their existing defaults, package identity
   and live output remain unchanged. No estimated physical value is labeled measured.
4. Inspect the existing Mapping/Details layout at 1496×761 and 1024×768. The function area scrolls
   within the modal; fields, errors and actions remain reachable without page-level overflow.

## Math and package evidence

Rust and shared editor tests cover signed/multi-turn ranges, descending and nonlinear curves,
clipping, all four raw widths, quantization, source requirements, JSON/package round trips,
f32 authoring parity and unchanged existing DMX resolution. Operator-path validation must record
its actual build and viewport separately from these unit results. Keep reports/screenshots under
the repository's canonical .artifacts paths.


## FIXTURE-OPTICS-001 — Author and retain a head optical path

1. In a mode's Color tab, create a physical path. Opening the tab alone must not create data.
2. Verify all declared native Color controls are included by channel identity. Cyan and Red aliases
   must not merge distinct native controls. Unknown source/transmission is visible.
3. Add two filter stages, give them distinct names, bind their native functions and reorder them.
   Save/reopen and package export/import must preserve order, UUIDs, bindings and provenance.
4. Set a fixed source to Measured without a source description. Save must fail visibly. Add the
   description and signed-independent nonnegative XYZ values; save succeeds. Black XYZ stays black.
5. In an additive source, retain explicit Visible/Ultraviolet/Infrared/Other non-visible roles,
   reversed native direction, maximum drive and response exponent through save/reopen.
6. An imported spectral filter or full-path measurement must survive unrelated edits. Changing
   native byte layout is refused. Deleted head/channel/function references block saving.
7. A native recipe must contain every participating channel exactly once, including parked wheels.
   Reject service/reset functions, foreign functions, duplicate/missing values and out-of-range raw
   values. Preserve U32 values. Measured appearance never implies exact cross-fixture matching.
8. Compare identities after a calibration-only change and a native function-domain change: the
   former updates the pinned appearance digest but keeps the native signature; the latter changes
   native compatibility. Installed Color calibration and appearance now compile each instance's live
   Color fitter, so a calibration-only change also changes the fitted native output.
9. Add a White channel outside the legacy color-system list. Saving the physical path must require
   owning that channel. Shared-head Color controls stay explicit dependencies; do not require plate
   RGB on an independent white beam head. Unmodeled owned functions remain visibly unknown.
10. A legacy fixture.control range authored as Fixed is not enough to establish a Color function.
    Require explicit Color classification before binding it or recording a native recipe; reset
    and other Control behavior functions remain excluded even if their label mentions Color.

## FIXTURE-INSTALLATION-001 — Independent Position calibration

1. Open a root fixture's Pan/Tilt dialog and Position calibration. Enter signed offsets, Estimated
   quality, source and revision; save, reload and verify the values.
2. Repeat for one multi-patch copy. The root and other copies keep their own values. Changing
   inversion, name, stage placement or a profile must retain saved calibration.
3. Reject nonfinite values, missing Measured/Manufacturer evidence and invalid revisions. The
   editor retains the draft and shows errors; Clear calibration saves absence for that instance.
4. Round-trip portable show/patch and Architect data. Old documents without the optional field
   still load. Malformed calibration is rejected through existing validation/recovery paths.
5. Observe output before/after saving offsets: for the same programmed Pan/Tilt degrees the emitted
   native words move by the saved zero offsets, and clearing restores them. Inversion still mirrors
   only the wire word and never stacks with the calibration; no physical accuracy claim.

## FIXTURE-GDTF-001 — Honest physical function export

1. Export a profile with multiple raw functions and gaps, descending Pan/Tilt/Zoom endpoints,
   angular-velocity functions and 8/16/24/32-bit/non-contiguous fine slots.
2. Inspect generated XML: preserve function boundaries, directed endpoints, explicit units,
   default/Highlight and InitialFunction. Gaps are NoFeature intervals.
3. Unsupported units or nonlinear sampled curves produce an actionable export diagnostic;
   no silently flattened physical result. MVR must report failures, including their actual reason.
4. Edit a profile with retained source GDTF. Export must describe the current profile and warn
   about the generated subset; original bytes must not silently replace edited physical data.
5. For a newly attached unchanged source, reuse its exact bytes and original mode names, including
   rich data the generator cannot represent. A revision-number-only change keeps the association.
6. Change appearance, function domains, mode order or the actual embedded mode subset while keeping
   profile IDs equal. Source matching must fail. Old database rows without association evidence
   remain unverified after migration; old bytes cannot conceal current generation errors.
7. Verify native MVR metadata still retains the complete profile. Standard GDTF optical, curve
   and native-provenance roundtrip remains a separate acceptance gate; current export tests do
   not establish that gate or full import fidelity.


## FIXTURE-GDTF-002 — Canonical preview and atomic import

1. Import a GDTF with 16-bit Pan on slots 1 and 3, default 32769, and directed physical
   endpoints +540° to −540°; put 8-bit Zoom 4°–50° on slot 2.
2. Preview must not publish a library revision. Disclose source-only geometry/optical information
   even if all attributes already map. Show mapping controls only for actual unknown attributes.
3. Confirm once. Verify all modes, exact raw values, fine-slot order, units and every supported
   function in the editor. Retrying the same request publishes exactly one revision.
4. Reject reused request IDs with changed payloads and conflicting expected revisions.
   Unknown custom names remain distinct; remembered mappings must not merge native channel IDs.
5. Unsupported executable relationships, missing/cyclic attribute references, invalid DMX domains
   and unsupported source constructs produce actionable diagnostics, never a silently guessed range.
6. Retain the exact original archive and association through save, package export/import to an
   empty library, and portable profile serialization. Ordinary edits leave the original evidence
   unchanged. Source integrity failures and conflicting DB-only reattachments are rejected.

## FIXTURE-GDTF-003 — Source identity and archive size in MVR

1. Export 300 fixtures sharing a source-bearing profile. Original bytes occur once; native fixture
   metadata uses references and contains no repeated base64 archive. Runtime/catalog projections
   omit source bytes while authoritative editable/portable profile revisions retain them.
2. Exercise actual MVR ZIP write/read with mixed-case manufacturer/model file names. Rehydrate every
   native fixture and verify original bytes, source hash and original association unchanged.
3. Export identical canonical snapshots carrying different source-only optical data. External MVR
   references must point to each correct source. A verified and unverified copy of the same profile
   must not share the verified export decision merely because IDs and channel data match.
4. Export edited and unverified profiles sharing the same original archive. Generated standard GDTF
   must not replace retained source evidence; separate associations share the original bytes.
5. Corrupt or remove the referenced archive. Native metadata cannot silently restore a different
   source; existing standards-based fallback remains distinct from successful native restoration.


## FIXTURE-GDTF-004 — Canonical MVR import and immutable preview bindings

1. Embed a directed 16-bit Pan fixture under an arbitrary mixed-case filename. Use two distinct
   archives with the same FixtureTypeID and a third filename containing identical bytes. Preview
   binds each MVR fixture UUID to the exact source and mode without writing the library.
2. Edit the library after preview. Apply to a new show and reopen it: the previewed source meaning,
   exact raw default, fine-byte order and physical endpoints remain unchanged. Different contents
   receive distinct revisions; identical source/mode bindings share one profile. The installation
   entry and portable document have the same show ID.
3. Each fixture is stored as a lean profile reference. Delete the library revisions and export the
   imported show: its retained immutable profiles still supply the exact original source bytes.
4. Native ToskLight metadata takes precedence over an unusable standard fallback. Conflicting
   immutable native contents fail before a new destination is created. Do not silently rewrite
   their revision, source association or installed calibration identity.
5. Invalid archives, ambiguous suffix matches, missing modes and unmapped attributes are visible
   and remain unresolved. Retain their source bytes once by content hash as opaque recovery
   evidence. A unique operator-approved mapping may be reused only for the exact source archive
   with a current association; a stale association cannot justify the mapping.
6. All-Skip publishes no fixture profiles. Successful new revisions notify attached library clients;
   identical publication is idempotent and a concurrent different revision is not overwritten.
7. Preview and apply see address occupancy from lean references, secondary splits and physical
   copies. Reimport preserves installed settings. Ordinary native import preserves secondary/copy
   addresses even when its primary is unpatched. Explicit ImportUnpatched and an address-conflict
   unpatch clear every output. A later fixture in the same import can use an address just freed.
8. For a fresh standard GDTF with multiple breaks, apply the available MVR primary address and
   explicitly leave additional breaks unpatched, with a warning. An imported non-primary break
   must not accidentally inherit the primary address.
9. Plan and commit 300 fixtures sharing one source profile. Profile projections share one instance;
   fixture records contain no repeated source archive; the show retains one complete profile.
   This case checks storage shape, not output cadence, UI latency or physical fixture accuracy.


## FIXTURE-INSTALLATION-002 — Independent installed Color observations

1. Open **Light source → Color calibration…** for a physical fixture with authored optical paths.
   Add an emitter gain of 0, Estimated quality and a source; save and reopen. Gain 0 is retained,
   while absent calibration means no installed correction. The library profile stays unchanged.
2. Save different observations for the root and one multi-patch copy. Change name, mounting pose,
   patch address and unrelated policy; both observations retain their exact values and evidence.
   New physical copies and Architect duplicates must start without Color calibration.
3. Add a CMY/wheel whole-path XYZ observation. Its native recipe contains each owned channel once,
   including parked wheels; 16/24/32-bit raw integers retain precision. Recipe editing sends no
   live values. Unknown color or filter transmission does not become a fabricated measurement.
4. Replace the fixture/profile/mode. Preserve old source identity and observations, visibly inactive
   when incompatible. Unrelated edits and portable show loading remain possible; editing the stale
   observation cannot silently rebind it. Clear it, then author a new observation against the
   server's current identity. Same-profile hardware replacement requires explicit operator review.
5. Reject malformed identities, duplicate emitters/recipes, missing required evidence, negative or
   nonfinite output, out-of-function raw values, and gains overflowing known source data. A failed
   save leaves the editor open with an error and performs no partial patch mutation.
6. Repeated request identity replays one exact sparse update. Saving or clearing one copy does not
   change the root, other copies, profile or preset/cue intent; only that instance's fitted Color output
   follows its own calibration.
7. Round-trip through portable inline/reference records and Architect patch DTOs. Full immutable
   profile identity is derived once per resolved mode; compact runtime projections cannot redefine
   identity. Source archives are not hashed per fixture frame or per emitter correction.

Software contract/interaction tests are evidence for these invariants, not physical meter accuracy
or the later Stage/fitting acceptance gate.


## FIXTURE-PHYSICAL-MOTION-001 — Exact geometry, motion and calibration contracts

1. In Fixture Library Geometry, opt in to physical geometry. Verify unknown quality remains
   unknown, measured/manufacturer quality needs evidence, and invalid pivots/axes/scales prevent saving.
2. Choose Fixed, Unknown and Hinge bracket forms. For Hinge choose its actual body and parent-frame
   pivot/axis. Verify changing a node preserves the declaration; removing a referenced body is an
   actionable validation failure. No GLB is required to retain the physical graph.
3. In Emitters & Motion, bind a named physical axis to an exact channel/function with absolute
   degrees. Verify directed multi-turn ranges, different heads and shared motor ownership.
   A static channel or degree/second mismatch is rejected; a velocity-only function remains velocity.
4. In Patch Pan / Tilt, author an individual axis zero/inversion pair on the root and a different
   pair on a physical copy. Save/reopen each. Confirm overrides replace the complete family pair,
   never stack two inversions or offsets, and survive portable save/load and sparse request replay.
5. Rename a part and alter unrelated beam/focus metadata. Calibration remains current. Change a
   real pivot or native angular mapping: retained overrides become inactive. Replace a fixture with
   another profile using identical template node IDs: its old overrides remain inactive. Ordinary
   replacement succeeds; trying to edit stale overrides fails atomically. Clear/re-author them.
6. Verify an open calibration dialog notices a new authoritative geometry identity. Duplicate
   physical hardware in Architect and confirm the new root/copies have no installed measurements.
7. Analytical checks: desk Y+90° becomes profile Z−90° through basis conjugation; the compound
   moving-mount reference resolves to (1,4,4) m; the bracket reference resolves its lens to
   (0,−450,−250) mm; inverted U16 +720→−720 with +30° zero maps requested +390° to raw 49151.
8. Verify generated wire schemas, exact source identities and axis overrides through snapshots,
   transactions and portable inline/lean records. With TL-546/TL-556 live activation, the Position
   fitter applies these overrides to semantic Position output (once, without stacking).


## FIXTURE-GDTF-005 — Architect exact preview and atomic apply

1. Import an MVR with a valid embedded U16 Pan GDTF under an arbitrary archive filename. With no
   installed matching profile, preview must resolve it and retain its directed degree endpoints,
   fine slot ordering and source bytes. Two different archives sharing FixtureTypeID retain
   different immutable revisions; matching names do not select a different source.
2. Replace or delete the picked file after preview. Apply imports the reviewed data. Cancel, a new
   successful preview and replacing the open document invalidate only their affected tokens.
3. Choose an out-of-universe address. Verify no fixture/profile/revision is committed, the preview
   stays available and correcting the decision permits one successful import. A successful token
   cannot import twice. Editing the destination patch requires a new preview.
4. Reimport a native export into its source show. Its own fixture address is not a conflict and
   normal default import keeps its addresses/copies. Another fixture overlapping a lean physical
   copy is reported. Architect still permits intentionally planned overlapping addresses; the
   live desk's stricter patch validation is unchanged.
5. A present broken/ambiguous GDTF remains unresolved even with an installed namesake. Keep
   unresolved and Skip are available; Choose address is not offered for missing physical meaning.
   A missing archive may use one exact installed revision, retaining its full source attachment.
6. Read limitations before Apply and after success. Completion warnings stay until Done. If another
   window cannot refresh after commit, say the import was saved and how to refresh that window;
   do not invite another import by reporting a write failure.


## FIXTURE-OPTICAL-UV-001 — UV control and visible appearance are independent

1. Create a physical path from a legacy additive UV channel. Its explicit `color.uv` identity
   remains **UV effect** even when the old emitter is marked visible.
2. An unmeasured legacy nonvisible zero swatch becomes unknown visible XYZ. A documented measured
   zero remains zero. A known nonzero visible violet contribution remains nonzero.
3. Save, reopen and package-round-trip each case. Exact native binding, UV purpose, optional
   visible XYZ and any spectrum spanning the UV/visible boundary remain unchanged.
4. ROOT PAR 6 declares a usable UV control with unknown visible appearance and spectrum. Missing
   optical measurement must not be described as an unsupported native control.
5. The later runtime acceptance in TL-557/559/560 verifies normalized UV drive, UV-only black,
   independent portable estimates, fixture replacement and unsupported destinations. This fixture
   data scenario does not claim those downstream behaviors already operate.


## FIXTURE-NATIVE-PRELOAD-001 — Exact partial ownership over Live

1. Patch a multi-head fixture and leave another fixture or split unpatched. Apply manual DMX
   overrides, including fine bytes, to channels outside the pending operation. Follow Preload.
2. Preview a same-value channel edit. Its channel changes from the diagnostic override to the
   projected native value; unrelated channels on that same fixture and its other heads stay Live.
3. Preview a manufacturer alias, function and explicit control action. Only actual winning
   consumers change. A shadowed alias or Freeze-suppressed edit does not erase a Live override.
4. Preview semantic Color with Direct mixing: unrelated wheel and UV overrides remain Live.
   With Intent mixing, controls the resolver explicitly parks open are owned. Inferred RGB systems
   work without authored color systems; a shared attribute can drive more than one native channel.
5. Preview Intensity on a fixture with virtual intensity. Its dependent channels follow the
   preview. Shared master and logical-head ownership stay distinct. Static channels stay static.
6. Preview Dynamic FixAt, Off and Release. Explicit ownership is retained even for an equal value
   or a missing/default fallback; unrelated live Dynamics keep progressing. Clearing Follow Preload
   restores Live without waiting for another DMX packet. Observation does not create a programmer.
7. Reject malformed ownership mask lengths atomically. Changed masks with identical raw values
   invalidate the Stage frame signature. Changed profile/installation identity rejects stale rows.

## FIXTURE-NATIVE-UV-002 — Passive Stage status and visible emission

1. Render native RGB magenta, warm white, UV-only and magenta plus UV using the same physical
   forward model as final DMX. Verify known visible XYZ separately from encoded UV drive.
2. UV-only with unknown or measured-zero visible output has no visible purple substitute and no
   black occluding cone or lingering visible beam. Its native drive and UV status remain active.
3. Existing labels show a steady UV mark, including activity from a dark secondary head. Estimated
   or incomplete visible prediction may show a small subdued triangle. Label visibility/coverage
   remain effective at wide and narrow sizes; no toast, banner, modal, sound or focus change occurs.
4. Known mixed RGB+UV output retains the RGB prediction; unknown UV leakage stays uncertain.
   Screen captures demonstrate renderer behavior, not fluorescence or physical-meter accuracy.
