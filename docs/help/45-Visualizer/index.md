# Visualizer

The Visualizer draws the rig: the lamps where they are patched, the light they are
putting out, and the stage they are pointed at. It reads the same patch the desk runs
from, so what it shows is what the desk is outputting, not a separate drawing that has to
be kept in step.

Every fixture in the picture has a body. A fixture profile can carry its own 3D model, and
the fixture library ships several that do. A fixture whose profile carries no model is not
drawn as a grey box: ToskLight picks one of the models it ships from what the profile says
about itself, so a rig of imported profiles still reads as a rig of lamps.

Movement is physical in the Visualizer, while desk output remains immediate. A fixture profile
can give each Pan, Tilt, wheel, or rotation function its exact angular endpoints and its maximum
speed, acceleration, and deceleration. Absolute functions keep their authored winding, endless
functions request a signed speed, and ordered colour and gobo wheels visibly cross intermediate
slots. The channel's exact **Default raw** value is decoded as the home target; the model's authored
local pose is physical 0°. Older profiles without these limits use deterministic responsive
Visualizer defaults of 540°/s maximum speed and 1,080°/s² acceleration and deceleration instead
of moving instantaneously.

Lasers are the exception to all of that. What a laser draws is decided by a pattern engine
inside the projector rather than by anything its channels describe, so a laser profile carries
that engine as a small script and the Visualizer runs it to find the path the beam takes.

An audience is a scalable **Venue — Crowd Area** fixture. Its mode chooses posture and density,
its independently authored width and depth remain visible as a plan footprint, and the Visualizer
fills it with deterministic silhouettes within the active quality budget.

Particle-producing equipment uses the same portable idea without pretending to be a laser. An
**Effect** fixture carries its own bounded DMX-to-emitter script for flames or sparks; the particles
are depth-tested, emissive and budgeted by quality.

A look on stage can also be kept and taken away: a snapshot freezes the rig as it stands and
can be turned into a Blender file for a finished render.

The desk decides which way the picture is looking. While an external Visualizer is connected,
**Running & Output** shows a **Visualizer** tab with the eight named views and rendering quality,
so the view can be changed from the desk rather than from the machine the Visualizer happens to
be running on. Internal Stage settings remain in **Settings**.

The rig itself can come from either side. The **Viz Editor** plans a rig against the same patch
sheet the desk uses, and each application offers the other's show directly when both are on one
network — no file to find, and nothing shared beyond the copy that crosses.

Continue with [Default Models](01-default-models.md) for how that choice is made, the
[Model Catalogue](02-model-catalogue.md) for every model ToskLight ships and the name it
goes by, [Snapshots and Blender](03-snapshots-and-blender.md) for keeping a look and
rendering it, [Driving the Visualizer from the Desk](04-desk-view-control.md) for
selecting the view and the quality from the desk, and [Lasers](05-lasers.md) for scan
scripts and the persistence-of-vision settings that make lasers and strobes read correctly, and
[Planning a Rig, and Moving It](06-planning-and-transfer.md) for the Viz Editor and moving a show
between it and the desk, and [Particle Effects](07-particle-effects.md) for transferable flame and
cold-spark engines, trigger semantics, restart behavior and quality budgets. Continue with
[Crowd Areas](08-crowds.md) for modes, footprint editing, deterministic placement, local amount
control, and measured quality budgets, and [Media Surfaces](09-media-surfaces.md) for screens, TVs,
LED walls, projectors, CITP output selection, and portable fallback pictures.
