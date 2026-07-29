# Product Demo Video Revision

## Status

**Implemented and reconciled — 2026-07-29.** Plan 20 of the refactoring queue implements this revision
through the maintained `DEMO-001` scenario. The exact lighting-only inventory in
[`76-separate-demo-and-benchmark-shows.md`](76-separate-demo-and-benchmark-shows.md) supersedes
the older scenery-specific patch, curtain, and ACL-location demonstrations below where they
cannot coexist with the required 262 controllable fixtures and 301 physical Stage instances.

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

Begin from a genuinely empty show and visibly open Show Patch. Then cross one explicit
fast-forward boundary that generates the canonical Plan 76 lighting venue: seven layers, the exact
262 controllable fixtures and 301 physical instances, deterministic Stage placement, all groups,
presets, Cuelists, playbacks, Dynamics, Speed Groups, and desktop layouts.

After the boundary, prove the result in the normal operator surfaces and continue using normal
keypad, pool, fixture-control, playback, and preload paths. The maintained demo must not introduce
scenery fixtures or a second demo-only patch because doing so would invalidate the release workload
that the recording is required to represent.

## ACL multipatch positioning workflow

The canonical generator creates four named eight-lamp ACL controls with reviewed deterministic
multi-patch placement, including the approved Front Split arrangement of four lamps left and four
right fanning inward. The old visible encoder-modal start-through-end location sequence is
superseded for this maintained recording: replaying it would require a provisional noncanonical
patch and duplicate setup path. Encoder through-expression behavior remains independently testable
outside `DEMO-001`.

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

1. The video starts with one title card and then immediately enters show setup.
2. Redundant show setup subsection title cards are gone.
3. A genuinely empty show and Show Patch are visible before one explicit canonical generation boundary.
4. The generated lighting venue contains exactly 262 controllable fixtures and 301 physical Stage
   instances; it contains no demo-only scenery filler.
5. All four reviewed eight-lamp ACL multi-patches, including Front Split, come from the canonical
   generator and remain shared with the release workload.
6. The visible show setup save step is absent.
7. Output route work is introduced by an **Output Configuration** title card.
8. Group and preset pools are empty before canonical generation and show only canonical objects
   afterward.
9. Demo-critical dialogs are centered in the complete recorded desk surface.
10. Built-in fixture control actions replace the old "Turn lights on" chapter and explain Lamp On,
    Fan Auto, Reset, Lamp Off, and related actions.
11. Red preset recall does not perform redundant zeroing of unchanged color channels.
12. Current-action narration describes operator intent and outcome, not literal clicks.
13. Fixture Sheet, canonical presets, and the final Cuelist topology are each proved through their
    production built-in surfaces.
14. Busking activates the canonical benchmark look and produces live DMX output.
15. Preload is exercised through the physical Preload Go path and finishes with an empty queue.
16. `./test demo` still produces the maintained product demo video and remains an executable
    regression test, not only a scripted recording.

## Related follow-up risks

The new-show checks distinguish genuinely empty state from the canonical generated state. The
recording asserts that its fixture-control dialog is centered against the complete recorded surface.
The older encoder-modal through-syntax concern is no longer a dependency of this maintained demo.

## Result

`DEMO-001` now starts from an empty show, crosses one explicit canonical-generation boundary, and
continues through production Show Patch, output routing, Fixture Sheet, fixture-control, preset,
Cuelist, playback, preload, Programmer, Stage, and live DMX paths. The maintained recording passed
with `LIGHT_VISUAL_RECORDING=1 LIGHT_UPDATE_DEMO_SHOW=1 npm run test:demo`.

Generated visual evidence:

- `.artifacts/test/visual-inspection/product-demo/tosklight-product-demo-h265.mp4` — 1920×1080,
  25 fps, 162.84 seconds;
- `.artifacts/test/visual-inspection/product-demo/tosklight-product-demo-1920x1080.png`; and
- `.artifacts/test/visual-inspection/product-demo/tosklight-product-demo-contact-sheet.png`.

The run refreshed `assets/demo.show`; its SQLite integrity check passes and it contains the exact
262 patched fixtures, 38 Groups, 30 presets, 30 Dynamics, seven Cuelists, and 13 Playbacks. The
fixture-control dialog has executable centering coverage against the complete recorded desk
surface. The royalty-free Theater script and script-specific Theater Cues remain pending exactly
as allowed by Plan 76.
