# Typical Workflow Test Definitions

This topic defines representative operator workflows for future acceptance testing. It is a test plan, not an implemented test suite. Each workflow should eventually be exercised through the real control UI and server, with output checked in the Fixture Sheet, Stage view, and rendered DMX where applicable.

## Common test show

Use a new show with the following fixtures:

| Fixture IDs | Name | Capabilities |
| --- | --- | --- |
| 1–4 | Front Wash | Dimmer |
| 11–16 | Overhead Profile | Intensity, pan, tilt, color |
| 21–28 | Cyc | Intensity, RGB color |

Create these starting groups in the listed order:

| Group | Name | Members |
| --- | --- | --- |
| 1 | Front Wash | 1–4 |
| 2 | Profiles | 11–16 |
| 3 | Cyc | 21–28 |
| 4 | All Stage | 1–4, 11–16, 21–28 |

Before each workflow, clear the command line, selection, programmer, preload, and active playbacks unless the workflow says otherwise. Create or load any named presets and cues that a workflow lists as prerequisites; do not rely on accidental state left by an earlier workflow.

## Workflow 1: Retain a live group reference

**Purpose:** Prove that programming stored against a group remains connected to that group's current membership.

1. Select group 2 by reference with `[GRP] 2 [ENTER]`.
   - **Expect:** Profiles 11–16 are selected in their group order, and the selection is identified as a live group reference.
2. Set intensity to 60% and choose a visible blue color.
   - **Expect:** All six profiles show the programmed intensity and color; unrelated fixtures are unchanged.
3. Record the look as cue 1 on an empty playback.
   - **Expect:** The cue stores group-relative changes, not six copied fixture changes.
4. Clear the programmer and run cue 1.
   - **Expect:** Profiles 11–16 reproduce the look from playback.
5. Patch a compatible profile as fixture 17 and add it to group 2 after fixture 16.
   - **Expect:** The group reports seven ordered members without requiring the cue to be rerecorded.
6. Observe the still-running cue, or release and run it again if required by the chosen playback state.
   - **Expect:** Fixture 17 receives the same group-relative intensity and color. Fixtures removed from group 2 stop receiving that group-relative playback data, while unrelated fixture-scoped values remain intact.

**Pass condition:** Membership can change after programming, and the cue follows the live group reference without duplicating or losing fixture-scoped data.

## Workflow 2: Compare a derived group with a frozen selection

**Purpose:** Prove the difference between a live subdivided reference and a static snapshot of group membership.

1. Select every second member of group 4 with `[GRP] 4 [DIV] 2 [ENTER]`.
   - **Expect:** Alternating fixtures are selected using the ordered membership of group 4.
2. Record the selection as group 5, named `All Stage Odd`.
   - **Expect:** Group 5 retains group 4 as its source and retains the every-second derivation rule.
3. Select the current members of group 4 as individual fixtures with `[GRP][GRP] 4 [ENTER]` and record them as group 6, named `All Stage Snapshot`.
   - **Expect:** Group 6 contains a frozen or static ordered membership and does not retain a live membership link to group 4.
4. Add fixture 17 to group 4 in a position where it changes the alternating pattern.
   - **Expect:** Group 5 recalculates from the new group 4 order. Group 6 remains unchanged.
5. Remove one original member of group 4.
   - **Expect:** Group 5 recalculates again. Group 6 still identifies its original member, including a warning if that member is no longer patched.

**Pass condition:** Derived groups track their source and ordering rule, while frozen selections preserve the captured membership until explicitly refreshed.

## Workflow 3: Program a short theater scene

**Purpose:** Exercise a typical theater-programming sequence using reusable presets, tracked cues, and playback.

The scene is a short evening interior: preset, lights up, an actor crosses to center, the room cools, and blackout.

1. Build and record reusable presets:
   - Record a warm color preset for groups 1 and 2.
   - Record a cool blue color preset for groups 2 and 3.
   - Record a center-stage position preset for group 2.
   - **Expect:** Each preset contains only its intended attribute family and can be recalled on compatible fixtures.
2. Program cue 1, `Preset`:
   - Set all stage intensity to 0 while preparing the warm color and center-stage position values.
   - Record cue 1 on a new playback with a zero-second or deliberately short fade.
   - **Expect:** Running cue 1 establishes the tracked color and position without visible light output.
3. Program cue 2, `Lights Up`:
   - Bring group 1 to 70%, group 2 to 45%, and group 3 to 20% using the warm look.
   - Record cue 2 with a 3-second fade.
   - **Expect:** GO fades from the preset state into the warm stage look over the configured time.
4. Program cue 3, `Cross to Center`:
   - Apply the center-stage position preset to group 2 and raise it to 65%.
   - Record cue 3 with a 2-second fade.
   - **Expect:** Unchanged values from cue 2 track forward; only the intended profile position and intensity change.
5. Program cue 4, `Night`:
   - Apply the cool preset to groups 2 and 3, lower group 1 to 25%, and set group 3 to 50%.
   - Record cue 4 with a 4-second fade.
   - **Expect:** The stage moves smoothly into the cool look while tracked position remains intact.
6. Program cue 5, `Blackout`:
   - Set group 4 intensity to 0 and record cue 5 with a 2-second fade.
   - **Expect:** All stage fixtures fade to zero without deleting their tracked non-intensity values.
7. Clear the programmer, release the playback, and run cues 1–5 using GO.
   - **Expect:** Cue order, current/next indication, fade timing, tracking, Stage view, Fixture Sheet, and rendered output agree throughout the sequence.
8. Use GO minus to return one cue, then GO again.
   - **Expect:** The previous and next looks are reconstructed consistently rather than depending on values left in the programmer.

**Pass condition:** A programmer can build, store, clear, and replay a complete tracked theater sequence with correct preset recall, timing, navigation, and output.

## Workflow 4: Prepare a change in Preload without disturbing the live scene

**Purpose:** Prove that an operator can prepare and store the next change while the audience-facing output remains stable.

1. Run cue 2 from the theater scene workflow and note its live values.
   - **Expect:** The warm `Lights Up` look is active.
2. Enter Preload and select group 3.
   - **Expect:** The UI distinguishes current output, the active preload scene, and pending preload values.
3. Set group 3 to blue at 60% as a pending preload change.
   - **Expect:** The pending values are visible in preload-aware views, but live Stage, Fixture Sheet current values, and rendered DMX do not change.
4. Store the pending preload values into a new cue.
   - **Expect:** The new cue contains the intended group-relative color and intensity while the live cue remains unchanged.
5. Clear pending preload values.
   - **Expect:** Only the pending values clear. The live scene, active preload scene, and stored cue are unchanged.
6. Recreate the pending change and press Preload GO.
   - **Expect:** The change moves into the active preload scene and becomes visible immediately according to Preload GO behavior.
7. Release the preload scene.
   - **Expect:** Preload output is removed cleanly and normal playback output remains authoritative.

**Pass condition:** Pending preload work is isolated from live output, can be stored independently, and can be applied or cleared without corrupting playback state.

## Workflow 5: Save, restart, and resume a show

**Purpose:** Verify the normal end-of-session and recovery path using persisted show data and durable operator state.

1. Save the common test show after creating groups, presets, and the theater cue list.
   - **Expect:** The dirty indicator clears only after the persisted show changes are saved.
2. Leave cue 3 active and place a distinct fixture value in the programmer without recording it.
   - **Expect:** Playback and programmer values are visibly distinguishable.
3. Stop and restart the real server and control application, then reconnect as the same user.
   - **Expect:** Startup succeeds without replacing the show, and the same active show opens.
4. Inspect patch, groups, presets, cue list, playback position, and programmer.
   - **Expect:** Persisted show objects reload with their ordering and references intact; the durable user's programmer returns; the running cue index and playback state follow the product's documented restart policy.
5. Clear the programmer and run the theater sequence again.
   - **Expect:** Playback output matches the pre-restart show data and no temporary programmer value has been written into a cue.

**Pass condition:** Restart preserves the portable show and durable user data without silently merging transient programmer values into stored programming.

## Future test implementation notes

When these definitions become executable tests, keep the layers separate:

- Use UI automation for operator actions, visible state, dialogs, and timing controls.
- Use server or engine tests for exact reference semantics, tracking, persistence, and restart behavior.
- Check rendered fixture values or DMX output for every workflow that claims a live lighting result.
- Give each test its own new show or restore a known fixture so tests cannot pass because another workflow left state behind.
- Preserve the workflow names and pass conditions so automated failures remain understandable to an operator.
