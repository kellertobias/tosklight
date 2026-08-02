# Product Demo Pacing, Group Icon, and Final Stage Look

## Status

**Done.** Claimed and completed on 2026-08-02. The maintained product-demo workflow now carries the
reviewed pacing, portable Group icon, and retained colored Stage proof.

Implement this plan against the single maintained `DEMO-001` workflow in
`tests/bench/show/productDemoScenario.ts`. Preserve that workflow as a real regression test and
keep its deterministic 25 fps edit timeline. Video acceleration changes below refer to retained
video time and visible click/type pacing, not to weakening assertions or racing the application.

## Progress

- [x] Claimed from `docs/plans/Next` after completing plan 00.
- [x] Audit the maintained DEMO-001 workflow, deterministic edit timeline, current generated-video
  path, and the previously observed Show Patch navigation blocker.
- [x] Implement the exact Patch, Group-selection, and icon-picker pacing contract.
- [x] Trace and correct the authoritative Pan/Tilt and color path into the final Stage rendering.
- [x] Run focused checks, canonical demo inventory verification, retained-video review, major suites,
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
- The concluding position is canonical Position 3.4 **Fan Out**, whose fixture-specific values span
  Pan and alternate Tilt across the moving-light targets. Stage consumes those semantic
  visualization axes directly; profile output is separately required and checked for calibrated
  `color_xyz`. JavaScript expectations use a narrow tolerance because normalized values cross the
  Rust `f32` wire.
- The already-open browser demo can retain its original default Programming pane even after the
  canonical layout object is installed. The final operator path therefore opens that desktop and,
  when necessary, visibly switches the Stage pane's accessible **Stage view** control from 2D to
  3D before the final hold. The canonical stored Programming pane remains explicitly 3D.
- The benchmark assignments used to demonstrate playback controls remain active after the final
  PRELOAD GO and can wash out the concluding programmer look. The retained final proof therefore
  stops those explicit benchmark sources through their supported physical/virtual control paths,
  explicitly zeros every projected intensity owner, then raises one blue Wash and the two
  non-uniform yellow Beam endpoints to a controlled 2% intensity. This isolates the reviewed
  canonical Color/Fan Out values without inventing a duplicate demo-only color state or retaining
  hidden logical-head/default output.
- The already-open live Stage pane also retained its stale **Improved beams** quality even though
  the installed Programming desktop specifies **Lines + beams**. Improved beams turns the eight
  largest contributors into surface SpotLights at `intensity * 500`, which kept the retained frame
  overexposed at the otherwise controlled 28% target intensity. The concluding visible settings
  path now applies both the stored 3D view and **Lines + beams** quality and asserts the live canvas
  reports that exact quality.
- Patch placement leaves non-default Stage navigation in persisted desk state, and the former low
  default camera looked through the venue deck instead of over the rig. The product now exposes a
  literal **Reset 3D view** action in both full Stage settings and compact Stage pane settings. Its
  canonical overview reuses the existing Stage-thumbnail camera, and the demo visibly invokes that
  action before asserting the live canvas camera position and target.

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
- Focused `plannedDemoLayouts`, `plannedDemoPresets`, and `productDemoTimeline` tests passed (4
  assertions across 3 files). They prove the canonical Programming Stage configuration, the
  fixture-specific Fan Out spread, and deterministic edit window.
- The final Stage-focused `npm run test:e2e -- tests/product-demo.spec.ts --workers=1` pass completed
  in 3.5 minutes. It proves every selected Beam fixture receives the stored non-uniform Pan/Tilt
  values in authoritative semantic visualization state, Wash and Beam families project distinct
  blue/yellow `color_xyz` values, and the final Programming Stage reaches live-ready state with its
  3D canvas visible. Intermediate diagnostic runs correctly exposed and then removed an invalid
  expectation that Pan/Tilt must also appear in profile output, and corrected the 3D toggle locator
  to its real `radio` semantics.
- The canonical `npm run test:demo` source capture passed in 21.6 minutes and refreshed
  `assets/demo.show`, the screenshot, raw WebM, and version-2 timeline. The first editor pass exposed
  mixed concat/crossfade timebases; normalizing every source and intermediate node to `1/25` fixed
  the graph. A focused Node graph test passed, and standalone re-encoding then produced exact
  1920x1080, 25 fps, 21,425-frame, 857.0-second WebM and H.265 outputs. VideoToolbox HEVC was
  unavailable (`-12908`), so the repository's successful `libx265` fallback was used.
- Retained review confirms the icon modal, Fixture type grid, and saved tile are readable at
  `05:26`, `05:27`, and `05:29`, and the remaining Patch-layer window is exactly
  `00:30:00`–`00:35:00`. The `14:15` retained Stage frame remains overexposed by unrelated active
  benchmark sources, however, so visual blue/yellow separation is not yet accepted despite the
  authoritative XYZ proof. Closeout remains pending while the concluding look is isolated to the
  colored targets at controlled intensity and re-recorded.
- After isolating the concluding look, the complete focused
  `npm run test:e2e -- tests/product-demo.spec.ts --workers=1` workflow passed in 3.7 minutes. The
  scenario now proves the explicit benchmark assignments are stopped and every target reaches the
  controlled final intensity before checking non-uniform Pan/Tilt, profile-calibrated blue/yellow
  XYZ values, the visible 3D Stage canvas, and the live-ready Stage state. Retained-video proof is
  still pending the canonical re-recording.
- The first canonical re-record after source isolation passed DEMO-001 in 21.7 minutes and encoded
  21,425 frames at 1920x1080, 25 fps, and 857.0 seconds to VP9 WebM and HEVC MP4. Direct inspection
  of `14:15` and `14:16` still showed white surface blowout. Source inspection and the canvas state
  identified the stale live **Improved beams** mode as the remaining cause, not color-value drift.
- A subsequent focused run exposed a pre-existing intermittent physical PRELOAD GO miss before the
  Stage step: capture mode, all 132 values, and the queued playback remained wholly armed. The demo
  now retries the visible commit only while that complete uncommitted state remains intact and
  rejects any partial ambiguity. Bench types, the 4 focused timeline/layout/preset assertions, and
  `git diff --check` passed after this hardening. The next focused DEMO-001 run passed in 3.6 minutes,
  including the exact live `lines_and_beams` canvas assertion.
- Final Stage isolation then identified all remaining profile-output intensity owners, including
  logical heads and visual-only profiles, and explicitly reduced them to the three visible proof
  fixtures. The refreshed embedded desktop bundle passed the full focused DEMO-001 scenario in 3.5
  minutes. The captured 1280×720 focused frame visibly shows the complete elevated rig, differently aimed
  Beam endpoints, saturated blue Wash volumes, and a separate yellow contribution. Desktop
  typechecking, 51 focused pane/reducer/Stage tests, the 176-test UI package, bench typechecking,
  focused timeline/layout/preset tests, the production desktop build, and `git diff --check` passed.
  Canonical retained-video re-recording remains required before accepting the Stage item.
- The final canonical `npm run test:demo` path rebuilt the embedded desktop, passed DEMO-001 in
  21.7 minutes, edited exactly 21,425 frames at 25 fps, and produced 1920×1080 VP9 WebM plus H.265
  MP4 outputs of exactly 857.0 seconds. This run used `hevc_videotoolbox` successfully. Direct
  inspection of retained `14:15` and `14:16` frames shows the complete elevated rig, visibly
  different Beam orientations, saturated blue Wash beams, and a separate yellow Beam contribution;
  the concluding Stage is no longer all white. The canonical show still contains 264 fixture
  records, matching the required inventory.
- The final version-2 timeline records every reviewed pacing boundary at 25 fps: remaining Patch
  layers are retained at `00:30:03`-`00:35:03` (125 frames); Front Truss Left at
  `01:26:24`-`01:59:18`; accelerated Front Truss Right at `01:59:18`-`02:09:12`; readable Profile
  Stage Center at `02:17:03`-`02:26:15`; accelerated Profile Stage sides at
  `02:26:15`-`02:30:10`; first-Group Fixture Sheet selection at `05:11:05`-`05:16:24`; and repeated
  Group creation at `05:38:01`-`06:27:16`. The icon modal, Fixture type grid, and saved tile markers
  are `05:26:04`, `05:27:10`, and `05:29:04` respectively.
- `npm run test:e2e-api` passed all 26 scenarios. The first full `npm run test:e2e-ui` run passed 143,
  skipped 1, and failed 13; three failures were the obsolete Stage overview-camera oracle changed by
  this plan. After aligning that shared test oracle, focused STAGE-001, STAGE-PERF-001, and
  STAGE-PERF-002 all passed, and a repeated full UI run passed 149, skipped 1, and failed 7. None of
  the repeated failures were Stage or DEMO-001: PATCH-PLACEMENT-001 lacked its expected 8-bit mode;
  BENCH-PAGE-PLAYBACK-001 received a screen-following 400; SHOW-000 and BENCH-GROUP-001 retained
  `FIXTURE` instead of the expected SET command; DMX-004 and DMX-009 did not find expected routes;
  and PRELOAD-005 timed out while comparing capture masks.
- `npm run test:unit` stopped in its architecture preflight before package unit tests because the
  existing `GridDynamicsWindow.tsx` CheckboxField lacks `stateLabel` and the tracked semantic-test
  catalog JSON/HTML is stale. Those unrelated repository gates were not rewritten in this plan.
  The current root manifest does not define `test:desktop-smoke`, so that named command was
  unavailable rather than silently substituted.
- The authoritative `npm run open` path rebuilt both macOS/Tauri bundles and opened the fresh
  ToskLight app. The app-owned log reached a healthy server bind; outside the restricted loopback
  sandbox, `/api/v2/readiness` and `/api/v2/bootstrap` both returned HTTP 200 in about 4 ms, with no
  recovery mode or active-show error. Against that live server and freshly built UI, the full Stage
  surface connected without console errors, exposed the exact **Reset 3D view** action, accepted it,
  and reported **3D** plus **Lines + beams** selected.

## Remaining work

- None.

## Result

The maintained DEMO-001 workflow now keeps representative Patch and Group operations readable while
compressing repeated work into the reviewed deterministic 25 fps windows. The first edited Group
visibly chooses **Fixture type** -> **Profile Moving Light**, persists the portable
`icon:fixture-type/profile-moving-light` identifier, and shows that icon on the saved tile.

The concluding proof now stops competing benchmark sources, clears hidden intensity owners, applies
non-uniform Fan Out Pan/Tilt values, and renders controlled blue Wash and yellow Beam contributions in
the live 3D Stage. Both full and compact Stage settings expose **Reset 3D view**, whose canonical
overview is shared with the Stage thumbnail, and the demo visibly selects **Lines + beams** before
the final hold. The canonical regenerated show retains 264 fixture records, and direct review of the
857-second retained video confirms the final Stage is colored, differently aimed, and no longer an
all-white look.

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
