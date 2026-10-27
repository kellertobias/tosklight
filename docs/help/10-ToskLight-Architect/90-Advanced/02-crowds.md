# Crowd areas

Add **Venue > Crowd Area** for a visual-only audience. It has no DMX address. Choose Sitting,
Standing still or Dancing, each at Sparse, Medium or Dense density.

Set **Info > Placement > Size > Width / Depth** in Architect, or **Footprint width / depth** in
Control's Show Patch. The footprint changes independently from human height. Crowd Areas can
range from 1 to 250 metres on each side.

The chosen footprint, mode and density produce a stable layout after saving or restarting.
Moving another fixture does not reshuffle the audience.

Draft and Standard omit crowds. High and Ultra draw them with a performance limit. Use
**Quick Settings > Crowd amount** to reduce the visible audience from 100% to 0% on this renderer.
It does not rewrite the show's authored area or density.

## Detailed people

**Quick Settings > Features > Detailed people** adds three-dimensional bodies, shaped clothing, hair and facial features in Ultra.
Architect exposes this switch in its Visualizer settings too. It defaults
to off, preserving the existing silhouettes. Clothing stays dark and skin and clothing respond
to the show lights; the bodies have no artificial glow or bright contour lines.

The switch remains available in Draft and 3D Lines. Those live views stay lightweight, while
**Render 4K** freezes the requested detail and uses it for the native Ultra image.
Changing the switch after starting the image does not change that frozen image.

Crowd amount still controls population. Detail does not move or resize people, change a crowd's
seed, or increase its live quality budget. Sitting people retain their seated height. Detailed 4K audiences cost more to render at full population; reduce Crowd amount when a smaller population is
appropriate. Plans keep their footprint rectangles, and High keeps the lightweight silhouettes.

Scene snapshots retain their simple crowd silhouettes. The Detailed people switch applies to
native Ultra and [4K images](03-snapshots-and-4k-images.md#render-a-4k-image).
