# Fixture and stage model brief

What a 3D model has to be for ToskLight's visualizer to use it, and what each model in the
standard set should look like. Hand this document to whoever builds the models — including a
later session of an AI assistant — together with the sentence "build the models described here".

Everything in part 1 is a hard requirement enforced by code. Everything in part 2 onwards is the
shape and size each model should have; treat those dimensions as targets, not tolerances, and
prefer a real product's measurements when one is being reproduced.

---

## 1 — The import contract

### 1.1 File format

- **glTF 2.0 binary (`.glb`), self-contained.** One file. No `.bin` sidecar, no external image
  files: any `uri` on a buffer or an image is rejected by the fixture-package validator.
- One model per fixture, stored in its package as `assets/model.glb`, with the profile's
  `model_asset` field set to `assets/model.glb`.
- Set `model_units` to `metres` in the profile when the GLB is authored at real-world size, which
  is what this brief asks for.
- Hard limits: **120,000 triangles per model** (the reader refuses more), 64 MiB per package
  entry, 32 entries per package. A lamp body should land between 500 and 8,000 triangles — it is
  a prop seen from ten metres away, next to beams that are the actual subject.

### 1.2 What the reader takes, and what it throws away

Taken:

- Node hierarchy, node names, node transforms (`matrix`, or `translation`/`rotation`/`scale`).
- Mesh primitives with **mode 4 (triangles) only**. A primitive drawn as lines or points makes
  the reader skip that part and report it.
- `POSITION` and `NORMAL`, both **float** accessors. A missing `NORMAL` is tolerated and comes out
  flat-shaded upward, so supply normals.
- Indices as unsigned byte, short, or int. Non-indexed primitives are accepted.
- `pbrMetallicRoughness.baseColorFactor`, `roughnessFactor` and `metallicFactor` as the part's
  surface. A primitive with no material is drawn as a painted housing.

Thrown away — do not spend time on it:

- **Textures and UVs of every kind**, including base-colour, normal, roughness and emissive maps.
- Emissive factors, alpha modes, double-sidedness.
- Animations, skins, morph targets, cameras, glTF lights, extensions, extras.
- Sparse accessors are not read; use ordinary buffer views.

Colour and the two shading factors are therefore the whole surface. Model the difference between
a black PAR and a silver PAR as two `baseColorFactor` values, and the difference between a wool
drape and the aluminium hanging it as two roughness values — a rig in which fabric catches the
same highlight as bare metal reads as painted cardboard. `tools/stage_models/kit.py` keeps the
set's finishes in one table:

| Finish | Metallic | Roughness | Used for |
| --- | --- | --- | --- |
| metal | 0.9 | 0.32 | couplers, yokes, reflectors, chain, truss |
| paint | 0.35 | 0.55 | lamp housings, cabinets, cases |
| plastic | 0.0 | 0.65 | mouldings and feet |
| glass | 0.0 | 0.12 | lenses, tubes, filaments |
| diffuser | 0.0 | 0.72 | milky diffusion panes and LED faces |
| fabric | 0.0 | 0.96 | drapes, chain bags, clothing |
| skin | 0.0 | 0.80 | figures |

A material that names a colour and nothing else keeps the reader's painted-housing defaults
rather than glTF's own, which would make every untagged part rough bare metal.

### 1.3 Axes, origin, and rest pose

- **Metres.** glTF's Y-up, -Z-forward convention, which is also the visualizer's: **+X** stage
  right as the audience sees it, **+Y** up, **+Z** towards the audience.
- **A lamp points straight down (-Y) at rest.** Emitters aim along the fixture's local -Y with pan
  and tilt at zero. A model built pointing forwards will light the wrong wall.
- **The origin is the mounting point**: where the clamp or hook bolts to the truss, on the pan
  axis, with the body hanging below it. For a moving head that is the centre of the base's top
  face; for a PAR or a profile it is the yoke bolt. Where a fixture is hung on **more than one**
  point — a pair of clamps, a batten's row of them — the origin is the point *between* them, so
  every variant of a model drops into a rig in the same place. A floor unit's origin is floor
  level under the middle of its feet, which is the same rule.
- **The pan axis is the model's own +Y through the origin.** Anything that turns with pan must be
  modelled centred on `x = 0, z = 0`.
- **The tilt axis is the centre of the head's bounding box.** The reader takes every part
  classified as head, takes the centre of their combined bounds, and tilts about that point. Model
  the head so its trunnions sit at the centre of its bounding box, or the head will swing slightly
  as it tilts. A long-nosed head is the case to watch: balance it about the trunnions or accept a
  few centimetres of travel.

### 1.4 Node naming — what moves, and where the light comes out

The reader classifies each node by its **name**, case-insensitively, and children inherit their
parent's classification unless their own name overrides it:

| Name contains | Becomes | Moves with |
| --- | --- | --- |
| `head`, `lamp`, `tilt` | Head | pan and tilt |
| `yoke`, `arm`, `pan` | Yoke | pan |
| anything else | Base | nothing |

Use these names deliberately:

```
moving-base            (base plate, display, cable entry)
  square-base
moving-yoke            (both arms and their crossbar)
  yoke-left
  yoke-right
  yoke-center
moving-head            (everything that tilts)
  head-body
  lens-ring
  lens
```

Two traps:

- A static lamp's body must **not** be called `lamp`, or it will be classified as a head and will
  tilt. Name a PAR can `par-can` or `body`, and reserve `lamp` for a cell that genuinely tilts or
  for individual sources inside a head.
- A node that is referenced twice is reported and skipped. Keep the graph a tree.

**Name the surface the light leaves.** A second set of names, read the same way, tells the desk
where the beam starts:

| Name contains | Means |
| --- | --- |
| `lens` | a lens and the ring round it |
| `source` | an emitter plate or array on an LED fixture |
| `cell` | the cells of a blinder, a sunstrip, a batten |
| `diffuser` | a milky pane |
| `aperture` | a laser window, a hazer nozzle |
| `emitter` | anything else that emits |

The anchor is the centre of those parts across the face and the **front** of them along the aim
axis, because light leaves the front of a lens rather than the middle of the glass. On a PAR that
is the difference between the mouth of the can and a point a third of the way down inside it.

This matters more than it looks. A profile that does not describe its own emitter geometry — most
of the library, and everything imported from a patch sheet — has nothing to say about where its
lens is, and the fallback is the fixture's origin. In this set the origin is the **rigging point**,
so without an anchor every beam starts at the clamp, a whole lamp above the lens it should come
out of, and a moving head's beam swings about the clamp instead of about its own trunnions.

Two traps of its own:

- Do not put one of these words on something that is not emitting. A lens *barrel* called
  `lens-tube` drags the anchor a third of the way back up the lantern; call it `focus-barrel`.
- A flashtube is an emitting surface but must not be called `lamp`, which would make it tilt.
  `xenon-source` says both things correctly.

### 1.5 Scale reconciliation

The visualizer scales the model uniformly so its largest dimension matches the physical size in
the fixture profile (`width_millimetres`, `height_millimetres`, `depth_millimetres`). Author the
model at true size **and** fill in the profile's physical dimensions with the same numbers, and
the scale factor comes out at 1. If they disagree, the profile wins and the model is squeezed.

### 1.6 Blender export settings

- Scene units: metres, unit scale 1.0.
- Apply all modifiers; no n-gons is not required, but triangulate on export.
- Clear parent inverse and apply object transforms **except** the ones you want as node
  transforms — the reader bakes node transforms into the vertices anyway, so either is fine, but
  keep the object origins where section 1.3 says.
- File → Export → glTF 2.0: format **glTF Binary (.glb)**, +Y up, include **Selected Objects** if
  the file holds more than one model, Data → Mesh: Apply Modifiers, Normals **on**, UVs, Vertex
  Colors, Materials → **No textures needed** (export materials as "Export" so base colours come
  through), Compression **off** (Draco is not read), Animation **off**.
- Name objects exactly as section 1.4 requires before exporting; Blender object names become node
  names.

### 1.7 Packaging and checking

1. Put the file at `assets/model.glb` inside the fixture package, set `model_asset` and
   `model_units`, and build the `.toskfixture` per `.agents/skills/build-light-fixtures/SKILL.md`.
2. Open a show that patches the fixture and look at it:
   `npm run open:viz -- --port <desk port> --ambient 40 --fog 10 --zoom 0.3`.
3. Check, in this order: the lamp hangs the right way up; it is the right size next to a
   1.7 m-tall reference; pan turns the yoke and not the base; tilt turns the head without moving
   it through the air; the lens is at the end the light comes out of; and the beam starts **at
   that lens**, not at the clamp and not beside the lamp as the head tilts.
4. A model that fails to load never leaves a hole — the fixture falls back to its procedural
   proxy and the reason appears in the scene warnings. If a fixture looks like a plain box, that
   is what has happened.

---

## 2 — Shared modelling conventions for lamps

- **Colours.** Lantern black `#1A1A1C`, moving-head and LED grey `#2A313A`, silver/natural
  aluminium `#B8BCC0`, clear lens `#C8D2DC`, brass/gold accents `#8A7040`, reflector `#D2D6DA`.
  True black loses a lamp's own silhouette; anything with a modern cast housing gets the grey.
- **Every hung lamp carries its hardware**: a half-coupler at the mounting point and a short
  safety bond loop. Two boxes and a torus are enough; it is what makes a rig read as rigged.
- **One clamp or two.** A mounting face under about 380 across takes a single half-coupler.
  Anything wider or heavier takes a pair straddling the origin — and **every moving head takes a
  pair whatever its base measures**, because hung on one it swivels on the bar and a base with a
  turning head on the end of it sits on the clamp like a see-saw. Two clamps also fix the fixture
  in yaw, so only a single-clamp model records the swivel about the hanging bolt.
- **Every flown model ships twice**: once with its mounting hardware and once without, the second
  named `<model>-no-clamp`. A fixture is not always on the clamp it ships with — it goes on a
  floor base, on the venue's own bracket, or into a case — and the origin is identical in both, so
  a show swaps one for the other without moving anything. `kit.unrigged` writes the bare one from
  the rigged one; nothing is modelled twice.
- **The bracket has to work.** A lantern's frame is sized from the body, not guessed at: build
  the body about its own tilt bolts, measure how far it reaches from them, and put the crossbar
  far enough above that ninety degrees of tilt either way swings clear — which is the body's own
  depth, because at ninety degrees the depth is what is pointing up. Set the arms outside the
  widest part, including accessories such as barn doors and colour frames. A moving head gets the
  same treatment against its head's whole turning radius, since a head goes right over.
  `hang_from_bracket` and `_moving_head_frame` in `tools/stage_models/lamps.py` do this, and
  every static lantern's body position is a consequence of it rather than a number in the builder.
- **Lens ring.** Give every lamp a distinct ring or bezel around the front element in a slightly
  lighter colour than the body. It reads as the business end from a distance and helps an operator
  tell which way a lamp is facing in the plan.
- **Nothing is buried.** An LED emitter, a reflector cell, a lamp capsule or a speaker cone that
  sits flush with or behind the plate carrying it is invisible from every angle but dead ahead.
  Stand sources proud of their plate, cells proud of their tray, and drivers proud of their
  baffle; put a ring round a driver rather than a disc over it, and never cover a face that is
  the only thing identifying the product.
- **A part has to shade against what it sits on.** Colour is most of the model's surface, so two
  near-blacks touching are one shape. A speaker cone is a shade up from its cabinet and its dust
  cap lighter again; a lens ring is lighter than the body it rings. This is not decoration — it
  is the difference between a 4x12 and a black box.
- **A bracket is bolted to the lantern.** Arms stand outside the *widest* part, which is often an
  accessory nowhere near the tilt bolts, so a trunnion has to span from the arm in to the body's
  own width at that height. A bolt drawn at the arm alone floats in the gap beside the lamp.
- **Vents and handles** as shallow boxes, not as geometry-heavy detail. Never model screws.
- **No cables.** No tails, no stubs, no looms. A cable adds depth the profile's dimensions do not
  declare — which makes the visualizer scale the whole lamp down — and reads as a stray line in a
  plan. Safety bonds and load chain are rigging, not cable, and stay.

---

## 3 — The lamp set

Dimensions are width × depth × height in millimetres unless stated, with the lamp hanging in its
rest pose (pointing down). "Yoke" always means a U-frame with a tilt bolt each side.

### 3.1 Moving Head Profile

- Base 400 × 250 × 175, with a small display panel on one side and a powercon/DMX panel on the
  rear face.
- Yoke arms 420 tall, 70 thick, spaced to clear a head 250 wide.
- Head 300 × 250 × 480, turned rather than boxed: a domed back, a barrel, the raised step where
  the shutters sit, and a flare down to the nose, with a front lens Ø 100 in a raised ring. An
  ovoid is what a profile head actually looks like from ten metres; a crate is not.
- Overall height falls out of the yoke, which has to be long enough for the head to go right
  over: about 850 with a 480 head. Do not shorten the arms to hit a catalogue figure.
- Two clamps on the base, spread across it, with the origin between them.
- Nodes: `moving-base`, `moving-yoke` (`yoke-left`, `yoke-right`, `yoke-crossbar`),
  `moving-head` (`head-body`, `head-cheeks`, `lens-ring`, `lens`).

### 3.2 Moving Head Wash

As 3.1 but a shorter, fatter head: 320 × 280 × 330 with a large front lens Ø 210 in a wide, light
bezel, and a squat shoulder rather than a hemisphere at the back. Base 400 × 250 × 170. The
silhouette difference between a profile and a wash is the whole point — keep the wash visibly
stubbier, and let the lens ring do the talking, because at this length the body alone is a ball.

### 3.3 Moving Head LED Wash

Wash proportions, but the front is a flat plate of individual lenses rather than one element:
concentric rings of emitter domes Ø 46 standing **proud** of the sunk face and clear of the front
lip. Three sizes: 300, 400 and 500 across, with 3, 4 and 5 rings of 1/6/12/18/24.

Compute the ring radii from a constant pitch across the family rather than writing them down: the
outermost ring plus a dome's own radius has to stay inside the sunk face, and a table of literals
is exactly what lets a 500 end up with its outer ring hanging over the rim. Base and yoke are cut
to the head too — the 300 under the 500's base looks like a lamp bolted to somebody else's motor.

Name the plate `head-lens-array` (it contains `lamp`, so it tilts — correct here).

### 3.4 Fresnel with barn doors

- Body 260 × 300 × 300, a rounded rear cowl, a lens Ø 200 with the stepped Fresnel rings modelled
  as three concentric ridges.
- Yoke bolted at the sides, hanging bolt at the top.
- Four barn-door leaves on a front-mounted frame, each 220 × 5 × 200, hinged at the frame and
  splayed 25–35° so they read as barn doors and not as a box. Name them `barn-door-top`,
  `barn-door-bottom`, `barn-door-left`, `barn-door-right` — they are base parts and must not
  contain `lamp` in their names.
- A colour-frame runner slot at the very front.

### 3.5 PAR 64 long nose

Can Ø 205, length 400, in black and in silver as two models. **The can is hollow**: rolled tube
with a visible inner wall, closed at the back with a half-ball, open at the front. The sealed beam
— a pressed reflector with a capsule at its focus and a lens face over it — sits about 40 % of the
way down the can, so a long-nose PAR is mostly empty tube and an operator can see whether it has
a lamp in it at all. Four spring clips on the front rim, a bracket with a hanging bolt, and an
**octagonal** colour-frame runner: a square holder is the one detail that makes a modelled PAR
look like a torch.

### 3.6 PAR 64 short nose

Same hollow Ø 205 can at length 280, otherwise as 3.5. Black and silver.

### 3.7 PAR 56

Hollow can Ø 180, length 260. Black and silver. Slightly lighter rim than the PAR 64 family.

### 3.8 ACL / PAR 16 narrow

Small hollow silver can Ø 55, length 130, a single-cell bracket, a very short nose and a bright
lens; no clips and no colour frame at this size. These are usually rigged in bars of four; model
the single lamp and let the show place four of them.

### 3.9 LED PAR "pizza lamp"

Hollow PAR 56 short-nose can (Ø 180, length 230) with a source plate set inside it carrying
individual domed sources: an outer ring of 18, an inner ring of 12, one in the centre, each dome
standing clear of the plate and just proud of the rim. This is the fixture whose front looks like
a pizza, and the individual sources are the reason it does.

### 3.10 LED PAR x-in-1

Same hollow can, source plate carrying 18 multi-chip emitters Ø 20 in shallow reflector cups that
open through the rim rather than hiding behind it. One model serves 4-in-1 through 7-in-1; the
difference is inside the chip, not in the silhouette.

### 3.11 Flat LED PAR

Slim hollow disc: Ø 230, depth 95, with a moulded rear housing carrying the driver and a display,
a thin front bezel, and a face of 18 emitters standing out through the bezel. Short stubby
bracket, because these hang tight to the bar.

### 3.12 Sunstrip

Bar 1000 × 90 × 90 carrying 10 cells on 100 mm centres in **one row**. A cell is a pressed
parabolic reflector Ø 84 standing proud of the extrusion's underside with a flat linear capsule at
its focus — a small reflector, not the sealed beam of 3.13, and not a bulb. Not a bulb — a sunstrip has no mushroom-shaped
E27 lamps hanging under a stick, and drawing them that way is the single thing that makes the
model read as a string of festoon. Bracket at each end, pivoting. Name each cell `cell-1` …
`cell-10`, never `lamp-n`, and keep the whole bar a base part.

### 3.13 Blinder, 2 / 4 / 8 cell

- One cell: a **PAR 64 sealed beam** — a warm tungsten glass face with concentric ridges for the
  prisms the lens is moulded with, filling a bright chromed retaining ring, held by three spring
  clips. Not a bare capsule in a pressed cup: what a blinder actually carries is a row of PAR
  lamps, and the chrome rings are what make the fixture recognisable at a glance.
- 2-cell: cells side by side, frame 400 × 200 × 220.
- 4-cell: two by two, frame 400 × 400 × 220.
- 8-cell: four by two, frame 800 × 400 × 220.
- The lamps stand proud of the tray, not flush in it.
- Yoke on the long sides, mesh guard optional as a single low-poly grid.
- Name the rings `cell-n`, not `lamp-n`.

### 3.14 Strobe, single flash bulb

Body 420 × 200 × 260 with a shallow well in the underside, about 65 deep. In it:

- **One straight xenon tube** Ø 16 on the axis of the well, between two electrode blocks. A stage
  strobe this size is a single linear flashtube; a folded or U-shaped one belongs to a compact
  camera flash and reads as the wrong product.
- **A white reflector lining the whole well** — a plate across the back and a splayed liner up all
  four sides, not a silver floor at the bottom of a black slot. The flash comes out of the entire
  aperture, and a black-walled well is a hole in a box.
- The tube sits level with the bezel, and the well stays shallow. Set back behind a deep surround
  the tube is visible only from directly underneath, which is not where a rig is ever looked at.

Hanging bracket underneath. The window is an opening, not a pane: the reader keeps no
transparency, so modelled glass in front of the tube would simply hide it.

### 3.15 LED strobe

Body 500 × 130 × 200, front face a single flat white diffusion plane standing **proud** of the
housing inside a bezel, with a rear housing and a slim bracket. Inset flush into a solid body the
plane is simply not there, and it is the whole point of difference from 3.14: no well, no tube,
one flat lit face.

### 3.15b Mirror scanner

A scanner is **not** a moving mirror. It is a whole lantern — lamp house, gate, lens tube — bolted
rigid inside a chassis, with the only moving part a mirror hung in a gimbal under the nose. Model
the optical train pointing down out of the chassis, the fork turning about that optical axis, and
the mirror rocking in the fork at forty-five degrees at rest. Two sizes: a 240 × 420 × 230 chassis
over a 150 mirror, and a 190 × 330 × 180 chassis over a 110 one. The optics sit on the pan axis
with the chassis spread fore and aft around them, because the reader turns everything that pans
about the model's own vertical through the origin. Nodes: `scanner-chassis`, `optics-house`,
`optics-nose`, `mirror-yoke` (`yoke-cheeks`), `head-mirror`. Nothing may be called `lamp`.

### 3.16 LED strip, RGBCCT

Extruded aluminium profile 50 × 45 in section with a milky diffuser running the full length.

**No bracket.** A batten is not hung in a yoke like a lantern: the clamps bolt through its own
extrusion, so the profile's top face *is* the mounting face and the origin sits on that bolt line.
Every other lamp here gets a frame because it has to be aimable; a strip is fixed where it is
bolted. One clamp about every metre, spread symmetrically so the origin stays the middle of the
strip. One model per length: **500, 1000, 1500, 2000, 2500, 3000 mm**. Keep the section identical
across the six so they read as one product family.

### 3.17 Hazer

Chassis 550 × 350 × 300 on four rubber feet, a fan grille on the rear face, a nozzle 80 long
projecting from the front at the top, a tank visible on one side, and a small control panel. Not
a hung fixture: no yoke, no clamp.

---

## 4 — Truss and rigging

The visualizer already draws trusses procedurally from a venue object's size and declared point
count, so a modelled truss must match those proportions or the two will look like different
products in one rig.

- **Chords** Ø 50 aluminium, **braces** Ø 20, welded ladder pattern with a bay every 340 mm and an
  upright at each node.
- **Sections**: 2-point (ladder, chords 290 apart), 3-point (triangle, 290 sides, apex up),
  4-point (box, 290 × 290). Also a 1-point **pipe**: a single tube Ø 48.3.
- **Lengths**: 500, 1000, 1500, 2000, 2500, 3000, 4000 mm per section type, plus a 500 mm corner
  block for the 4-point.
- Conical or fork-and-spigot connectors at each end, modelled as a simple boss — no pins.
- Natural aluminium `#B8BCC0`; a black-anodised variant is a colour change only.
- Model the truss lying along **+X**, centred on the origin, so a show can rotate it into place.

Ground support and towers use the same 4-point section vertically, with a base plate 600 × 600
× 30 and a sleeve block.

### 4.1 Chain hoist

A one-tonne touring hoist hung the way a rig hangs it: motor up, hook down. Shackle and swivel at
the origin, body 430 × 200 × 250 with a carrying handle across the top and a gearbox boss on one
end, a fabric chain bag 200 × 190 × 330 under the body, and the load chain out of the other end
with a hook on it. The bag is what makes the model read as a hoist rather than as another black
box on the truss.

### 4.2 Load chain

A simplified 500 mm run of 8 mm chain: oval links with every second one turned into the other
plane, which is the only thing that makes a chain read as a chain rather than as a bead necklace.
Shipped as its own model because a chain is never the right length in a plan — a show stacks as
many as the drop needs rather than carrying a model per height.

### 4.3 Truss lift

A wind-up tower: a cross base 1800 across on four adjustable feet with A-frame braces up to the
mast, three telescopic square sections drawn part way out to 4.8 m, a winch with its drum and
crank at working height, hazard banding on the base section, and a two-prong fork at the top.
Drawn collapsed it reads as a speaker stand, and drawn fully out it does not fit next to anything
else in the review file.

---

## 5 — Stage and venue elements

### 5.1 Stage decks

Standard deck sizes **1000 × 500**, **1000 × 1000** and **2000 × 1000**, each at leg heights
**200, 400, 600, 800, 1000 mm**. A deck is a 40 mm-thick top in matt black with a visible aluminium
edge frame, and four legs Ø 48 with adjustable feet. Model the top and legs as separate objects
so a deck without legs can be stacked.

### 5.2 Railings

Guardrail 1000 high to suit each deck width: uprights Ø 40 at each end and every 1200 mm, a top
rail Ø 40 and a knee rail Ø 32, plus a 100 mm toe board along the bottom. Black.

### 5.3 Curtains

Wool serge drapes with **50% fullness**, gathered into folds about every 225 mm. Widths 2000,
3000, 4000 and 6000 mm; heights 3000, 4000, 6000 and 8000 mm.

Both faces are gathered, and neither the depth nor the width of a fold repeats on a short cycle.
Fabric hung at fullness pleats onto its webbing all the way through; a flat back gives the model
away the moment a rig is looked at from upstage or from the side, and two depths alternating are
as obviously mechanical as no folds at all. Six depths between 38 and 140 mm on the front, the
same sequence at 45 % three folds out of step on the back, and six fold widths averaging one so
the finished width is unchanged and only the spacing wanders.

Fabric finish throughout — the drape must be visibly less shiny than the truss above it. A top
webbing with tie tapes, and a visible hem weight at the bottom. Black, and a light grey variant
for a cyc.

### 5.4 Mirror balls

Diameters **200, 300, 400, 500, 750 and 1000 mm**. A sphere faceted with flat 25 mm square mirror
tiles laid in rings — the facets must be flat quads, not a smooth sphere, because the flashes come
from the flat faces. A hanging eye and a 250 mm drop chain at the top. Mirror tiles near-white and
fully reflective; the gaps between them dark.

Model a matching **mirror-ball motor** as a 120 × 120 × 90 box with a rotating hook.

---

## 6 — Priority order

If the whole set cannot be built at once, this order gives a usable rig soonest:

1. PAR 64 short nose and long nose, black and silver — the backbone of any rig.
2. 4-point truss at 2000 and 3000, plus the pipe.
3. Moving Head Profile and Moving Head Wash.
4. Blinder 4-cell, ACL, Fresnel with barn doors.
5. LED PAR x-in-1 and flat LED PAR.
6. Stage decks, railings, curtain.
7. Sunstrip, strobes, LED strip family, hazer, mirror balls.
8. Moving Head LED Wash, LED PAR pizza lamp, PAR 56, remaining truss lengths and sections.

---

## 7 — Checklist before handing a model over

- [ ] `.glb`, glTF 2.0, self-contained, under 120,000 triangles.
- [ ] Metres, +Y up, lamp pointing **-Y** at rest.
- [ ] Origin at the mounting point — between them when there is more than one — on the pan axis.
- [ ] Hung on a pair of clamps if it is a moving head, or wide or heavy enough to need them.
- [ ] Shipped both rigged and as `-no-clamp`, with the same origin.
- [ ] Head geometry centred on its own trunnions.
- [ ] Node names follow section 1.4; no static part is called `lamp`.
- [ ] The emitting surface is named, so the beam starts at the lens rather than at the clamp.
- [ ] Base colour, roughness and metallic set per material; no textures relied upon.
- [ ] No cables anywhere.
- [ ] The bracket clears the body through its whole stated travel.
- [ ] Physical dimensions in the fixture profile match the model.
- [ ] Loaded in the visualizer and checked against section 1.7 step 3.
