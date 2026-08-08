# Stage Positions and Scenery

Stage provides an operator selection surface for the saved spatial model; the model itself is edited in **Show Patch**.

There is one set of fixture positions, and it is the 3D one. A 2D Stage is the renderer's plan of those positions, projected from whichever side the operator chooses in the Stage settings — above the rig, from the house, from upstage, or from either wing. It is not a second arrangement to be authored or regenerated, so a fixture moved in **Show Patch** is in its new place in every view at once.

## Position fixtures

Fixture positions and rotations are part of the patch: open **Show > Show Patch** and edit a fixture's placement for accurate meter-based values. Multi-patch instances can have their own physical positions while sharing logical programming.

Press **Preview Stage** in the Show Patch title bar to check the result visually while patching. The overlay is movable — drag its top grip to reposition it over the Patch.

![Full Stage window](../assets/screenshots/workflows/stage-window-2d.png)

![Full Stage settings](../assets/screenshots/workflows/stage-settings.png)

The Stage built-in offers **Select fixtures** for programming and **Navigate** to orbit and inspect without moving show objects; it does not edit positions.

In 3D, **Beam direction guides** adds a dotted aim line while a directional emitter is off. This applies to fixed Profiles, Fresnels, PARs, washes, and moving fixtures alike. Emitters marked as broad sources, such as strobes and Sunstrip-style fixtures, remain visibly distinct from their dark housings but do not receive an aim line. The full Stage and every Stage pane can disable these guides independently in their settings.

## Add scenery and models

Add scenery through **Show > Show Patch**. Choose a visual-only profile from the **Venue** manufacturer, such as Stage, Truss, Pipe, or Curtain. Patch assigns these objects IDs in the reserved `0.x` range (`0.1`, `0.2`, and so on) and does not ask for a DMX address. Position and rotate them through their patch placement like any other fixture, with **Preview Stage** open for visual feedback.

Stage does not maintain a second scene-asset collection. Standalone MVR geometry is reported as an import warning; recreate required scenery with Venue fixtures so it remains visible and editable through Patch and the Fixture Library.

## Visualization limits

Stage is a programming aid, not a photometric proof. Check real fixtures and DMX output for focus, color, beam, and intensity. Use **Follow Preload** to choose whether Stage displays the live scene or the active Preload scene.
