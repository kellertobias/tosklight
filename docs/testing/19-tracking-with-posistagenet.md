# Tracking with PosiStageNet

## Purpose

Prove that a PosiStageNet source can drive 3D Points without ever moving a light the operator did
not ask it to, that a source going quiet holds rather than releases, and that a zone runs its Macros
once per crossing rather than once per frame.

## Nothing is bound

1. Open **Show Patch → Tracking** in a show that has never used tracking. Confirm the tab reads as
   off, with nothing bound and no error.
2. Switch **Receive PosiStageNet** on and transmit a PSN frame with two trackers from a sender on
   the configured group.
3. Confirm both trackers appear with their positions in show metres and their age, and that the
   status line names the sender.
4. Confirm no fixture value and no DMX output changed. Traffic alone moves nothing.

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

## Compatibility

1. Open a show saved before tracking existed. Confirm it loads, the tab reads as off, and nothing is
   bound.
2. Bind a tracker, save the show, reopen it, and confirm the binding, the zones, and the calibration
   came back with it.
