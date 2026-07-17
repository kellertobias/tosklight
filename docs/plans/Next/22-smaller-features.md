# Fixture Sheet Color Dot Outline

Improve the Fixture Sheet's resolved-color indicator so dark output colors remain distinguishable from the desk's dark background.

Draw a thin light-grey outline around every fixture color dot. The outline must be present for black, dark saturated colors, white, transparent/no-color states, active Highlight, and any mixed or approximate color indication. It is a boundary treatment only: do not change the resolved fill color, introduce a glow, resize the fixture row, or use the outline as another state signal.

The outline should remain visible at supported display scaling and in both software-only and hardware-connected layouts. Use the existing theme vocabulary where possible, but keep sufficient contrast against the Fixture Sheet background and against very light dot fills.

## Acceptance criteria

1. Black and representative dark RGB colors have a clearly visible light-grey boundary.
2. The fill still matches the authoritative resolved fixture color.
3. White and bright colors remain legible without the outline dominating the dot.
4. Dot dimensions, row geometry, selection styling, and other Fixture Sheet state indicators do not move or change.
5. Component and visual coverage includes dark, bright, absent, and mixed color states at representative software-only and hardware-connected sizes.

