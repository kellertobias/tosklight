# Architect scripting and integration reference

This reference covers fixture-package scan scripts, particle scripts, physics actions, and Architect MCP tools. Operators configure and preview these fixtures through the [Architect manual](../help/10-ToskLight-Architect/index.md).

## MCP tools

The MCP server edits the show that is open in the Architect, the same way its own windows do. Each
edit is written against the revision it read, so a change you make in a window at the same time is
refused, not overwritten. Fixtures are named by the number an operator says out loud; a Venue object
is named by its `0.N` number (pass `"0.10"` as text, because the number 0.10 is 0.1). Anything
outside what the Architect allows: a size outside a profile's range, a DMX address on a Venue
object, a source for a server that is not there: is refused with the reason.

**Fixtures and Venue objects**

* `search_fixture_library`: profiles by manufacturer, name or mode, with their ids, modes, patch
  policy and Venue kind.
* `list_fixtures`: every fixture and Venue object, with placement, patch and layer. A Venue object
  also reports its kind, its size in metres, which measurements can be set, its colour, a chain's
  ends and its model scale.
* `add_fixture`: adds a fixture or a Venue object. Name the profile by `profile_name`, plus
  `manufacturer` when two makers use the name, for its newest revision and first mode; or pass the
  ids `search_fixture_library` returns. A Venue object gets the next free `0.N` number and no DMX
  address. `size_metres`, `colour`, `chain_mode` (or `chain_top` and `chain_bottom`) and
  `model_scale` set its size and look as it is placed.
* `remove_fixture`, `set_fixture_placement`, `set_fixture_shaper_and_gel`, `set_fixture_identity`,
  `set_fixture_patch`, `add_multipatch` and `assign_position_master`: edit one fixture. Nothing
  else about the fixture changes.
* `set_venue_size`: sets a Venue object's width, height or depth in metres. Only the measurements
  its profile makes adjustable can be set, within the profile's minimum and maximum, as in the patch
  sheet.
* `set_venue_options`: sets a Venue object's colour as `#RRGGBB`; `null` gives it its kind's own
  material back. On a chain it also sets the ends: `chain_mode` is `plain`, `motor_top` or
  `motor_bottom`, or set `chain_top` and `chain_bottom` one at a time.
* `set_model_scale`: draws an object at 0.01 to 100 times the size it was built at; `null` draws it
  at its built size.
* `list_layers`, `remove_layer` and `set_fixture_layer`: list the patch layers, empty one onto
  another, and move one fixture. `save_layer` works on a desk only; the Architect has no route that
  creates or renames a layer.

**Media layout**

These tools edit the Media workspace's layout. A desk has no media layout, so there they are
refused.

* `get_media_layout`: media servers, sources, LED module types, surfaces with their sections, and
  projectors, each with its id and revision.
* `put_media_server`, `put_media_source`, `put_led_module_type`, `put_media_surface` and
  `put_media_projector`: create an object when no `id` is given, or update the one named. An
  update changes only the fields given. A surface's `sections` replace its sections. Each section
  is a `projection_screen`, `tv` or `led` section, and an LED section without `occupied_cells`
  fills every cell of its grid.
* `delete_media_object`: deletes one object by `kind` and `id`. Deleting a server removes its
  sources; a surface that showed one keeps its fallback image.


## Laser profile fields

A fixture is treated as a laser when its profile's fixture type says so: anything containing
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
| `points_per_second` | Scanner speed: the figure a manual quotes as "30 kpps" |
| `divergence_milliradians` | How much the beam has spread by the time it lands |
| `aperture_millimetres` | Beam diameter at the output window |
| `optical_power_milliwatts` | Total output with every colour at full |

The scanner speed matters more than it looks. Together with how many points a figure has, it
decides how many complete passes of that figure fall inside one frame: which is the difference
between a pattern that reads as solid and one that reads as a travelling dot. It is also why the
same projector looks dimmer on a large pattern than on a small one.

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

**A point carries the colour of the run that arrives at it**, not the one that leaves it. That is
the usual convention, and it is what makes blanking expressible: a black point means the scanner
travelled to it with the light off, which is how a pattern jumps between figures without drawing
the join. Without it every corner would be joined to every other one.

**`amount` is a brightness as well as a timing.** The scanner moves at a fixed speed, so a run it
crosses quickly receives less light per metre than one it lingers on. That is why the corners of
a real laser figure are visibly brighter than its edges, and drawing a figure with even timing
along an uneven path will look like a wireframe rather than a laser. The percentages do not have
to add up: they are rescaled: and a script that names no timing at all is drawn as an even
sweep.

**The module keeps its state between frames.** A pattern that rotates or animates should
integrate `input.elapsed` rather than reading the clock, so it turns at its own rate from wherever
it happens to be. Each patched fixture gets its own copy, so two projectors of the same model
running the same script do not animate in lockstep.

A script has no host beyond `input`: no files, no network, no timers, no console, no imports. It
gets a few milliseconds per frame and is stopped if it runs longer. Anything that goes wrong ,
a script that will not compile, one that throws, one that returns something unusable, one that
overruns: leaves that laser dark with the reason on screen. It never takes the frame with it.

## Editing a script without rebuilding a package

Use loose scripts while authoring a scan engine. A packaged script travels with the show.

Point the Visualizer at a directory of loose scripts instead:

```bash
viz-renderer --laser-scripts ~/laser-patterns
```

A file named after a fixture's profile ID: `<profile id>.js`: replaces whatever that fixture's
package carries. Save the file and the change appears; delete it and the packaged engine comes
straight back. Nothing about the show moves: the fixture, its patch and its profile are untouched
throughout. When the pattern is right, it goes into the package and the loose file comes out.

`TOSKLIGHT_VIZ_LASER_SCRIPTS` sets the same directory.


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
typed Off, trigger, hold, release or retrigger state. Version 1 rejects physics-backed debris results.

## Rendering and restart behavior

Particles are emissive geometry in the normal opaque/depth pass. Scenery in front hides them, and
bright flame and sparks feed the existing bloom. They do not cast dynamic light or shadows.

A live Visualizer restart starts a fresh simulation from current authoritative DMX. It does not
claim to reconstruct individual pre-restart particles. Deterministic capture uses stable identity,
seed and timeline time so repeated product captures match without turning live particle history
into portable show state.


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
