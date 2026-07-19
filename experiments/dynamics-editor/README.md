# Dynamics editor experiment

Standalone, clickable frontend dummy based on `docs/plans/Later/02-dynamics.md`.

This is deliberately isolated from `apps/control-ui`: it has no imports, packages,
API calls, persistence, or backend behavior. The small CSS subset and controls used
by the dummy are copied into this folder, so changes to the production UI do not
affect the experiment.

Open `index.html` directly in a browser, or serve this folder with any static file
server.

The current layout explores vertically stacked scalar attribute lanes, a primary
six-encoder workflow, a compact half-height iPad layout, and a fixed live 10×10
fixture grid. Each fixture square shows intensity and RGB output, with a position
dot whose X coordinate is pan and whose Y coordinate is tilt.
The selected lane also has a persistent keyframe strip for inserting points and
choosing Preset, Fixed, or Current as each keyframe's scalar source.
Clicking a lane selects only that lane. Shift-clicking adds an unselected lane to
the selection or removes a selected lane; the selection always retains at least
one lane. A later ordinary click collapses the selection to that one lane.
Each encoder exposes regular −/+ turns, pushed −/+ turns, and a central Push action.
Holding any regular or pushed turn button starts keyboard-style auto-repeat after
380 ms, then repeats the turn every 85 ms until release.
Turning encoder 1 selects a keyframe in Keyframes mode or chooses the built-in
function in either Function mode. Its pushed turn cycles Keyframes, Function
max/min, and Function amplitude. Push opens a three-tab modal where the operator
can choose the mode and its selected keyframe or function. Encoders 2 and 3 edit
the relevant typed sources or amplitude;
pushed turns cycle Current, Fixed, and Preset, while Push opens the tabbed editor.
PWM maps attack/on to encoder 4 and decay/off to encoder 5. Attack is contained
inside On and decay is contained inside Off; On and Off remain the fixed cycle
partition, so editing either slope does not move the start, boundary, or cycle end.
Encoder 6 turns lane speed and uses pushed turns for width. Function width
compresses the function into the beginning of the cycle and holds its terminal
value until the cycle restarts.

Each lane preserves all three configurations while switching modes. The bottom
editor mirrors the three-way mode selection. Function lanes offer Sinus, Cosinus,
Linear +, Linear −, PWM, Random gate, Random timing, Random gate + timing, and a
future Macro placeholder. Max/min mode uses
typed Top and Bottom sources; amplitude mode uses a typed Middle source plus
Amplitude. Keyframed segments offer Linear, Ease in, Ease out, Ease in + out, Hold,
and Drop interpolation—never PWM. The circle example uses a Current-centered Sinus
for Pan and a Current-centered Cosinus for Tilt.

Random is split into three separately selectable functions. **Random gate** makes
seeded minimum/maximum gate decisions at regular opportunities. **Random timing**
places maximum pulses at random moments. **Random gate + timing** randomizes both
the event moments and their minimum/maximum gate decisions. Density is a target
rather than a fixed count, so each timed loop varies; Grouping moves events from
scattered hits toward bursts of two to four, and Pulse controls event length.
Random 1–4 are shared seed sources. Lanes
linked to the same source receive the same underlying per-loop random stream while
retaining their own bounds, amplitude, density, grouping, and pulse settings.
Encoder 4 edits Pulse and pushed-turn edits Grouping. Encoder 5 edits Density and
pushed-turn selects the shared Random source. Encoder 6 keeps lane speed and its
pushed-turn edits function Width. A compact Help modal explains these mappings,
selects the shared Random source, and can generate a new seed for every linked lane.
Every Random function starts and finishes at the lane minimum; in particular, a
gate-based Random never begins with a maximum gate. Width compresses its events
into the start of the cycle and holds that minimum through the remaining idle tail.

Three title-bar views separate the operator tasks: **Curves** defines scalar lane
shapes and keyframes, **Phase spread** applies one shared fixture distribution and
shows it on every attribute curve, and **Speed** configures shared duration or
Speed Group transport and synchronization. Per-lane speed multipliers live with
their attribute curves. The six encoders remap to the active view.

Clicking the example name switches between a circle around the current position, a
top-down linear tilt travel with an intensity envelope, a radial red-white color
wave, and an intensity-only Random Strobe. Selection, random-each-loop, grid-linear,
radial-out, radial-in, and axial/radar fixture orders all drive the live grid
entirely in the browser.

Grid-linear ordering has an explicit direction angle: 0° runs left to right,
90° top to bottom, 180° right to left, and 270° bottom to top; intermediate
angles run diagonally. In Phase spread, encoder 6 controls that angle whenever
Grid linear is selected. Phase span is presented as wave width: 180° broadens
the spatial wave, 360° places one cycle across the grid, and 720° compresses it
to two repeated, narrower cycles. Tightening the color or intensity keyframes in
Curves instead produces a narrower single band without adding a second wavefront.
**Random each loop** creates a new seeded permutation of all 100 fixtures whenever
the effect begins another iteration. The Random Strobe example combines Random
timing, burst-oriented grouping, a 45% function width, Random 1, and this changing
fixture order, so both flash timing and fixture order visibly change each round
before an idle minimum-value tail.

The Speed view includes six simulated Speed Groups at 60, 85, 95, 105, 120,
and 150 BPM. A group drives a four-beat cycle, so selecting another group changes
the live fixture-grid animation immediately; Fixed duration remains available.

The dummy explores proposed UI vocabulary only. It does not settle any of the open
product decisions or make the Dynamics plan implementable.
