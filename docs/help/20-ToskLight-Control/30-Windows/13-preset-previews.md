# Preset previews

A Color or Position Preset recorded as a [Color Intent](../20-Programmer-and-Cues/05-color-intent.md) or as Pan/Tilt angles
or a Target draws its own small preview in the tile's picture box. The preview is read from what
the Preset stores; it never recalls the Preset, changes the Programmer or depends on the current
output, and it follows every Record and Update of the Preset as soon as it is stored. It looks the
same in the software-only and the hardware-connected layout. A Mixed Preset shows its colour, or
its aim when it holds no colour.

- **Color** shows the colour the Preset asks for, at full brightness: Intensity is a separate
  family, so a dim blue still reads blue and only a requested black reads black. White Blend and
  its white target are included. A Preset holding several different colours shows one segment per
  distinct colour, side by side in hue order, never a mixed average; a colour spread, such as a
  rainbow across a Group, shows the colours it passes through. At most six segments are drawn;
  with more, the first and last are kept and the rest are sampled evenly. UV without visible light
  is drawn violet, and a Direct colour without a known appearance is drawn hatched grey.
- **Position** shows a small square with up to ten dots, one per distinct aim: Pan across and
  Tilt up for angles, or Stage X across and upstage up for a Target. The highest and lowest aim on
  each axis and the outermost corners are always drawn; the remaining dots are spread evenly
  through the rest of the aims, so a dense cluster keeps more dots than a single outlier. The square keeps the true shape of the
  spread, so a line of fixtures is drawn as a line. When a Preset mixes angles and Targets, the
  kind most fixtures use is drawn.

An icon or button colour chosen in the Preset's button settings, or stored with the Preset in the
show, replaces the automatic preview and keeps the tile as it was. **Automatic icon** in the button
settings removes the chosen icon and colour again. Presets without a programming intention, such
as a Color Preset that only holds a colour-wheel slot, Mixed Presets without colour or aim, and
Intensity and Beam Presets keep the plain tile. Intensity has no preview because the tile already reports
whether the Preset is active, and a level glyph could not tell a stored level from the output.

See [Presets](../20-Programmer-and-Cues/23-presets.md) for recording, recall, and live source links.
