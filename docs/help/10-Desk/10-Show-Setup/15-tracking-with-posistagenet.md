# Tracking with PosiStageNet

A PosiStageNet tracking system sends performer positions to ToskLight. Bind a tracker to a **3D Point**, then aim fixtures at that Point to follow it. Network traffic alone moves nothing.

Open **Show Patch > Tracking**.

## What the desk does and does not do

The sender provides positions; ToskLight applies them only to explicitly bound Points. While bound, a Point follows the tracker instead of Cue or encoder positions. Unbind it or disable the binding to return control to the show.

## Set up the source

1. Open **⚙ > Tracking** in Show Patch.
2. Match the sender's **Multicast group** and **Port**; defaults are `236.10.10.10` and `56565`.
3. Set **Stale after (ms)** for lost-input reporting and press **Apply tracking settings**.
4. Enable **Receive PosiStageNet** on the Tracking tab.

**Revert** restores the stored settings. Invalid values remain marked without changing the receiver. The status distinguishes listening with no data, receiving from a sender, and stale input.

## Bind a tracker to a 3D Point

Choose a Point in a discovered tracker's row. Its position is shown in show metres; a new tracker can initially show a number until its name arrives.

**Bound points** shows the resulting positions. **Out of reach** means the tracker exceeds the Point's travel: move the Point's patched resting position nearer the working area.

## When the tracking system goes quiet

Bound Points hold their last position, so lights do not jump to the origin or back to Cue positions. Check the stale message and sender/network. Turning **Receive PosiStageNet** off deliberately returns Points to show control immediately.

## Zones

A zone runs a Macro when it becomes occupied and another when its last watched tracker leaves.

| Setting | Use |
| --- | --- |
| **From / To** | Opposite corners in show metres. |
| **On entering / On leaving** | Macros to run; None means no action. |
| **Hold for** | Required settling time before a change counts. |

Use Hold for to avoid repeated actions near an edge. Lost tracking holds zone occupancy rather than firing a leave Macro.

## Calibration

**Origin x/y/z**, **Rotation**, and **Scale** align tracking with the rig. Offsets and zones use desk coordinates: X across stage, Y upstage, Z up. Rotation is about desk Z. PosiStageNet's Y-up/depth convention is converted automatically; its positive depth becomes downstage.

Set the origin and rotation against known stage points before aiming real fixtures. Use Scale only when the sender's units differ from metres.

## Where the configuration lives

Source settings, bindings, zones, and calibration travel with the show. Older shows start with tracking disabled and no bindings.
