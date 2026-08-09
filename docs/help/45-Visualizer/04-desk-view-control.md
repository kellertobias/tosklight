# Driving the Visualizer from the Desk

The Visualizer is usually somewhere else: on the far side of the desk, on a second screen,
or on another machine at the back of the room. **Running & Output** carries a
**Visualizer** section so the view can be changed from where the operator is sitting
rather than from the keyboard the picture happens to be next to.

## Choosing the view

The eight named views are the same ones the Visualizer's own number keys reach, in the
same order:

| | |
| --- | --- |
| **3D Full** | Everything the renderer can do: beams, haze, shadows and bloom |
| **Top Down** | The stage plan |
| **Front → Back** | The elevation from the audience |
| **Left → Right**, **Right → Left** | The elevations from the wings |
| **3D Simple** | A fast 3D picture for previsualization |
| **Back → Front** | The elevation from upstage |
| **3D Lines** | Bodies and aim lines, no beams |

Pressing one sends it. The Visualizer takes the view and frames the rig in it, so **Top
Down** is a plan of this rig rather than a plan of wherever the camera happened to be.

**Rendering quality** — Draft, Standard, High or Ultra — is sent the same way. It decides
how much the renderer spends on volumetrics, shadows and bloom, and applies immediately
without the Visualizer reconnecting or reloading the show.

## Who wins

The view is an instruction, and the Visualizer obeys it: whatever the operator standing at
that machine had selected is replaced, and its camera goes with it.

Between instructions, that operator is in charge. They can turn, walk and zoom the camera,
and select another view with the number keys, and the desk does not take it back until it
sends something new. Selecting the view that is already shown is therefore still worth
doing — it puts the camera back where the desk says it should be.

A Visualizer can also hold its own **rendering quality** locally, from its Quick Settings.
A quality held that way says `(local)` on its status bar and is not replaced by the desk;
setting it back to **Follow source** hands the choice back.

## A camera patched as a fixture

The Generic **Visualizer Camera** is a transferable 17-slot personality for a camera that must
be recorded and played back with DMX. It controls the dedicated external 3D Visualizer only:
embedded Stage panes and every 2D view keep their own camera.

While current camera DMX is arriving, its X/Y/Z position, Yaw/Pitch/Roll orientation and Zoom are
authoritative. Dragging, walking or zooming locally changes the status line to **Local camera
control** and holds that override. Press `C` to release it and apply the current DMX pose
immediately. If input is lost or the camera is unpatched, the Visualizer holds the last pose and
local control remains available; it does not jump to defaults. A second patched Visualizer Camera
is ambiguous, so camera routing is disabled and the connection warning names both fixtures.

## More than one Visualizer

Each Visualizer follows one **renderer target**, and the desk keeps a view for each one.
A Visualizer started with no target follows `main`, which is what a single renderer beside
a single desk does without anyone configuring anything.

Start a second Visualizer with `--target front-of-house` and it follows a view of its own:
the desk can then have the plan on one screen and the beams on the other. The
**Renderer** chooser appears in the section as soon as there is more than one to choose
between.

## What is not sent

The view is where the camera is looking, not what the rig is doing. Every live value the
Visualizer draws still arrives as real Art-Net or sACN from the show's own output routes —
changing the view never changes the output, and nothing the desk sends here can light a
lamp.

The haze, the exposure trim and the ambient level stay with the Visualizer: they are that
window's own settings on that machine, adjusted with the wheel over their readouts, and
they never travel into the show.

## Where it is kept

The view belongs to the desk, not to the show file. It is stored with the installation, so
it is still there after a restart, and a show taken to another building arrives without
anyone's camera positions in it.
