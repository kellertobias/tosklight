# Fixture and stage models

The model set described by `docs/engineering/fixture-and-stage-model-brief.md`, plus the
rest of what stands on a stage, built with Blender and exported as self-contained
glTF 2.0 binaries.

```
lamps/     63 files   moving heads, scanners, profile spot, PARs, LED PARs, blinders, strobes, strips, hazer
truss/     46 files   2-, 3- and 4-point sections, pipes, the corner set, a ground-support base,
                      a chain hoist, a half-metre of load chain, a wind-up truss lift
stage/     60 files   decks, railings, curtains, mirror balls and their motor
av/         6 files   projectors and a show laser
cases/      6 files   19" flight-case racks, open front and back with gear in them
backline/  14 files   drums, amps, DJ gear, PA, mic stands, keys, sax
people/     4 files   singer, guitarist, pianist, deejay
```

Anything that flies appears twice: `<model>.glb` with its mounting hardware and
`<model>-no-clamp.glb` without. Forty of the models are flown, so the set is 199 files.

`manifest.json` lists every model with its bounding size in millimetres, its triangle
count, where its origin sits, its node list with each node's classification, which node is
its truss coupler, and the hinges its hardware swivels on.

`docs/help/45-Visualizer/` shows the whole set with a render of each model; both the
renders and that catalogue page come from `npm run models:render`.

## Using one in a fixture package

1. Copy the `.glb` into the package as `assets/model.glb`.
2. Set `model_asset` to `assets/model.glb` and `model_units` to `metres`.
3. Copy `width_millimetres`, `height_millimetres` and `depth_millimetres` from the
   manifest entry into the profile — **not** the manufacturer's catalogue figures. The
   visualizer scales a model uniformly until its largest dimension matches the profile,
   using the smallest ratio of the three axes, so a profile that omits the coupler or the
   bracket draws the whole lamp smaller than the rig around it.

## The truss coupler

Everything that flies carries one node called `truss-coupler`, with the safety bond as
its child, and the manifest names it in `coupler_node`. Hide that node and the fixture
loses its rigging in one move; show it and it is mounted. Nothing else in a model
depends on it, so it can be switched per fixture depending on how the show has it
rigged — or the show can point at the `-no-clamp` file instead and never touch the tree.

A single half-coupler holds a mounting face up to about 380 across. Anything wider or
heavier — and **every moving head**, whatever its base measures — is hung on a pair
straddling the origin, because one clamp under a base with a turning head on the end of
it swivels on the bar. The origin is the point between the clamps, so the one-clamp and
two-clamp models drop into a rig in the same place. An LED strip has no bracket at all:
its clamps bolt straight through the extrusion, about one per metre.

## What the models assume

- Metres, glTF +Y up, and a lamp pointing along -Y with pan and tilt at zero.
- The origin is the mounting point for anything that hangs, and the point between them
  when a fixture is hung on more than one. Floor units — the hazer,
  decks, railings, cases, backline and figures — stand on the origin instead, trusses
  are centred on it, and a curtain or mirror ball hangs from it. The manifest states
  which, per model, and floor-standing objects face the audience at glTF +Z.
- A moving head's or scanner's moving geometry is centred on the pan axis, and the head
  is centred on its own trunnions, because the reader tilts about the centre of the
  head's bounding box.
- Only fixtures that really articulate have `yoke` and `head` nodes. A PAR, a Fresnel or
  a blinder carries a `hanging-frame`, and a figure's arms are `figure-sleeves`, because
  a node called `arm` or `head` would turn under pan and tilt.
- Every fixture names the surface its light leaves — `lens`, `source`, `cell`, `diffuser`,
  `aperture` — so the desk starts the beam there. A fixture whose profile carries no emitter
  geometry otherwise falls back to the model's origin, which here is the rigging point: the
  beam would start at the clamp and, on a moving head, swing about the clamp as the head tilts.
- Every bracket clears its own body through the travel the manifest states for it. The
  frame is measured off the body rather than chosen, which is why a lantern with barn
  doors hangs lower than one without: the arms have to be long enough to let it turn.
- Colour, roughness and metallic are the surface; there are no textures, and transparency
  is not read. Anything that has to be seen — a strobe's xenon tube, a blinder's reflector
  cups, the gear in a rack — sits behind an opening rather than behind modelled glass, and
  sources stand proud of the plate carrying them rather than sunk into it.
- No cables. Safety bonds and load chain are rigging and stay; power tails do not, because
  they add depth the profile never declares and the visualizer scales the lamp down for it.

## Truss

All sections share one geometry: 50 mm chords, 20 mm bracing, and 290 mm across the
outside of the chords whatever the section. Only the number of chords and where they sit
differ between the 2-, 3- and 4-point. Corner blocks come in the seven the rental stock
carries — 2-way, 3-way T, 3-way with a leg down, 4-way cross, 4-way with a leg down,
5-way and 6-way — for both the 3-point and the 4-point, with 500 mm arms.

## Rebuilding

```bash
npm run models          # rebuild every .glb and check it against the import contract
npm run models:verify   # check the shipped files without Blender
npm run models:open     # rebuild, then open the whole set as one .blend to look at
```

Building needs Blender on `PATH`. The builders live in `tools/stage_models/` and the
entry point in `tools/build_stage_models.py`; the models are generated, so change the
Python rather than editing a `.glb`. A full build also deletes any `.glb` that the
builders no longer produce. `models:open` writes a throwaway review file under
`.artifacts/tmp/`, one collection per model, every origin on `z = 0` — the grid floor
stands in for the truss, so a lamp modelled the right way up hangs below it.

The set is about 12 MB. Mirror balls are faceted with individually modelled flat tiles,
which is what makes them flash; each tile is a single quad on a dark core sphere, since
a tile is only ever seen from outside the ball.
