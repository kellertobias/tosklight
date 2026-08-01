# Lasers

A laser is the one fixture the Visualizer cannot draw from its channels.

Every other kind of light can be described by parameters. A profile at forty per cent through
a blue gel with the iris at half is a cone the Visualizer can build, and two fixtures given the
same values look the same. A laser given the same values does not. What an audience sees comes
out of a pattern engine inside the projector, and the DMX only chooses between its presets and
modulates them. Two projectors on the same address draw completely different pictures, and no
list of attributes will ever say which.

So a laser profile carries its pattern engine as a small piece of JavaScript, and the Visualizer
runs it. Once per drawn frame the fixture's script is handed its own DMX slots and returns the
path the beam actually takes.

## What a laser needs

A fixture is treated as a laser when its profile's fixture type says so — anything containing
the word `laser`. That is enough to draw it as a scanned path rather than as a cone, but a laser
with no script does not project: it stays dark, and the Visualizer says why in the status line
rather than inventing a pattern for it.

The rest lives in the profile's `laser` block, and every figure in it is optional. A profile that
declares none of them is treated as a typical mid-range projector: a 25-degree scan angle,
30 000 points per second, one milliradian of divergence, a 3 mm aperture and one watt.

| Field | What it is |
| --- | --- |
| `scan_script_asset` | The pattern engine, packaged as `assets/scan.js` |
| `scan_angle_degrees` | Full optical scan angle, the whole cone the scanner reaches |
| `scan_angle_y_degrees` | The same across the vertical, when the scanner is not square |
| `points_per_second` | Scanner speed — the figure a manual quotes as "30 kpps" |
| `divergence_milliradians` | How much the beam has spread by the time it lands |
| `aperture_millimetres` | Beam diameter at the output window |
| `optical_power_milliwatts` | Total output with every colour at full |

The scanner speed matters more than it looks. Together with how many points a figure has, it
decides how many complete passes of that figure fall inside one frame — which is the difference
between a pattern that reads as solid and one that reads as a travelling dot. It is also why the
same projector looks dimmer on a large pattern than on a small one.

## What you see

There are two pictures in a laser and they are made of the same light.

The **figure** is the path the beam draws across whatever it lands on. It is what the pattern
engine describes, and it is there whether or not there is anything in the air.

The **beam** is everything between the projector's window and that figure: the sheet the beam
sweeps as it draws, over and over, faster than an eye can follow. It is brightest at the window,
where every part of the figure is still one beam, and thins out towards the pattern. Haze is what
makes it visible, so it fades as the fog amount comes down while the figure on the deck does not.

The projector itself is a box with its output window in the front face, and the beam leaves that
window pointing the way the window looks. A laser hung level fires level, and the bracket angle in
the patch is what aims it — the same thing that aims a lantern in a clamp. Its position channels
are *not* a yoke: on almost every show laser they move the figure inside the scan field, the scan
engine already applies them, and the Visualizer leaves the projector where it is hung.

## Laser brightness

**Laser brightness** in Quick Settings sets how strongly every laser in the rig is drawn, from
nothing at all to four times the built-in strength. It moves the figures and the beams together:
it answers "how much of this picture is laser", not anything about one of the two.

It exists because a laser is the one thing on the stage with no honest reference. How strong a
beam looks against the rest of a rig depends on the haze, the size of the room, the projector and
whether it is an eye or a camera at the other end — and a designer checking that a figure is the
right shape and someone showing a client what the night will look like do not want the same
answer. Turning it to zero draws the rig without its lasers, which is one way to see what they
were covering.

Like the fog amount and persistence, it belongs to this Visualizer on this machine and is never
taken from the show. `--laser <percent>` sets it at launch.

## Writing a scan engine

A scan script is a JavaScript module exporting one function:

```javascript
export function scan(input) {
  return {
    points: [
      { x: -1, y: 0, r: 1, g: 0, b: 0, amount: 50 },
      { x:  1, y: 0, r: 1, g: 0, b: 0, amount: 50 },
    ],
    pointsPerSecond: 30000,
  };
}
```

`input` carries `dmx`, the fixture's own slots in patch order from its start address; `time`,
seconds since the Visualizer started; `elapsed`, seconds since this laser's last frame; and
`intensity`, the decoded master level.

Each control point has:

| Field | Meaning |
| --- | --- |
| `x`, `y` | Deflection, `-1` to `1` of the scanner's half angle |
| `r`, `g`, `b` | Colour, `0` to `1`. A missing channel is off |
| `amount` | Percentage of one complete scan spent reaching this point |

A point may also be written compactly as `[x, y, r, g, b, amount]`, which is worth doing once a
figure runs to hundreds of points.

Three things about that list are worth knowing before writing one.

**A point carries the colour of the run that arrives at it**, not the one that leaves it. That is
the usual convention, and it is what makes blanking expressible: a black point means the scanner
travelled to it with the light off, which is how a pattern jumps between figures without drawing
the join. Without it every corner would be joined to every other one.

**`amount` is a brightness as well as a timing.** The scanner moves at a fixed speed, so a run it
crosses quickly receives less light per metre than one it lingers on. That is why the corners of
a real laser figure are visibly brighter than its edges, and drawing a figure with even timing
along an uneven path will look like a wireframe rather than a laser. The percentages do not have
to add up — they are rescaled — and a script that names no timing at all is drawn as an even
sweep.

**The module keeps its state between frames.** A pattern that rotates or animates should
integrate `input.elapsed` rather than reading the clock, so it turns at its own rate from wherever
it happens to be. Each patched fixture gets its own copy, so two projectors of the same model
running the same script do not animate in lockstep.

A script has no host beyond `input`: no files, no network, no timers, no console, no imports. It
gets a few milliseconds per frame and is stopped if it runs longer. Anything that goes wrong —
a script that will not compile, one that throws, one that returns something unusable, one that
overruns — leaves that laser dark with the reason on screen. It never takes the frame with it.

## Editing a script without rebuilding a package

A packaged script is what makes a show portable, and it is the wrong thing to author against:
every experiment would mean rebuilding an archive and re-importing it.

Point the Visualizer at a directory of loose scripts instead:

```bash
viz-renderer --laser-scripts ~/laser-patterns
```

A file named after a fixture's profile ID — `<profile id>.js` — replaces whatever that fixture's
package carries. Save the file and the change appears; delete it and the packaged engine comes
straight back. Nothing about the show moves: the fixture, its patch and its profile are untouched
throughout. When the pattern is right, it goes into the package and the loose file comes out.

`TOSKLIGHT_VIZ_LASER_SCRIPTS` sets the same directory.

## Persistence of vision

A drawn frame is an instant. An eye is not.

A laser completes its figure hundreds of times a second and is seen as a solid shape; a strobe
puts light in the room for a few milliseconds and is seen as a flash that lingers. Sampling
either at the moment a frame happens to fall gives neither — a strobe that flickers irregularly
against the refresh rate, and a laser made of dots.

So the Visualizer holds what an observer still has. A light that goes dark decays towards its
new level over a set time rather than dropping to it, while a light coming up is seen
immediately. Two settings control it, beside the fog amount:

**Persistence of vision** is how long a light takes to go from full brightness to black. A tenth
of a second is roughly what an eye does, and it is what makes a strobe read as a strobe. Setting
it to zero turns the whole mechanism off, and every frame then shows exactly what the desk is
sending at that instant — useful for checking what a fixture is actually being told.

**Persistence falloff** shapes the tail. At `1.00` the fade is a straight line. Above it the
first, brightest part of a flash is unchanged and the dim remainder drops away faster, so the
stage does not sit in a permanent haze of its last cue. The default of `2.00` is a square
falloff.

The decay is measured against full brightness rather than against whatever level a light happened
to be at, so a half-brightness flash disappears in half the time a full one does — which is both
what an eye does and what an operator expects.

Both settings belong to this Visualizer on this machine and are never taken from the show, for
the same reason the fog amount is not: they describe the observer and the display, not the rig.
An operator who wants to see individual strobe flashes can turn persistence down without editing
the show everyone else is running.

Strobes benefit from this whether or not there is a laser in the rig. The shutter gate is worked
out from how much of each frame it was actually open, rather than from whether it happened to be
open at the instant the frame fell, so a strobe delivers its real weight at any rate against any
frame rate.

## What is not drawn

A laser lights the air and puts its figure where it lands. It does not cast shadows, and it does
not light surfaces the way a lantern does — the scattered light from a beam a few millimetres
across is small enough that a rig full of them would still be lit by the lanterns.

The figure lands on the stage floor, or runs on out of the room when the projector is aimed above
it. Walls and scenery cut the beam off where it meets them, but the pattern is not drawn on them.

Lasers do not appear in the plan views, which draw fixtures and their aim rather than what is in
the air.
