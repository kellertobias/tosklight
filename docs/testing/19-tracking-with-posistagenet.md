# Tracking with PosiStageNet

## Purpose and status

Prove that a PosiStageNet source can drive 3D Points without ever moving a light the operator did
not ask it to, that a source going quiet holds rather than releases, and that a zone runs its Macros
once per crossing rather than once per frame.

Automated in `tests/123-tracking-with-posistagenet.spec.ts`; test titles carry the IDs below (all
match `PSN-`). Each case puts the desk's real receiver on its own free UDP port and sends real PSN v2
datagrams unicast to `127.0.0.1` from the bench sender `tests/bench/protocols/psnSender.ts`, whose
bytes are checked against the Rust encoder (`crates/shared/psn/src/encode.rs`) by
`tests/bench/protocols/psnSender.test.ts`. For manual testing against a running desk,
`node tools/psn-sender.mjs --tracker 1:Presenter:0,1,2 [--host <group|host>] [--port <port>]
[--circle <m>]` sends the same stream (default `236.10.10.10:56565`).

| Section | ID | Covered |
| --- | --- | --- |
| Nothing is bound | `PSN-NOTHING-BOUND` | Steps 1–4 (tab, switch, tracker rows, status line, unchanged DMX, Point pose and Programmer). The port is stored through the API first; the switch is touched on the tab. |
| Coordinate boundary | `PSN-COORDINATE-BOUNDARY` | Steps 1–3 on the tracker readout and the bound Point's output pose. Step 4 (displayed beam against desk Aim with compound mount and Point rotations) is native Stage rendering and stays manual; the Aim side is Rust (`aim_point_tests.rs`, `engine/src/tests/mount_projection.rs`), calibration reprojection is Rust (`psn/service_tests.rs` `calibration_*`). |
| A marker moves a point | `PSN-MARKER-MOVES-POINT` | Steps 1–5: binding through the tab's 3D Point choice; Point output pose follows the marker; a mover programmed to Position Target at the Point changes its Pan/Tilt DMX with it (gated by `requireSemanticContract`); a recorded cue and a Programmer Point value do not take the Point; switching the binding off returns the Point to the show in the next output frame while the tracker keeps moving. The Stage view drawing itself is native and not asserted. |
| Silence holds | `PSN-SILENCE-HOLDS`, `PSN-SILENCE-HOLDS-NEW-SOURCE` | Steps 1–5 with the same sender socket stopping and resuming. `PSN-SILENCE-HOLDS-NEW-SOURCE` is an expected failure (`test.fail`): a sender restarted from a new source port is tracked again, but the status stays stale. |
| Zones run Macros | `PSN-ZONES-RUN-MACROS` | Steps 1–5, counted from the Macro runtime's tracking-triggered executions. The command line has no text form for a playback Off, so the two Macros set and clear the front dimmers instead of a playback. |
| What arrives that should not | `PSN-UNWANTED-ART-NET`, `PSN-UNWANTED-UNICAST-GROUP`, `PSN-UNWANTED-PORT-IN-USE` | Steps 1–3. |
| Settings stay in reach | `PSN-SETTINGS-IN-REACH` | Steps 1–4 at 1280×560 (also covered per window size in `tests/100-show-patch-configuration-layout.spec.ts`). |
| Compatibility | `PSN-COMPATIBILITY-LEGACY-SHOW`, `PSN-COMPATIBILITY-ROUND-TRIP` | Step 1 with the committed pre-tracking `compact-rig` show; step 2 through the downloaded show file opened as a new show, and the original reopened. |

Multicast group delivery itself (sender to `236.10.10.10`) is not asserted in CI because it depends
on the host's multicast routing; the receiver path is the same socket. Use `tools/psn-sender.mjs`
without `--host` to check it on a rig.

## Nothing is bound

1. Open **Show Patch → Tracking** in a show that has never used tracking. Confirm the tab reads as
   off, with nothing bound and no error.
2. Switch **Receive PosiStageNet** on and transmit a PSN frame with two trackers from a sender on
   the configured group.
3. Confirm both trackers appear with their positions in show metres and their age, and that the
   status line names the sender.
4. Confirm no fixture value and no DMX output changed. Traffic alone moves nothing.

## Coordinate boundary

1. With zero calibration send PSN `(1, 2, -3)` metres. Confirm the desk shows `(1, 3, 2)` metres.
2. Increase only PSN Y. Confirm the Point rises in desk Z without moving across/upstage.
3. Apply 90° calibration rotation. Confirm the Point rotates in the desk XY plane with height unchanged.
4. With compound mount and Point rotations, confirm the displayed beam and desk Aim calculation
   agree on the same target. Existing pre-v1 calibration files use the corrected axes; no inferred
   migration of an operator's previous offsets is performed.

## A marker moves a point

1. Patch a 3D Point and aim a moving light at it. Bind tracker 1 to that point on the Tracking tab.
2. Move the marker. Confirm the point follows it in the Stage view, and the light follows the point.
3. With the marker still moving, go a cue that stores a position for that point. Confirm the point
   stays with the marker.
4. Take the point's position encoder and move it. Confirm the point stays with the marker.
5. Switch the binding off. Confirm the point returns to what the show says, in the same frame, and
   that the tracker is still listed and still moving.

## Silence holds

1. With a binding live and the light following, stop the sender.
2. Confirm the status line reports the source as stale with how long it has been silent, and the
   tracker row is marked stale.
3. Confirm the light has not moved: the point holds the last position that arrived.
4. Start the sender again. Confirm the point picks the marker up without an operator action.
5. Switch **Receive PosiStageNet** off. Confirm every bound point returns to the show at once, and
   that the bindings are still listed for when it is switched back on.

## Zones run Macros

1. Create a Macro that turns a playback on and another that turns it off. Add a zone covering a
   downstage area, choose those two Macros for entering and leaving, and leave **Hold for** at its
   default.
2. Walk a marker into the zone. Confirm the entering Macro ran once and the zone reads as occupied.
3. Stand the marker exactly on the zone boundary so its reported position crosses in and out.
   Confirm neither Macro runs again.
4. Walk out. Confirm the leaving Macro ran once.
5. Stop the sender while the zone is occupied. Confirm the leaving Macro does **not** run and the
   zone stays occupied.

## What arrives that should not

1. Send an Art-Net packet to the PSN group. Confirm it is counted as ignored, no tracker appears,
   and the desk keeps receiving PSN.
2. Set the group to an address that is not a multicast group. Confirm the desk refuses the edit,
   names the address and the reason, and keeps listening where it was.
3. Configure a port already in use by another program. Confirm the tab shows an actionable error and
   the rest of the desk keeps working.

## Settings stay in reach

1. In a short window, open **Show Patch** and switch between **Fixtures**, **Media Servers**, and
   **Tracking**. Confirm the tabs and **⚙** stay in exactly the same place, and that Media Servers
   and Tracking each scroll to their last control with Settings-sized inner margins.
2. On the Tracking tab, press **⚙**. Confirm Settings opens on **Tracking** with **Multicast
   group**, **Port**, and **Stale after (ms)** holding the stored values, and that these fields are
   no longer on the Tracking tab itself.
3. Enter a group outside `224.0.0.0`–`239.255.255.255` and apply. Confirm the field names the
   problem and nothing is stored.
4. Enter a valid group, port, and Stale after, apply, close and reopen Settings. Confirm the values
   came back and the desk reports them.

## Compatibility

1. Open a show saved before tracking existed. Confirm it loads, the tab reads as off, and nothing is
   bound.
2. Bind a tracker, save the show, reopen it, and confirm the binding, the zones, and the calibration
   came back with it.
