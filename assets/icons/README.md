# ToskLight icon set

This directory contains an original, from-scratch icon family for lighting-desk controls, show content, and fixture types. The artwork does not trace or reproduce a third-party icon set.

## Visual contract

- 64 × 64 SVG canvas
- white artwork (`#fff`) on a transparent background
- primarily 3 px rounded outline strokes
- simple forms intended to remain legible at compact control-surface sizes
- lowercase kebab-case filenames grouped by purpose
- an accessible `<title>` in every SVG

The files use fixed white rather than `currentColor` to preserve the requested delivery appearance. A consuming UI may replace `#fff` with `currentColor` during its asset pipeline if theme tinting is later required.

## Catalog

### Position

`down`, `up`, `down-fan-out`, `down-fan-in`, `up-fan-out`, `up-fan-in`, `down-cross-1`, `down-cross-2`, `up-cross-1`, and `up-cross-2`.

Fan icons use three position paths. Cross 1 is a tighter two-path crossing; Cross 2 is a wider crossing with end-reference bars.

### Gobos

`open`, `ring`, `line`, `cross`, `dot`, `dot-line`, `dots-floral`, `stars`, `flower`, and `triade`, plus six additional designs: `spiral`, `triangle`, `grid`, `burst`, `crescent`, and `breakup`.

`open` is a fully clear aperture. `ring` is an outlined annular shape.

### Beam size

`narrow`, `mid`, and `wide`.

### Prism

`3-facet`, `5-facet`, `rotate-left`, and `rotate-right`.

### Flash

`1`, `3`, and `1-rnd`.

### Miscellaneous and instruments

`microphone`, `drums`, `guitar`, `piano`, `truss-segment`, and `stage`.

### Functionality

`dynamics`, `timecode`, `schedules`, and `macros`. Timecode uses an original layered-track and playhead motif familiar from timeline-based editing without copying a specific NLE.

### Fixture type

`fresnel-barn-doors`, `profile-dimmer-lamp`, `parcan`, `acl-set`, `4-blind`, `blinder`, `parcan-short`, `blower`, `hazer`, `strobe`, `strip-light`, `laser`, `profile-moving-light`, `wash-moving-light`, `led-wash-moving-light-lenses`, `projector`, and `led-wall`.

These are source SVG assets. They are not automatically bundled by the control UI, and ToskLight fixture-package stage icons currently use raster formats. Import or rasterize an individual icon explicitly when integrating it.

Every fixture-type icon is drawn as a side elevation. Moving-light outputs point up-right; suspended conventional lamps point down-left; other optical outputs retain a clear side-facing direction. Multi-cell fixtures show each lamp body in profile.

Composite fixture icons draw lamp bodies behind their foreground arm or bracket. A transparent mask removes hidden fixture strokes and leaves a small clear separation gap around the mount.

### Fixture base

`moving-light-base` provides a compact rounded floor/base body. Its separate slender arm has a rounded top, squarer lower corners, no pivot mark, and a clear mounting gap above the base. `overhead-bracket` is a narrow top-mounted bracket with subtle top corners, a strongly rounded lower end, and a pivot at the center of that lower radius. Their centered mounting positions are intended for attaching modular lamp drawings.
