# Particle Effects

Flame units and cold-spark fountains are patched and programmed like any other DMX fixture. Their
fixture type is **Effect**. ToskLight does not carry a built-in program for a manufacturer or
guess what one of its DMX slots means: the transferable `.toskfixture` carries
`assets/effect.js`, which maps that product's exact slots to what the Visualizer should show.

The shipped **Generic Cold Spark Fountain** demonstrates separate intensity, fountain-height and
visible-spark-lifetime controls. **Generic Five-nozzle Flame Unit** demonstrates one package-owned
program producing five independently positioned and angled nozzles. A manufacturer profile should
replace those generic charts and script with the manual's exact modes, trigger, retrigger, release,
Off and supported-fluid-colour behavior.

## Portable script boundary

An Effect script receives only:

- the fixture's raw DMX slots in patch order;
- decoded master intensity;
- authoritative time and elapsed time;
- stable fixture identity; and
- a deterministic capture seed.

There is no filesystem, network, timer, console, import host, renderer access or GPU access. Each
patched fixture has isolated script state. A compile error, exception, malformed result, memory
failure or 4 ms deadline overrun turns off and diagnoses that fixture without stopping any other
fixture or the renderer.

Version 1 returns at most 32 declarative flame or spark emitters. Each emitter may state local
origin and direction, width, height/reach, intensity, density, particle lifetime, colour and a
typed Off, trigger, hold, release or retrigger state. The versioned family contract reserves a
future physics-backed debris result for confetti through TL-107; version 1 rejects it rather than
silently pretending to support it.

## Rendering and restart behavior

Particles are emissive geometry in the normal opaque/depth pass. Scenery in front hides them, and
bright flame and sparks feed the existing bloom. They do not cast dynamic light or shadows.

A live Visualizer restart starts a fresh simulation from current authoritative DMX. It does not
claim to reconstruct individual pre-restart particles. Deterministic capture uses stable identity,
seed and timeline time so repeated product captures match without turning live particle history
into portable show state.

## Quality budgets

Population is best effort and the major shape is preserved: every active declarative nozzle gets
one particle before the remaining budget is distributed round-robin. The upper bounds are
128 particles in Draft, 512 in Standard, 2,048 in High, 4,096 in Ultra and 8,192 in Extreme.
Extreme follows the renderer hardware ladder, reducing that ceiling together with resolution and
volumetric work when the measured GPU frame cost cannot sustain 60 Hz. When requested
population is higher, **FrameStats** exposes requested and drawn counts and marks the frame
degraded; density is reduced without dropping an entire nozzle or allowing a package to grow GPU
buffers without limit.

The standard built-in benchmark includes a package-equivalent cold-spark fountain. On the Apple
M5 Max Metal reference machine, the 33-fixture Full 3D scene rendered its 220 requested particles
without particle reduction in High, Ultra and Extreme. Extreme alone owns the adaptive ladder;
High and Ultra keep their documented fixed budgets. Machine-specific measurements are recorded in
the Visualizer GPU cost engineering note rather than presented as a product-wide maximum.
