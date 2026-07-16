# Highlight and Step Through

## Status

This is a planned feature. Its runtime behavior, fixture-level highlight configuration, UI placement, OSC contract, and hardware mapping must be designed together before implementation or acceptance coverage is added.

## Operator intent

Highlight provides a temporary way to identify and work through fixtures without writing the identifying look into the programmer. The operator can select fixtures from the Fixture Sheet, Stage Sheet, or Groups, turn Highlight on, and see the selected fixture or fixtures turn on in their configured highlight look. The normal default is full intensity and white, but an individual fixture may instead be configured to use another useful color such as blue.

Highlight is especially useful while focusing or positioning several fixtures. After capturing a selection, the operator can step to the next fixture, position or otherwise program it normally, then continue to the next fixture without rebuilding the original selection.

## Highlight output is not programmer data

Highlight is a transient output layer, not a programmer value, preset, Cue value, or selection macro.

- Turning Highlight on must not add Intensity, Color, Beam, Position, or other values to the programmer.
- Highlight values must never be included by Record, Update, Merge, presets, Cues, or any other store operation.
- Normal values deliberately changed while Highlight is active still belong to the programmer and remain after Highlight is turned off.
- Turning Highlight off removes only the temporary highlight contribution and immediately reveals the correct underlying programmer/playback output.
- Clearing or releasing the programmer must not be required to remove Highlight.
- Highlight state is runtime operator state and must not be written into show programming or restored as live output merely because a show is saved and reopened.

The implementation therefore needs an explicit highlight contribution or diagnostic output layer. It must not simulate Highlight by silently putting values into the programmer and later trying to remove them.

## Fixture-level highlight look

Each fixture needs a persisted Highlight Look setting that describes how that fixture identifies itself. This belongs to the fixture, independently of its DMX address or other patch-address details. A fixture-type/profile default may be useful, but an operator must be able to override the look for an individual fixture.

The default semantic look is:

- Intensity/Dimmer at full;
- white on fixtures that can produce color; and
- no unnecessary change to Position or other attributes.

The configuration must also support a different identifying color, such as blue. Planning must define how this semantic color is represented across RGB, RGBW, CMY, color-wheel, conventional dimmer, and other fixture capabilities. Some fixtures may also require an open shutter or another fixture-specific value before they emit light; the Highlight Look must be capable of describing those required values without turning them into programmer data.

Old shows without a Highlight Look must receive a safe, deterministic default. Copying or duplicating a fixture should preserve its configured look. Before implementation, decide whether this setting is stored directly on the show fixture, inherited from the fixture definition with a per-fixture override, or represented by another fixture-owned model. It must not be coupled to universe/address assignment.

## Remembered selection and stepping

Highlight needs two separate pieces of transient state:

1. the complete ordered **step selection** captured when Highlight/stepping begins; and
2. the **active step fixture** currently being identified and edited.

When Highlight is first enabled, all fixtures in the current selection may show their Highlight Look so the operator can confirm the selection. Starting Step Through focuses the first fixture in the remembered order and removes the highlight contribution from the other members. **Next** advances to the next valid fixture; **Previous** should be available for correcting an accidental advance. Whether stepping wraps at the ends or stops there remains a product decision and must be visible in feedback.

The complete step selection must survive ordinary programming work. In particular:

- changing Position, Color, Beam, or other programmer values for the active fixture must not replace or shorten the remembered step selection;
- the active fixture becomes the current working selection so normal encoders and special dialogs operate on it, while the separate remembered step selection remains intact;
- moving to Next or Previous changes the working fixture while retaining the original ordered step selection;
- an explicit command to capture a new selection replaces the remembered step selection deliberately, rather than this happening as a side effect of programming; and
- fixtures removed from the show or otherwise no longer addressable are skipped safely.

The order should follow the desk's authoritative ordered selection, including group order where that is defined, rather than sorting again implicitly. Planning must settle how multipatch fixtures, compound/logical heads, overlapping groups, duplicate selection members, unpatched fixtures, and fixtures with no usable intensity output participate.

Turning Highlight off should normally retain no active highlight output. Whether the remembered step selection remains available for a quick resume, and for how long, must be decided explicitly; it must never resume output unexpectedly after a show load or operator/session change.

## Surfaces and controls

The feature must be reachable while working in at least:

- the Fixture Sheet;
- the Stage Sheet; and
- Groups or a selection created from a Group.

The exact software placement is intentionally open. Options to evaluate include always-visible controls near the programmer/command line, contextual controls in the Fixture and Stage sheets, or a small Highlight/Step tool that can be placed in the configurable window system. The controls must not consume so much space that they impair fixture or stage work, and their state must remain obvious when the initiating sheet is no longer visible.

The minimum operator actions are:

- Highlight On/Off or Toggle;
- capture or replace the current step selection;
- Next fixture;
- Previous fixture; and
- exit/release Highlight without clearing programmer values.

Software buttons, physical controls, keyboard bindings, and OSC must all invoke the same authoritative actions rather than maintaining separate client-side stepping state. Button labels and feedback should make clear whether all selected fixtures or one stepped fixture is currently highlighted.

## OSC and hardware control

Highlight and stepping require OSC input so a physical controller can provide dedicated buttons. The OSC surface must support at least Highlight On, Highlight Off/Toggle, Next, Previous, and capturing/resetting the step selection. It should also expose feedback for:

- whether Highlight is active;
- the active step index and total number of valid fixtures;
- the active fixture identity/number and name; and
- whether Next or Previous is available when end behavior does not wrap.

The OSC contract must use the same user/session state as the software controls. Repeated messages, reconnects, and simultaneous software/hardware operation must not advance twice or cause the clients to disagree about the active fixture.

## Runtime ownership and safety decisions

Highlight belongs to the operator/session rather than to a playback. If the same user is connected from multiple devices, every device should see the same Highlight and step state; a different user must not silently take over that state.

Before implementation, define the highlight layer's priority and safety interaction with Grand Master, Blackout, parked values, playback priority, programmer priority, output-disabled modes, Blind/Preview, and fixture limits. The result must be deliberate and documented: Highlight needs to be useful for identifying fixtures, but must not accidentally bypass a safety control or create unexpected live output.

Also define whether Highlight changes snap immediately or use a short configured transition, what happens when the active fixture is already controlled by a higher-priority source, and whether Blind/Preview can prepare or inspect a step selection without emitting Highlight output.

## Required implementation contract and tests

Before enabling the feature, add executable coverage for at least:

1. Highlighting a multi-fixture selection from the Fixture Sheet, Stage Sheet, and a Group.
2. Default full/white behavior and an individual fixture configured to identify in another color.
3. Highlight output never appearing in programmer state or any recorded object.
4. Normal Position changes remaining in the programmer while Highlight is stepped through several fixtures.
5. Next and Previous retaining the complete original ordered selection after each programming change.
6. Removal or invalidation of a fixture during stepping without corrupting the remaining sequence.
7. Turning Highlight off revealing the correct underlying programmer/playback values without a transient default frame.
8. OSC and software controls sharing authoritative state and feedback without double advances.
9. User/session isolation, reconnect behavior, and save/reload behavior.
10. The chosen Grand Master, Blackout, Blind/Preview, priority, and safety semantics.

The final implementation change must update operator help, OSC documentation, fixture/show schema migration notes, and hardware-control mappings together.
