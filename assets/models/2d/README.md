# 2D drawings of the shipped models

A line drawing of every model in `../manifest.json`, in three orthographic views, as a
starting point for the fixture and stage symbols the PreViz plan and elevations draw.
They are generated from the GLB files and are meant to be edited by hand.

```
<group>/<model>/top.svg     seen from above
<group>/<model>/front.svg   seen from the front
<group>/<model>/side.svg    seen from the left
```

## What is in a drawing

Each file is plain SVG. One user unit is one millimetre, and `width` and `height` carry the
same size in `mm`, so a drawing opens at its real size. The page's y axis runs down, as SVG's
does, so up on the page is up on the model in the front and side views.

- `silhouette` — the area the model covers, as one filled path. A loop inside another is a
  hole, so the gap between a yoke's arms stays open.
- `lines` — every edge you can see: the outline, creases where faces meet at more than 35°,
  and the edges where one part meets another. Lines hidden behind the model are left out.
  They are grouped by the part they belong to:
  - `base` — what is bolted to the truss or stands on the floor
  - `yoke` — what turns with pan
  - `head` — what turns with pan and tilt
  - `hardware` — the hanging frame and the truss coupler, in a lamp that turns in a hanging
    frame (see below)
- `origin` — a red cross at the model's origin, which is its mounting point: where the clamp
  meets the truss, or the floor under a floor-standing model. Keep the drawing aligned to it;
  the plan places a symbol by that point.

A moving head is drawn as the CAD shows one: pointing forward from above, and pointing down
in the front and side views, except that the LED washes (`moving-head-led-wash-300`, `-400`,
`-500`) look straight at the viewer in the front view. Some models are turned in their hanging
frame so a plan reads them at a glance, the hardware staying where it hangs:

- PARs and Fresnels point forward in the top view, so they show their length rather than a
  round face.
- Blinders (every cell count and layout), the flat LED PAR, the strobes (`led-strobe`,
  `strobe-xenon`) and the flood face the audience in the top and front views, and their side
  view file leans 20° from vertical toward the viewer. That lean is only how the file reads on its
  own: the CAD turns the body to the fixture's bracket angle (see below).

The same rules apply to a model's `-no-clamp` variant.

### Lamps that turn in a hanging frame

A lamp whose manifest records a `hanging-frame` swivel keeps its hanging hardware apart from the
body, so the CAD can turn the body by the fixture's bracket angle while the frame stays put:

- `silhouette` holds two paths: `silhouette-body` and `silhouette-hardware`.
- `lines` holds a `hardware` group beside `base`, `yoke` and `head`. Body lines are hidden only by
  the body and hardware lines only by the hardware; the CAD lays the hardware over the body and
  hides the body lines under it.
- The root element carries `data-hinge="x y"`: the point the body turns about, in the view's page
  millimetres.
- A drawing whose body is turned in its frame carries `data-bracket`: the bracket angle, in
  degrees, that the drawn pose equals in the Visualizer — a turn about the lamp's transverse
  axis, where a positive angle tips a face-down lamp's beam upstage. The face-forward side views,
  leaning 20° toward the viewer, carry `data-bracket="-70"`. Absent means 0: hanging as modelled.

In the CAD's side views the body is turned about the hinge from its drawn pose to the fixture's
configured bracket angle, 0 included, so it hangs exactly as the 3D Visualizer poses it: at 0 a
blinder hangs face-down like any other lamp, and its direction indicator leaves the face. Top and
front views keep their drawn poses. A drawing without a hinge or without the separate hardware
parts — for example one edited back into a single silhouette — is drawn as it is and never turned.

The views follow the Visualizer's stage axes: the model's +Z is toward the audience. The top view's
page y runs downstage, the side view's page x runs downstage, and the front view is seen from the
audience.

People are drawn as the audience figure the crowd uses (`assets/viz/crowd/audience-outline.json`),
scaled to each figure's height, so every person on a plan reads alike. The root element records
the pose in `data-pose`: `authored-home`, `moving-forward`, `moving-down` or `tilted`.

Screws, bolts, grilles, glass and textures are left out of the drawing, the same as in the
generated CAD projections.

## Editing

Open a file in any vector editor, keep the group ids, and redraw whatever reads badly:
simplify a busy crease, add a detail the mesh does not have, restyle the strokes. A drawing
with nothing inside a group can drop that group. Keep `hardware`, `silhouette-hardware`,
`data-hinge` and `data-bracket` together: drop any of the first three and the CAD stops turning
the body, drop `data-bracket` from a turned drawing and it turns from the wrong pose.

## Regenerating

```sh
npm run models:2d
npm run models:2d -- --only moving-head-profile --only par-64-short-nose-black
```

The generator records the checksum of every file it writes in `generated.json`. It only
replaces a drawing that still matches that checksum, so a drawing someone has edited is kept
and reported as `edited since it was generated, kept`. Pass `--force` to regenerate edited
drawings as well — that throws the edits away.
