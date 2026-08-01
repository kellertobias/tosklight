# Product Demo Video Revision

## Status

**Finished and reverified — 2026-08-01.** The maintained `DEMO-001` scenario, deterministic edit
timeline, canonical video, and exported starter show implement this revision. The release workload
distinguishes its 262 controllable lighting fixtures from 33 visual-only Venue records, so the
requested scenery and visible Patch workflow remain part of one canonical show without changing
its DMX workload.

## Goal

Revise the single maintained product demo so the recording reads like a polished desk walkthrough and remains a real regression test. The video should show the operator intent and the visible desk outcome, not spend time on redundant title cards, saved-show ceremony, or interactions whose purpose is unclear.

The implementation target is the existing single demo test in `tests/product-demo.spec.ts` and the `./test demo` output described by `docs/testing/README.md`.

## Video structure

The demo should begin with one title card before any show setup interaction. After that card, it should directly start show setup.

Remove the dedicated section title cards that currently interrupt the setup sequence, including:

- Show Setup stage title card;
- Show Setup ACL title card;
- Front title card; and
- other fixture-family or section title cards that only announce the next part of the same show setup flow.

Keep title cards only where they introduce a real new chapter in the demo. The remaining chapter cards should include:

- the initial product/demo title card;
- **Output Configuration**, after patch/setup work and before route configuration;
- **Group Preparation**;
- **Built-in Fixture Control Actions** or a similarly clear title for the lamp/control workflow;
- **Preset Programming**;
- **Cue Programming**; and
- **Busking** or later playback-operation chapters as needed.

Remove the show setup save step from the visible demo. The scenario may still persist or prepare state internally if the regression test needs it, but the recording should not spend time showing a separate "save the show" action.

## Show setup sequence

Create and verify a genuinely empty show before creating the page whose video is retained. Open
Show Patch and visibly add all three four-segment trusses through the Touch UI. Keep the first
segment interaction at normal recording pace, then reduce the gaps between repeated actions.
Fast-forward the stage deck, curtains, back and side railings, and vertical pipes through the API.
Visibly patch and place all eight front Fresnels through the UI, add one ACL control and its seven
multi-patches through the UI, and add one moving Profile through the UI. One final fast-forward
completes the remaining canonical lighting patch and prepared show objects.

After the boundary, prove the result in the normal operator surfaces and continue using normal
keypad, pool, fixture-control, playback, and preload paths. The maintained demo must not introduce
scenery fixtures or a second demo-only patch because doing so would invalidate the release workload
that the recording is required to represent.

## ACL multipatch positioning workflow

The first canonical ACL set is visibly created with seven multi-patches. The final API boundary
adopts those UI-created identities and applies their reviewed deterministic fan placement; the
other three ACL sets are completed by the same canonical generator.

## Output configuration

After patch and physical setup, show an **Output Configuration** title card.

Then configure the output routes. Keep this section concise and operator-facing: the viewer should understand that universes are being mapped and output is being enabled, without replaying unnecessary setup details.

## Group preparation issues to verify

Group preparation can keep its current broad flow, but the revised demo must address two visible problems:

1. Groups appear to already exist when the demo enters group preparation, even though this should be a new show with no prior group programming. The implementation must determine whether this is seeded state, leaked demo state, stale UI, or a real persistence/test setup bug. The recording should enter group preparation with group pools in the correct new-show state.
2. Some modals are visibly off-center. The record modal has appeared at the left side of the screen. Demo-critical modals must open centered in the visible application surface unless a specific workflow intentionally anchors them elsewhere.

If the modal-centering defect is fixed before this plan is implemented, the demo revision should still include a regression check or visual assertion path that keeps the record modal centered.

## Built-in fixture control actions

Replace the current "Turn lights on" framing. The title should be **Built-in Fixture Control
Actions** or a similarly clear operator label, not "Turn lights on". These actions are fixture
profile controls, not operator-authored [Macros](../Later/46-macros-and-scheduled-macros.md).

The supporting text should describe the purpose, for example:

> Directly run fixture control actions such as lamp on, fan auto, reset, and lamp off across the selected fixtures or the whole show.

In the visible workflow:

- clear the selection if the intent is to apply the action globally;
- open Programmer control and the special/control action dialog;
- highlight **Lamp On** and run it;
- briefly highlight other available actions such as **Fan Auto**, **Reset**, and **Lamp Off**; and
- update the current-action narration to describe what each highlighted action would do.

The section should demonstrate that these are fixture control actions available from the desk, not
just one lamp-on button.

## Preset programming issues to verify

Preset programming should not begin with presets that already appear to exist in a new show. The implementation must determine whether this comes from seeded demo data, leaked prior state, stale pool rendering, or an actual persistence/test isolation issue.

When recording the red color preset, set Red to 100%. Do not also record unnecessary Green and Blue 0% operations if those channels are already at zero. Activating one color attribute for an RGB/RGBW fixture should activate the color attribute family for that fixture, so the demo should not imply the operator must manually zero unchanged sibling color channels.

Current-action narration should explain the purpose of each step, not echo a literal click. For example, avoid text like "Click 1 diamond Red Color" and use action text that explains that the operator is storing or updating the red color preset for later recall.

## Cue programming revision

Cue programming is currently too short in the recording. The viewer sees the **Cue Programming** title and then nearly immediately sees **Busking**, without enough visible cue creation.

The revised cue chapter must show the final canonical programming topology in production surfaces,
not fabricate a second transient set of cues for the recording. The preceding chapters use Fixture
Sheet and the Color preset pool; Cue Programming then opens Cuelists and proves the seven-Cuelist,
13-playback result, including `Start` and the four-Cue `ACL Chase`. Narration explains the design
goal and result rather than echoing clicks.

## Acceptance checks

The implementation is complete only when the maintained demo video and test satisfy all of the following:

1. The video first shows the active empty show, then one title card, and immediately enters setup.
2. Redundant show setup subsection title cards are gone.
3. The saved video page is created only after a genuinely empty show is active, and Show Patch is
   the first operator setup surface.
4. All truss segments are visibly added through Touch UI; only the first uses normal action gaps.
5. Stage elements, curtains, back and side railings, and vertical pipes use one labelled API
   fast-forward boundary.
6. Eight Fresnels are patched and placed visibly through UI; one ACL set with seven multi-patches
   and one moving Profile are patched visibly through UI.
7. The complete venue contains 262 controllable lights, 33 visual-only Venue records, 295 total
   patch records, and 343 physical Stage instances.
8. All four reviewed eight-lamp ACL multi-patches, including Front Split, come from the canonical
   generator and remain shared with the release workload.
9. The visible show setup save step is absent.
10. Output route work is introduced by an **Output Configuration** title card.
11. Group and preset pools are empty before canonical generation and show only canonical objects
   afterward.
12. Demo-critical dialogs are centered in the complete recorded desk surface.
13. Built-in fixture control actions replace the old "Turn lights on" chapter and explain Lamp On,
    Fan Auto, Reset, Lamp Off, and related actions.
14. Red preset recall does not perform redundant zeroing of unchanged color channels.
15. Current-action narration describes operator intent and outcome, not literal clicks.
16. Fixture Sheet, canonical presets, and the final Cuelist topology are each proved through their
    production built-in surfaces.
17. Busking activates the canonical benchmark look and produces live DMX output.
18. Preload is exercised through the physical Preload Go path and finishes with an empty queue.
19. `./test demo` still produces the maintained product demo video and remains an executable
    regression test, not only a scripted recording.

## Related follow-up risks

The new-show checks distinguish genuinely empty state from the canonical generated state. The
recording asserts that its fixture-control dialog is centered against the complete recorded surface.
The older encoder-modal through-syntax concern is no longer a dependency of this maintained demo.

## Result

`DEMO-001` creates its retained recording page only after the empty show is active, performs the
requested visible Touch UI Patch sequence with explicit progressive fast-forward boundaries, and
continues through production Show Patch, output routing, Fixture Sheet, Group, preset, Dynamic,
Cuelist, playback, preload, Programmer, Stage, and live DMX paths. The JSON-shaped
`PRODUCT_DEMO_SCRIPT` at the top of the scenario owns exact 25 fps chapter lengths, the 15-frame
crossfade, action pacing, all 14 layers, universe-one address bands, THRU placement strings, curtain
height, and moving-fixture rotation. Those Patch values now drive both the visible value pads and
the canonical generated starter show.

Generated visual evidence:

- `.artifacts/test/visual-inspection/product-demo/tosklight-product-demo-h265.mp4` — 1920×1080,
  HEVC, 25 fps, exactly 18,510 frames / 740.4 seconds;
- `.artifacts/test/visual-inspection/product-demo/tosklight-product-demo.webm` — 1920×1080, VP9,
  25 fps, exactly 18,510 frames / 740.4 seconds; and
- `.artifacts/test/visual-inspection/product-demo/product-demo-edit-timeline.json` — canonical
  source markers, target frames, and chapter timecodes for voice-over editing.

The final non-recording export refreshed `assets/demo.show`; its SQLite integrity check passes and
it contains exactly 295 Patch records (262 controllable and 33 visual-only), 343 physical Stage
instances, 14 layers, 35 Groups, 30 presets, 30 Dynamics, seven Cuelists, 13 Playbacks, and eight
output routes. The saved user layout is exactly Group Programming, Busking, Programming, and
Theater; Group Programming uses a 16/24 Fixture Sheet plus an 8/24 seven-column Group Pool, and
Busking uses a 10×5 Virtual Playback pane. Export verification also proves all eight Fresnels and
the seven visibly patched Profile movers retain their canonical non-origin positions.

Verification after the final reconciliation changes:

- `LIGHT_UPDATE_DEMO_SHOW=1 npm run test:e2e -- tests/product-demo.spec.ts --workers=1` — passed
  in 2.5 minutes without recording or transcoding;
- `npm run test:e2e -- tests/76-demo-show-generation.spec.ts --workers=1` — passed;
- focused Patch, Group, layout, Dynamic, pool-preference, reducer, and Stage tests — passed;
- desktop TypeScript and bench type checks — passed; and
- the recorded Playwright run used for the canonical video passed in 14.4 minutes before the final
  deterministic encode.

The top-left modal constraint, full-height fixture search, direct numeric value pad, live Stage
readiness, progressive scenery/lighting appearance, globally owned pool palette, and pane-local
pool layout/mode settings all have implementation or executable regression coverage. Intermediate
choreography changes use the non-recording regression; the maintained video is generated only for
the final deliverable.
