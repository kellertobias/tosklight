# Product Demo Pacing, Group Icon, and Final Stage Look

## Status

**Doing.** Claimed on 2026-08-02. This plan is now the sole active implementation contract for the
next reviewed revision of the maintained product-demo video.

Implement this plan against the single maintained `DEMO-001` workflow in
`tests/bench/show/productDemoScenario.ts`. Preserve that workflow as a real regression test and
keep its deterministic 25 fps edit timeline. Video acceleration changes below refer to retained
video time and visible click/type pacing, not to weakening assertions or racing the application.

## Progress

- [x] Claimed from `docs/plans/Next` after completing plan 00.
- [x] Audit the maintained DEMO-001 workflow, deterministic edit timeline, current generated-video
  path, and the previously observed Show Patch navigation blocker.
- [x] Implement the exact Patch, Group-selection, and icon-picker pacing contract.
- [ ] Trace and correct the authoritative Pan/Tilt and color path into the final Stage rendering.
- [ ] Run focused checks, canonical demo inventory verification, retained-video review, major suites,
  and the real desktop path.

## Decisions

- The literal retained-video boundaries and visible Fixture Sheet/icon-picker interactions below
  are acceptance requirements; faster setup must not bypass them through API-only shortcuts.
- Earlier focused Stage geometry checks are supporting evidence only. A previous maintained-demo run
  stopped before fixture patching because Show Patch did not become visible, so current video and
  operator-path proof must be established afresh.
- The current authoritative visualization/provider and Stage hierarchy are structurally correct.
  The closeout defect is in demo state and proof: the final position values are uniform, the color
  assertion stops before profile-calibrated output, and the final Busking desktop has no Stage pane.
  Renderer changes require contrary retained-frame evidence rather than assumption.
- Catalog icon choices must persist a stable short identifier, not a Vite-generated asset URL.
  The generated URL exceeded the Group wire contract's 64-byte icon limit in the real workflow
  and would not be a portable show value across builds. Rendering retains the bundled URL as
  separate catalog metadata and recognizes the current-build URL as a compatibility alias.

## Verification

- `npm run test:e2e -- tests/product-demo.spec.ts --workers=1` passed the complete maintained
  BENCH-PRODUCT-DEMO-001 workflow in 3.5 minutes. The historical Show Patch navigation blocker is
  therefore no longer current.
- The current editor keeps one deterministic 25 fps duration per title-card section. Raw interaction
  pacing is controlled by `DeskDriver` click delays and explicit frame holds, but the edit timeline
  has no named intra-section markers for the required Patch/Group speed boundaries. Plan 00A must add
  those markers and their mapped retained frame/timecode evidence without weakening DEMO-001.
- Source audit traced semantic values through `/api/v2/output/visualization`, engine profile output,
  the single desktop visualization provider, Stage attribute ownership, authored pan/yoke/head
  hierarchy, and emitter/beam color updates. Existing renderer tests cover those seams, but the demo
  currently proves neither non-uniform final motion nor calibrated multi-color Stage output.
- `npm run test:ui-package` passed typecheck and all 176 UI-library tests after separating stable
  catalog icon identifiers from their bundled rendering URLs. A raw root Vitest invocation was
  also attempted but lacked the UI package's jsdom configuration, so its 36 `document is not
  defined` failures are invocation/configuration failures rather than product results.
- The updated full `npm run test:e2e -- tests/product-demo.spec.ts --workers=1` workflow passed in
  3.5 minutes outside the sandbox. It now performs the required visible Patch batches, saves the
  stable `icon:fixture-type/profile-moving-light` value, and proves the resulting catalog image on
  Group tile 1. The first sandboxed attempt stopped before the scenario with loopback `listen
  EPERM`, so it is recorded as an environment restriction rather than product evidence.
- `npm run test:bench-types`, the two focused `productDemoTimeline` Vitest cases, the demo encoder's
  Node syntax check, and `git diff --check` passed. The timeline unit proves the remaining-layer
  fixed window is exactly 125 frames at 25 fps and rejects missing named markers; canonical retained
  source/target timecodes still require the recording run.

## Remaining work

- Complete the audit and every unchecked progress item above.

## Goal

Remove repetitive setup time while retaining the first representative interaction of each useful
workflow. Give the Group icon picker enough time to be understood, demonstrate selection from the
Fixture icon set, and finish with a Stage view that visibly proves real moving-light position and
color values.

## Show Patch and layer pacing

Keep creation of the **Stage & Venue** layer at the current readable pace. Immediately after that
layer is committed, compress creation of all remaining Patch layers—including **Trusses** and all
Profile, Wash, LED PAR, and Conventional Light layers—into approximately **five seconds total**.
The section must not spend close to a minute replaying identical layer-name and confirmation work.

Keep the first representative fixture placement readable, then use the following boundaries:

- keep **Front Truss Left** creation and placement at the current visible pace;
- fast-forward **Front Truss Right** creation and placement;
- keep the already-demonstrated **Profile Stage Center** workflow at normal pace;
- fast-forward the corresponding **Profile Stage Left** and **Profile Stage Right** repeated work;
- retain the four independently addressed **House Lights**, but type their three additional DMX
  addresses substantially faster; and
- create and place the first complete ACL set visibly, then visibly add the Stage Profile fixture
  batch before entering the final remaining-lighting fast-forward.

The final fast-forward that completes the rest of the canonical lighting show must run faster than
the current version. Preserve the visible destination-layer changes so the Patch surface still
communicates where each fixture family belongs, but minimize per-item delay and dead holds. Do not
change canonical fixture identities, layer ownership, patch addresses, positions, multipatches, or
the exported show inventory merely to shorten the recording.

## Group-programming pacing

Select the first Group's fixtures from the Fixture Sheet at **twice the current visible speed**.
Every fixture must still visibly become selected in fixture-ID order; this is an edit-pacing change,
not permission to replace the demonstrated Fixture Sheet workflow with an API selection.

After the second Group has been completely created and named, accelerate the intervening repeated
Group creation sequence to **four times the current visible speed**. Return to normal readable pace
when selection for **Beam Show** begins. The exact retained-video boundary is therefore:

1. normal pace through completion of the second Group;
2. 4x from the next repeated Group action through completion of the last intervening first-level
   Group; and
3. normal pace again before selecting the fixtures or source Groups used to create **Beam Show**.

## Group icon picker

For the first visibly edited Group icon, do not open and immediately dismiss the icon modal.

1. Open **Choose icon** and hold the complete modal on screen for a few seconds so the icon choices
   can be seen.
2. Open **Icon group**.
3. Select **Fixture type**.
4. Hold the Fixture type icon grid long enough to read it.
5. Select **Profile Moving Light** (`profile-moving-light.svg`).
6. Save the Group with that catalog icon and prove the icon is visible on its Group tile.

This replaces the current built-in/family-icon choice for that visibly demonstrated Group. The
remaining generated Groups may retain their canonical icons unless their derivation depends on the
changed Group.

## Live pan, tilt, and color in Stage

The final Stage proof must not show nominal programmer data while all moving fixtures remain in the
same rendered orientation. Investigate the complete authoritative path from the recalled Position
or Dynamic values through visualization state into the Stage model hierarchy. At least one clearly
visible moving-light set must receive non-default, non-uniform **Pan** and **Tilt** values, and the
recorded Stage must visibly show the resulting base/yoke/head or beam movement. API state, encoder
text, or DMX values alone are not sufficient proof.

Likewise, the final illuminated look must not render every light white. Apply the canonical Color
preset/programmer values used by the concluding look and ensure the Stage visibly renders at least
two distinguishable colors across appropriate color-capable fixture families. Conventional white
sources may remain white; the acceptance failure is an all-white Stage despite programmed color
values.

If the canonical final look already contains non-white normalized RGB values but the recording is
white, fix the visualization-value or renderer path rather than adding duplicate demo-only state.
Keep one provider authoritative and prove that the retained final frame matches the values actually
used by the desk.

## Acceptance criteria

1. Plan 00A remains pending until implementation and verification are complete; this file has no
   `Result` section while it is only a plan.
2. After **Stage & Venue**, all other Patch layers are created in about five seconds of retained
   video.
3. Front Truss Left and Profile Stage Center remain readable first examples; Front Truss Right and
   Profile Stage Left/Right are fast-forwarded.
4. The four House Lights remain independently addressed, with additional address entry visibly
   typed faster.
5. The first ACL set and the Stage Profile batch are visibly placed before the faster final Patch
   fast-forward begins.
6. The final Patch fast-forward retains visible layer context while running faster than the current
   recording.
7. First-Group Fixture Sheet selection is 2x its current pace and still visibly selects every
   fixture in fixture-ID order.
8. The interval after completing the second Group and before beginning Beam Show selection is 4x
   its current pace.
9. The icon modal receives readable holds, visibly switches to **Fixture type**, selects **Profile
   Moving Light**, and shows the saved icon on the Group tile.
10. The final Stage visibly responds to real non-default Pan and Tilt values; stored/API-only values
    do not satisfy this check.
11. The concluding illuminated Stage includes at least two visibly distinct programmed colors and
    is not an all-white look.
12. The canonical demo show inventory, fixture identities, patch, multipatches, and output contract
    remain unchanged unless a separately approved product correction requires a migration.

## Required verification when implemented

- Run the focused product-demo scenario through the repository's documented `./test demo` path and
  retain the deterministic edit timeline.
- Inspect the generated video at every speed-change boundary and record the resulting source and
  target timecodes or frame ranges.
- Inspect the icon modal and saved Group tile in the retained video, not only in a DOM assertion.
- Verify Pan/Tilt and RGB values in authoritative desk/visualization state, then visually inspect
  the corresponding Stage movement, beam direction, and colored final frame.
- Re-run the canonical demo-show generation/inventory checks and confirm no fixture, layer,
  address, physical-instance, Group, preset, Dynamic, Cuelist, Playback, or output-route totals
  drifted unintentionally.
