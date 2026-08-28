# Tracking with PosiStageNet

A tracking system — an OpenFollow station, or anything else that speaks PosiStageNet — says where
things on stage are. ToskLight listens, and an object it is told about can become a **3D Point** in
your show. Fixtures already follow 3D Points, so a light aimed at the point follows the performer.

Open **Show Patch** and choose the **Tracking** tab.

## What the desk does and does not do

The tracking system does not aim anything. It sends positions; the desk decides what they mean.
That is why **traffic on the network moves nothing on its own**. A tracker only moves a 3D Point
after you have given it one on this tab.

While a tracker is bound to a 3D Point, **that point is the marker**. A cue with a stored position
cannot pull it away, and an encoder will not move it. To take it back, unbind it, or switch the
binding off — both are on this tab, and both take effect immediately.

## Set up the source

PosiStageNet is multicast, so the desk joins a group and listens; there is nothing to connect to.

| Setting | What it is |
| --- | --- |
| **Receive PosiStageNet** | Off means nothing is received and no point is held. |
| **Multicast group** | Where the tracking system transmits. `236.10.10.10` unless it has been moved. |
| **Port** | `56565` unless it has been moved. |
| **Stale after** | How long without a packet before a tracker is reported stale. |

The line above the settings says what is happening in plain words:

- **Listening … Nothing has arrived yet** — the sender may be off, or the desk may be on another
  network. PosiStageNet cannot tell those apart, so the desk does not guess.
- **Receiving on … from …** — packets are arriving, and this is who is sending them.
- **Nothing heard … for 4s. Bound points are holding their last position.** — see below.

## Bind a tracker to a 3D Point

Every tracker the desk has heard from is listed with its position in your show's own metres and how
long ago it was last seen. Choose a 3D Point in its row to bind it.

Names come in their own packet about once a second, so a tracker that has just appeared shows only
its number for a moment. That is normal.

Under **Bound points** each binding shows where the point actually ended up. If a marker walks
further from the point's patched position than a 3D Point can reach, the point stops at the end of
its travel and the row says **out of reach** — move the point's patched position closer to the area
being covered.

## When the tracking system goes quiet

**Bound points hold their last position.** They do not jump back to what the show says, and they do
not go to the origin. A light stays where it was pointed, which is a light you can take over. The
Tracking tab says the source is stale, and how long it has been silent.

Switching **Receive PosiStageNet** off is different: that is something you did on purpose, and the
points go back to the show at once.

## Zones

A zone is a box on stage. When somebody the zone watches is inside it, the zone runs a **Macro**;
when the last of them leaves, it runs another. Anything you can write in a Macro is what a zone can
do, including turning a playback on and off again.

| Setting | What it is |
| --- | --- |
| **From / To** | The two opposite corners of the box, in show metres. |
| **On entering** | The Macro to run when the zone becomes occupied. |
| **On leaving** | The Macro to run when it becomes empty. Leave it at **None** for a zone that should not turn itself off. |
| **Hold for** | How long the change has to last before it counts. |

**Hold for** is what keeps a performer standing on the edge of a zone from firing the Macro over and
over: the desk waits for the new state to settle before acting on it.

A source going quiet does not empty its zones. A network switch rebooting is not everybody walking
off stage, so nothing runs and the zones stay as they were.

## Calibration

PosiStageNet and ToskLight already agree: metres, x to the right, y up, z into the depth of the
stage. A tracking system whose origin is your show's origin needs no calibration at all.

Use **Origin x/y/z** when the tracking system was told its origin is somewhere else, and
**Rotation** when it was set up facing another way. **Scale** is there for a system reporting in
something other than metres.

## Where the configuration lives

All of it is show data: the bindings, the zones, the calibration, and the group. It travels with the
show to whichever desk is in the building. A show saved before this existed opens with tracking
switched off and nothing bound.
