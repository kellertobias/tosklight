# Product Demo Video Revision

## Status

**Specification only.** This plan records requested changes to the maintained `DEMO-001` product walkthrough and its generated video. It does not implement the Playwright scenario, runtime behavior, screenshots, or test changes.

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
- **Built-in Control Macros** or a similarly clear title for the lamp/control macro workflow;
- **Preset Programming**;
- **Cue Programming**; and
- **Busking** or later playback-operation chapters as needed.

Remove the show setup save step from the visible demo. The scenario may still persist or prepare state internally if the regression test needs it, but the recording should not spend time showing a separate "save the show" action.

## Show setup sequence

At the beginning of show setup, patch the first stage element normally so the viewer sees the operator workflow.

After that first visible patch, fast-forward the repetitive creation of the remaining trusses, stage elements, and curtains. The fast-forward should still make the resulting stage state understandable, but it should not force viewers to watch every repeated setup action.

Curtains need to sit further downstage/backdrop-visible than they do in the current recording, and the stage likely needs more curtain instances so the curtain coverage spans the whole background of the stage. The implementation should adjust the curtain count, placement, and spacing until the Stage view visibly reads as a continuous background.

## ACL multipatch positioning workflow

Do not demonstrate fixture location spreading before the ACL multipatch exists.

Once the ACL multi-patch is created, set the relevant fixture or multipatch locations through the encoder tab:

1. Open the encoder tab for the position/location attribute.
2. Open the encoder input modal.
3. Enter a through expression with a start position and end position.
4. Apply it so the selected ACL items visibly spread from the start location through the end location.

The point of this sequence is to prove and demonstrate that encoder-modal input accepts through syntax for location spreading. The visible action text should explain the setup goal, for example placing ACL lamps across the truss or spreading ACL aim points across the stage, rather than merely describing the click target.

## Output configuration

After patch and physical setup, show an **Output Configuration** title card.

Then configure the output routes. Keep this section concise and operator-facing: the viewer should understand that universes are being mapped and output is being enabled, without replaying unnecessary setup details.

## Group preparation issues to verify

Group preparation can keep its current broad flow, but the revised demo must address two visible problems:

1. Groups appear to already exist when the demo enters group preparation, even though this should be a new show with no prior group programming. The implementation must determine whether this is seeded state, leaked demo state, stale UI, or a real persistence/test setup bug. The recording should enter group preparation with group pools in the correct new-show state.
2. Some modals are visibly off-center. The record modal has appeared at the left side of the screen. Demo-critical modals must open centered in the visible application surface unless a specific workflow intentionally anchors them elsewhere.

If the modal-centering defect is fixed before this plan is implemented, the demo revision should still include a regression check or visual assertion path that keeps the record modal centered.

## Built-in control macros

Replace the current "Turn lights on" framing. The title should be **Built-in Control Macros** or a similarly clear operator label, not "Turn lights on".

The supporting text should describe the purpose, for example:

> Directly run fixture control actions such as lamp on, fan auto, reset, and lamp off across the selected fixtures or the whole show.

In the visible workflow:

- clear the selection if the intent is to apply the action globally;
- open Programmer control and the special/control macro dialog;
- highlight **Lamp On** and run it;
- briefly highlight other available actions such as **Fan Auto**, **Reset**, and **Lamp Off**; and
- update the current-action narration to describe what each highlighted action would do.

The section should demonstrate that these are fixture control macros available from the desk, not just one lamp-on button.

## Preset programming issues to verify

Preset programming should not begin with presets that already appear to exist in a new show. The implementation must determine whether this comes from seeded demo data, leaked prior state, stale pool rendering, or an actual persistence/test isolation issue.

When recording the red color preset, set Red to 100%. Do not also record unnecessary Green and Blue 0% operations if those channels are already at zero. Activating one color attribute for an RGB/RGBW fixture should activate the color attribute family for that fixture, so the demo should not imply the operator must manually zero unchanged sibling color channels.

Current-action narration should explain the purpose of each step, not echo a literal click. For example, avoid text like "Click 1 diamond Red Color" and use action text that explains that the operator is storing or updating the red color preset for later recall.

## Cue programming revision

Cue programming is currently too short in the recording. The viewer sees the **Cue Programming** title and then nearly immediately sees **Busking**, without enough visible cue creation.

The revised cue programming chapter must show meaningful programming work and its result. It should run on a desktop layout designed for this chapter, not on a generic layout that hides the outcome.

Configure the desktop before cue programming, preferably during show setup, so that the cue workflow has these panes visible:

- Fixture Sheet;
- preset pools, with color presets at the top and position presets below; and
- a cue list pane in the bottom-right that shows the active cue list.

The cue list pane should make the active cue list and newly recorded cues visible while the demo records or updates them. The point is for the recording to show the operator building the cue list, not only the final playback behavior.

Cue programming narration should explain the design goal of each look or cue, for example building the main stage look, adding the second cue, storing color playbacks, or configuring the ACL chaser.

## Acceptance checks

The implementation is complete only when the maintained demo video and test satisfy all of the following:

1. The video starts with one title card and then immediately enters show setup.
2. Redundant show setup subsection title cards are gone.
3. The first stage element patch is visible, then repetitive truss, stage, and curtain setup is fast-forwarded.
4. Curtains are placed and duplicated so they visibly cover the full stage background.
5. ACL multipatch location spreading is demonstrated after ACL multipatch creation through the encoder input modal using start-through-end syntax.
6. The visible show setup save step is absent.
7. Output route work is introduced by an **Output Configuration** title card.
8. Group pools are correct for a new show when group preparation starts.
9. Record and related modals are centered in the visible app surface.
10. Built-in control macros replace the old "Turn lights on" chapter and explain Lamp On, Fan Auto, Reset, Lamp Off, and related actions.
11. Preset pools are correct for a new show when preset programming starts.
12. Red preset programming does not perform redundant zeroing of unchanged color channels.
13. Current-action narration describes operator intent and outcome, not literal clicks.
14. Cue programming lasts long enough to show actual cue creation and visible cue-list updates.
15. The cue programming desktop shows Fixture Sheet, color and position preset pools, and the active cue list pane.
16. `./test demo` still produces the maintained product demo video and remains an executable regression test, not only a scripted recording.

## Related follow-up risks

This plan intentionally does not decide whether the observed pre-existing groups, pre-existing presets, or off-center modals are demo-script defects or application defects. The implementation must investigate them on the current code path and fix the correct layer.

If the encoder-modal through syntax is not already supported for fixture location attributes, that support needs its own implementation and focused test coverage before the demo can rely on it.
