# Stage Positions and Scenery

Stage provides an operator selection surface for the saved spatial model; the model itself is edited in **Show Patch**.

Control Stage provides a 2D plan and a lightweight 3D wireframe view. Both use the same saved positions. Changing a fixture in Show Patch updates every view; the 2D plan can be viewed from above, the house, upstage, or either wing.

## Position fixtures

Fixture positions and rotations are part of the patch: open **Show > Show Patch** and edit a fixture's placement for accurate meter-based values. Multi-patch instances can have their own physical positions while sharing logical programming.

Press **Open Stage Renderer** in the Show Patch title bar to inspect the changing rig in a dedicated PreViz Renderer window.

![Full Stage window](../../assets/screenshots/workflows/stage-window-2d.png)

![Full Stage settings](../../assets/screenshots/workflows/stage-settings.png)

The Stage built-in offers **Select fixtures** for programming and **Navigate** to orbit and inspect without moving show objects; it does not edit positions.

In 3D, **Beam direction guides** adds a dotted aim line while a directional emitter is off. The setting is off by default and is shared by the Desk's Stage window and panes. Fixed additional-screen Stage configurations retain their own setting. The option is not shown in 2D. Emitters marked as broad sources, such as strobes and Sunstrip-style fixtures, remain visibly distinct from their dark housings but do not receive an aim line.

## Add scenery and models

Add scenery in **Show > Show Patch** from the **Venue** manufacturer. Choose a stage deck, truss, curtain, stairs, or another scenic object, then set its available dimensions and color. These objects receive `0.x` fixture IDs and no DMX address.

Position and rotate them like other patch objects. Decks, stage elements, and stage stairs stand on their feet: Z 0 places the footprint on the floor, and height changes raise the deck. Most other objects use their centre. Open **Stage Renderer** to check placement.

Standalone MVR geometry is not imported. Recreate required scenery with Venue fixtures, or import a model in Architect.

A venue model that is not in the fixture library — the hall itself, a stage build, a set piece — is imported as a GLB file on the **Patch** sheet of ToskLight PreViz, with **Show all** on, with **+ Import 3D model**. It is still a Venue object: it lives in the show under the manufacturer **Imported models**, appears in Show Patch with its `0.x` ID, and is positioned, rotated, layered, and drawn like the shipped ones. See [Place your own venue models](../../20-ToskLight-PreViz/06-planning-and-transfer.md#place-your-own-venue-models).

Stage, the PreViz Renderer, and Architect show the same Venue objects. Hiding a patch layer hides its scenery and lights.

## Visualization limits

Use Architect for full rendered lighting, haze, and shadows; Control Stage remains a lightweight selection and aiming surface. Stage is a programming aid, not a photometric proof. Check real fixtures and DMX output for focus, color, beam, and intensity. Use **Follow Preload** to choose whether Stage displays the live scene or the active Preload scene.
