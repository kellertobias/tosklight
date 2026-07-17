# Update with Shift+Record

## Status and scope

**Implementation status: Complete.** **Update** is implemented as a keyword and storage workflow for applying the current programmer contents to an existing recordable object. Its shortcut is `[SHIFT] [REC]`. The four Cue modes, Preset and ordered Group modes, target preview and context validation, touch/Enter paths, Update Update, desk-scoped settings, software/OSC/hardware gestures and feedback, atomic undo behavior, help, and focused/paired acceptance coverage are implemented.

Update is closely related to Record: anything that can be recorded can also be an Update target. It must reuse each target's normal persisted storage model and revision checks rather than create a separate kind of update data. Unlike Record, Update normally asks how the programmer contents should be applied to the existing target.

## Core example

Assume Cuelist 1 contains two Cues for moving lights 1 through 4:

1. Cue 1 stores Position and Intensity.
2. Cue 2 changes Position and tracks the Intensity from Cue 1.
3. Cue 2 is currently active on its playback.
4. The operator gives the same four fixtures a Color and a different Intensity in the programmer.

The operator presses `[SHIFT] [REC]` to enter **Update**, then identifies the Cuelist or its assigned playback. The result depends on the selected Cue update mode:

- **Existing Only** updates eligible fixture/attribute addresses at the Cue events from which the active Cue currently tracks them. In this example, Intensity is updated at its tracked source in Cue 1. Color is ignored because that fixture/attribute address does not yet exist anywhere in the Cuelist.
- **Existing in Current Cue** updates only eligible fixture/attribute addresses explicitly stored in Cue 2. The tracked Intensity from Cue 1 is not pulled into Cue 2, and Color is not added.
- **Add to Current Cue** writes eligible addresses into Cue 2 when those same fixture/attribute addresses already exist somewhere in the Cuelist. In this example, the new Intensity is stored in Cue 2 instead of changing Cue 1. Color is ignored because it is new to the Cuelist. This is the default Cue update mode.
- **Add New** merges all applicable programmer addresses into Cue 2, including the new Color. For the current Cue this is equivalent in storage effect to Record Merge.

The update calculation must use the authoritative current Cue and tracking state. It must not infer the source merely from what is visible in a client, and it must not modify an unrelated earlier occurrence of an address when a later Cue event is the source of the value currently being tracked.

## Entering Update and choosing a target

Pressing `[SHIFT] [REC]` once invokes the **Update** keyword. The command line and every attached control surface must show the same armed Update state.

After entering Update, the operator may identify an existing recordable target through the same target surfaces used by Record, including:

- touching a Cuelist in its pool;
- touching a playback assigned to a Cuelist;
- touching a Preset, Group, or another supported recordable object; or
- entering the normal playback-address syntax, such as `[UPDATE] [SET] <playback-number> [ENTER]` or its explicit page form.

For a Cuelist or Cuelist playback, an Update without an explicit Cue targets the currently active Cue on that concrete playback. Current-page and explicit-page playback addressing must remain distinct. If a pool Cuelist is not represented by one unambiguous active playback/Cue, the desk must require an explicit Cue or playback context rather than guessing.

Target confirmation has two paths:

- **Enter confirmation:** completing a command-line target with `[ENTER]` applies the configured default for that target type directly. It does not open the per-operation Update modal.
- **Touch confirmation:** touching the target opens the per-operation Update modal and asks how to update it, unless the operator has disabled that modal in Update Settings. When the modal is disabled, touch confirmation also applies the configured default directly.

No target is changed merely by entering Update or opening a modal. Cancel closes the modal, disarms the pending operation, and changes no show data. Invalid, empty, ambiguous, or stale targets fail atomically with an actionable explanation.

## Cue and Cuelist update modes

The per-operation modal for a Cue/Cuelist target offers exactly these modes:

| Mode | Eligible programmer addresses | Storage location |
| --- | --- | --- |
| **Existing Only** | Exact fixture/attribute addresses already present somewhere in the Cuelist and supplying the active Cue through an explicit value or tracking | Replace each active value at its authoritative source Cue event |
| **Existing in Current Cue** | Exact fixture/attribute addresses explicitly stored in the current Cue | Replace those values in the current Cue only |
| **Add to Current Cue** | Exact fixture/attribute addresses that exist somewhere in the Cuelist | Merge those values into the current Cue; do not add addresses new to the Cuelist |
| **Add New** | All applicable programmer fixture/attribute addresses | Merge them into the current Cue, including addresses new to the Cuelist |

**Add to Current Cue** is the initial default. The operator can choose another default in Update Settings.

Eligibility is per exact fixture/attribute address, not merely per broad feature family. For example, Color existing for fixture 1 does not by itself make Color eligible for fixture 4 under a mode that excludes new addresses.

Update consumes only actual programmer changes eligible for the chosen target and mode. Resolved playback output, Highlight output, defaults, and unchanged tracked output must not be written merely because they are visible on stage. The final implementation must follow the programmer's established value/reference and LTP semantics and must define whether successfully updated programmer values are cleared or retained consistently with the desk's Record workflow.

## Presets, Groups, and other recordable targets

Non-Cue recordable targets use the same distinction between updating stored content and adding new content, adapted to the unit stored by that object.

For a Preset, the modal offers:

- **Update Existing**, which updates only exact fixture/attribute addresses already stored in that Preset; and
- **Add New**, which also adds applicable fixture/attribute addresses that are not yet stored in the Preset.

For example, if a Color Preset contains Color for fixtures 1 and 2 and the programmer contains a new Color for fixtures 1 through 4, **Update Existing** changes only fixtures 1 and 2. **Add New** changes fixtures 1 and 2 and adds the Color values for fixtures 3 and 4.

Groups and every other recordable target follow the same boundary using their own stored unit. For a Group, that unit is ordered fixture membership rather than fixture attributes: an existing-only operation must not introduce a fixture that is not already a member, while add-new may add selected fixtures according to the normal ordered Group Merge rules. Update must not silently invent removal, reordering, overwrite, or dereference behavior that the corresponding Record operation does not provide.

Each target family may present only the modes that are meaningful for its storage model, but every mode must make the existing-versus-new boundary explicit. A successful update uses the target object's normal persistence, ordering, reference, compatibility, and revision rules.

## Update Update: eligible-target menu

Pressing `[SHIFT]`, pressing `[REC]`, keeping `[SHIFT]` held, and pressing `[REC]` a second time invokes **Update Update**. This opens a target-selection menu rather than waiting for one target to be entered.

The menu can list currently active or currently referenced Presets, Groups, Cuelists, and playbacks that can be related to the current programmer changes. A Cuelist shown through more than one active playback must retain its concrete playback/Cue context; the menu must not collapse distinct current Cues into an ambiguous target.

The default filter is **Eligible for Update Existing**. It shows only targets for which an Update Existing operation would make a real change from the current programmer contents. Eligibility means that the target contains at least one exact stored unit corresponding to a current programmer change; merely being active is not sufficient.

The operator can switch the filter to **Show All Active**. This includes active targets even when they contain no currently eligible existing value. In this view the menu provides an update-mode control, including at least **Update Existing** and **Add New**, so the operator can deliberately choose whether new stored content may be added.

Selecting a target from this menu immediately performs the mode shown for that target. Targets that would be no-ops remain visibly distinguishable and must not report success as though data changed. The menu must make target type, number/name, concrete playback and current Cue where applicable, eligible value count, and chosen mode clear enough to prevent updating the wrong stored object.

## Update Settings: hold Shift+Record

Pressing `[SHIFT] [REC]` and continuing to hold the chord for the defined long-press interval opens **Update Settings**. This is a configuration menu comparable to Record Settings; it does not perform an update.

Update Settings configures:

- the default Cue/Cuelist update mode: **Existing Only**, **Existing in Current Cue**, **Add to Current Cue**, or **Add New**;
- the default existing-versus-add-new mode for Presets, Groups, and each other supported recordable target family; and
- **Show Update modal on touch**, enabled by default.

When **Show Update modal on touch** is disabled, selecting a target by touch applies that target family's configured default immediately. Command-line completion with `[ENTER]` always uses the configured default directly regardless of this setting. Because ordinary Update no longer shows the modal while this option is disabled, the operator re-enables it by opening Update Settings again with the long-press gesture.

The settings UI must clearly separate show-persisted programming data from operator/desk workflow preferences. The implementation must decide the established settings scope to reuse, migrate old settings with deterministic defaults, and keep the defaults synchronized across the software surface and attached hardware belonging to the same desk.

The long-press must not also fire the normal single Update action on press or release. Likewise, the second `[REC]` used for Update Update must be resolved distinctly from the long-press gesture and must not produce two updates.

## Feedback, safety, and future acceptance coverage

Update changes existing programming and therefore needs explicit feedback before and after the operation:

- the armed command line shows **UPDATE** and the addressed target;
- each modal shows the target identity, current Cue where applicable, chosen mode, and what will be eligible, ignored, changed at its source, or added to the current Cue;
- successful completion reports which object and Cue/source events changed and whether any programmer values were ineligible; and
- a no-op, stale revision, missing target, missing current Cue, or ambiguous playback context reports the reason and changes nothing.

The implementation must update a target atomically under the normal show revision mechanism. Undo/revision behavior must treat one confirmed Update as one operator action even when Existing Only changes source events in several Cues.

Before this feature is marked complete, add operator help and executable acceptance coverage for at least:

1. The four Cue-update modes in the example above, proving the difference between changing a tracked source and writing into the current Cue.
2. Exact per-fixture/per-attribute eligibility, including new Color values being excluded or added according to mode.
3. Preset Update Existing versus Add New with fixtures 1 through 4.
4. Group ordered-membership behavior without implicit removal or reordering.
5. Touch targets opening the modal, `[ENTER]` using defaults directly, and the modal-disable setting.
6. Current-page, explicit-page, pool-Cuelist, and ambiguous multi-playback target handling.
7. Update Update's eligible-only and show-all-active filters, mode control, target identity, and no-op handling.
8. The single-press, double-press-while-Shift-is-held, and long-press gestures being mutually exclusive on software, keyboard, OSC, and attached hardware paths.
9. Atomic revision behavior, cancellation, invalid targets, and one-step undo/history behavior.
10. Old-show and old-settings migration plus shared same-desk feedback and different-desk isolation.
