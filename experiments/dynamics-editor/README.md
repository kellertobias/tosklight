# Dynamics editor experiment

Standalone, clickable frontend dummy based on `docs/plans/Later/02-dynamics.md`.

This is deliberately isolated from `apps/control-ui`: it has no imports, packages,
API calls, persistence, or backend behavior. The small CSS subset and controls used
by the dummy are copied into this folder, so changes to the production UI do not
affect the experiment.

Open `index.html` directly in a browser, or serve this folder with any static file
server.

The current layout explores vertically stacked scalar attribute lanes, a primary
six-encoder workflow with simulated turn and push gestures, a compact half-height
iPad layout, and fixture phase positions drawn as numbered points on each curve.
The selected lane also has a persistent keyframe strip for inserting points and
choosing Preset, Fixed, or Current as each keyframe's scalar source.

Three title-bar views separate the operator tasks: **Curves** defines scalar lane
shapes and keyframes, **Phase spread** applies one shared fixture distribution and
shows it on every attribute curve,
and **Speed** configures duration or Speed Group transport, synchronization, and
per-lane multipliers. The six encoders remap to the active view.

The dummy explores proposed UI vocabulary only. It does not settle any of the open
product decisions or make the Dynamics plan implementable.
