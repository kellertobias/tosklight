# ToskLight icon set

This directory contains an original, from-scratch icon family for lighting-desk controls, show content, and fixture types. The artwork does not trace or reproduce a third-party icon set.

## Visual contract

- 64 × 64 SVG canvas
- black artwork on a transparent background
- editable originals primarily use 3 px rounded outline strokes
- generated `.expanded.svg` siblings use only filled geometry
- simple forms intended to remain legible at compact control-surface sizes
- lowercase kebab-case filenames grouped by purpose
- an accessible `<title>` in every SVG

Every root SVG has the shared `tosklight-icon` class, a black `color` presentation attribute, and visible strokes and fills expressed as `currentColor`. Inline consumers can recolor an icon with ordinary CSS:

```css
.tosklight-icon {
  color: #5ee7f0;
}
```

CSS outside an `<img>` cannot style the contents of the referenced SVG. Inline the SVG or load it as a component when runtime recoloring is required. Literal black and white values inside source masks are geometry operations and do not render as icon color.

Run `npm run icons:contact-sheets` after changing the catalog. It generates a filled-geometry `name.expanded.svg` beside every editable `name.svg`, then renders every per-group PNG plus `complete-library.png` under `.artifacts/generated/icon-contact-sheets`. An ignored mirror under `docs/help/assets/icon-contact-sheets` makes the same generated sheets available to in-app Help, the manual, and the static Pages export; contact-sheet PNGs are not committed. Do not edit `.expanded.svg` files directly. The generator skips them as inputs, removes orphaned derivatives, resolves source strokes, transforms, patterns, and transparent mounting cutouts, and Boolean-unions overlapping painted paths into one scale-stable compound path per icon.

## Catalog

### Position

The 25 arrow layouts include:

- four-arrow `down`, `up`, fan-in, fan-out, and cross families;
- six-arrow `left-right-in`, `left-right-fan-vertical`, and `top-bottom-fan-horizontal`;
- parallel `down-left`, `down-right`, `up-left`, and `up-right` diagonals;
- target-converging variants for all four diagonal directions: one ending at an exact shared point and one retaining a small target spread.

Cross 1 swaps each local pair of arrows. Cross 2 swaps the complete left and right two-arrow banks.

### Position beams

`position-beam` contains a one-to-one counterpart for every arrow layout. Each filled beam uses the same source and destination coordinates, tapering from a narrow source to a wider destination. The beam silhouettes receive a subtle edge expansion for stronger legibility.

### Gobos

`open`, `ring`, `line`, `cross`, `dot`, `dot-line`, `dots-floral`, `star`, `stars`, `flower`, and `triade`, plus `spiral`, `triangle`, `grid`, `burst`, `crescent`, `breakup`, `jungle-vines`, `jungle-breakup`, and `radioactive`.

`open` is a fully clear aperture. `ring` is an outlined annular shape.

### Laser shapes

`waveform`, `circle`, `line`, `sine`, `dashed-line`, and `double-line`. Each projected shape uses the standard 3 px stroke, with 1 px projection rays originating at the horizontal center and 30% canvas height.

### Beam size

`narrow`, `mid`, and `wide`.

### Prism

`3-facet`, `5-facet`, `rotate-left`, and `rotate-right`.

### Flash

`1`, `3`, and `1-rnd`.

### Miscellaneous and instruments

`drums`, `guitar`, `piano`, `truss-segment`, and `stage`.

### Functionality

`dynamics`, `timecode`, `schedules`, `macros`, `sine`, `cosine`, `linear-plus`, `linear-minus`, `square`, `pwm`, `random`, and `keyframe-based`. Keyframe-based uses four filled points on its curve. Timecode uses an original layered-track and playhead motif familiar from timeline-based editing without copying a specific NLE.

### Fixture type

`fresnel-barn-doors`, `profile-dimmer-lamp`, `parcan`, `acl-set`, `blinder`, `parcan-short`, `blower`, `hazer`, `strobe`, `strobe-lines`, `strobe-squares`, `strobe-squares-flash`, `strip-light`, `laser`, `scanner`, `profile-moving-light`, `wash-moving-light`, `led-wash-moving-light-lenses`, `projector`, and `led-wall`.

The files without `.expanded` in their names are the editable source SVG assets. Their generated `.expanded.svg` siblings are the scale-stable integration assets. They are not automatically bundled by the control UI, and ToskLight fixture-package stage icons currently use raster formats. Import or rasterize an individual icon explicitly when integrating it.

Every fixture-type icon is drawn as a side elevation. Moving-light outputs point up-right; suspended conventional lamps point down-left; other optical outputs retain a clear side-facing direction. Multi-cell fixtures show each lamp body in profile.

Composite fixture source icons draw lamp bodies behind their foreground arm or bracket. A binary source mask describes the small transparent mounting gap; generation turns that mask into the final cut-out path, so expanded icons contain no mask or shadow-like compositing.

### Fixture base

`moving-light-base` provides a compact rounded floor/base body. Its separate slender arm has a rounded top, squarer lower corners, no pivot mark, and a clear mounting gap above the base. `overhead-bracket` is a narrow top-mounted bracket with subtle top corners, a strongly rounded lower end, and a pivot at the center of that lower radius. Their centered mounting positions are intended for attaching modular lamp drawings.
