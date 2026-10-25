# Drive PreViz from the Desk

Use **Running & Output > Visualizer** to control a connected external Visualizer from the Desk.
The tab is available while a Visualizer is connected. Settings for the Desk's internal Stage view
remain in **Settings**.

## Choosing the view

The eight named views are the same ones the Visualizer's own number keys reach, in the
same order:

| | |
| --- | --- |
| **3D Full** | Everything the renderer can do: beams, haze, shadows and bloom |
| **Top Down** | The stage plan |
| **Front > Back** | The elevation from the audience |
| **Left > Right**, **Right > Left** | The elevations from the wings |
| **3D Simple** | A fast 3D picture for previsualization |
| **Back > Front** | The elevation from upstage |
| **3D Lines** | Bodies and aim lines, no beams |

Pressing one sends it. The Visualizer takes the view and frames the rig in it, so **Top
Down** is a plan of this rig rather than a plan of wherever the camera happened to be.

**Rendering quality** — Draft, Standard, High or Ultra — is sent the same way. It decides
how much the renderer spends on volumetrics, shadows and bloom, and applies immediately
without the Visualizer reconnecting or reloading the show.

**Draft** keeps fixture geometry, labels and lasers at the view's physical resolution, with a
simpler diffuse surface shader and a separate beam layer at 35% of each physical dimension. Uniform unshadowed
beam segments use analytic integration; optical and shadowed segments retain bounded sampling.
Draft retains a two-light shadow budget for opaque scenery; other lamps can light through
opaque objects outside that budget. Drapes retain separate blocking checks, and visible opaque
silhouettes still clip the beam layer. The result is an interactive approximation for aim, color and
scene relationships; use Ultra or a [4K image render](03-snapshots-and-blender.md#render-a-4k-image)
to inspect fine lighting detail.

**3D Lines** uses model-derived edges and actual depth occlusion without the lighting, shadow or
bloom passes. A held static view does not need recurring render submissions. Camera movement and
moving heads still redraw the real rig.

Standalone Draft and 3D Lines use a 60 Hz ceiling for steady animation, including on 120 Hz screens.
Immediate input or reveal redraws can occur sooner; static pictures stop GPU submissions.
Standard, High and Ultra lighting views retain display-driven pacing.

Fresnel fields have soft edges; PAR and ACL fields retain a brighter centre with a soft falloff. Draft and Standard retain these basic field shapes and physical barn-door cuts. Draft's unshadowed, unshaped shafts keep a fixed four-point angular estimate alongside analytic distance integration.

Explicit tungsten and halogen sources use a nominal lamp response: fast heating, fast initial dimming and slower cooling near low levels. Choose the installed **Light source** in Show Patch when the profile does not identify it. LED and unknown sources remain immediate. This changes the visualized lamp only; DMX values and Cue fades keep their own timing. Observer persistence remains separate. The response is an estimated preview, not measured calibration for every lamp or dimmer.

**Reset physics scenery** is a separate authoritative show-control action. It restores every
released physics body on the selected renderer target to its authored pose. Re-selecting a view,
losing DMX, or reconnecting is not a reset.

At Ultra, the dedicated visualizer offers independent cloudiness and turbulence controls for lamps and lasers in **Quick Settings**. Cloudiness ranges from even to patchy haze; turbulence controls how quickly it changes. Draft, Standard, and High use uniform fog.

Control's internal Stage provides only the lightweight 2D plan and 3D wireframe views. Open the separate Architect visualizer for rendered beams, haze, and shadows.

## Desk and local control

A Desk view command selects and frames that view on the connected Visualizer. Between Desk
commands, the local operator can navigate the camera or select a view with the number keys.
Selecting a view again restores its Desk framing.

The local operator can set **Rendering quality** in **Quick Settings**. A local setting is marked
`(local)` in the status bar and is kept until **Follow source** is selected. If the renderer must
reduce the requested quality, Quick Settings shows the active limit and how to restore it.

## Visualizer Camera

Patch **ToskLight Visualizer Camera** when camera position and zoom must be recorded and played
back with DMX. It controls the dedicated external 3D Visualizer only; Stage panes and 2D views
keep their own cameras.

While camera DMX is live, it controls X/Y/Z, Yaw/Pitch/Roll, and Zoom. Local camera navigation
sets **Local camera control**. Press `C` to resume the current DMX pose. If camera input is lost
or the camera is unpatched, the Visualizer holds its last pose and local navigation remains
available.
