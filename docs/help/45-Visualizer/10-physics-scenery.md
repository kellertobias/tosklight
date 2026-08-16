# Physics-driven Scenery

A fixture package can describe a scenic body whose movement begins from exact DMX values. The
package carries `assets/physics.js`; its bounded `physics(input)` function receives the complete
raw fixture footprint, stable fixture identity, time/elapsed, and the authoritative released,
settled, and timeline state. It returns one versioned action: `hold`, `release`, or `reset`. A script
cannot integrate motion or replace the collision solver. A script error is reported for that body
and does not stop lighting, another scenic body, or the Visualizer.

**Release** is a discrete, latched action. Once released, a body continues falling if the DMX
source disappears, reconnects, or continues to return Release. Only an explicit **Reset** action
or **Reset physics scenery** in **Running & Output** > **Visualizer** restores the authored pose.
The desk command is authoritative and reaches every renderer following that target.

The solver uses deterministic gravity against the authored floor and always settles a falling
body with its lower face on that plane. High and Ultra may also collide with static scenery;
Draft and Standard can omit that optional work while retaining gravity and the floor result.
Scenery collision and collision between physics bodies are independent profile capabilities.
Simulation steps and script execution are bounded so a slow or faulty body cannot make the whole
Visualizer unbounded.

The shipped **Generic Kabuki Curtain** is the reference package. Its one-slot personality uses
`0-31` for Reset, `32-191` for Hold, and `192-255` for Release. These ranges describe this Generic
fixture only; manufacturer packages must map the real device's documented slots in their own
script.

Live body state is matched by stable physical fixture-instance identity when the scene changes.
DMX gaps hold the latest raw values without undoing a release. Each renderer keeps a small
target-specific Release snapshot beside its local Visualizer settings; restart restores the
original event time or settled result instead of replaying Release from a new zero. Reset clears
that snapshot. Timecode moving backwards replays the renderer's bounded discrete Release/Reset
history, so a seek reconstructs the appropriate latched state rather than treating a continuous
DMX level as a new event.
