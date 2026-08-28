# ToskLight Architect

![ToskLight Architect application icon](../assets/branding/tosklight-architect.png)

ToskLight Architect combines CAD-based venue and rig planning with live visualization. Its **PreViz Renderer** draws the live 3D picture, while its **PreViz Rig Editor** prepares a portable show and venue before a desk is involved. Control and the Rig Editor use the same `.show` format and its embedded fixture-profile revisions. Transferring a show between them copies the document; it does not create two simultaneous writers.

## Choose a scene source

Architect can render from:

- a running ToskLight Control session, using the desk's show data and real Art-Net or sACN values;
- a show open in the PreViz Rig Editor, using its temporary fixture preview or configured live DMX inputs;
- a directly opened portable show file; or
- a writable copy of the bundled Demo Show.

The Rig Editor's preview does not require a DMX route. A standalone renderer following live desk output does: configure a real Art-Net or sACN route, including explicit loopback unicast when both products run on one computer.

## Open Architect

Start the PreViz Rig Editor. On a new installation it opens a writable copy of the Demo Show; otherwise it reopens the recent document. Use **Open Viz** in the editor to launch the renderer against that planning document. The CAD drawing is not a separate window: **CAD** is a screen in the sidebar beside Show, Patch, Venue, Effects and Media. Use **Open Window** to open another Architect window on the same show when the drawing and the patch sheet should be on two screens at once; every window edits the one open document, so a fixture patched in one appears in the other. The renderer stays the one window of its own. Use **Load from Desk** when a discovered Desk should be copied into the editor, or **Load from Visualizer** on the Desk to copy the editor document back.

Discovery uses ToskLight's local-network show service. It is optional, and it is separate from the CITP service used for Media Server libraries and previews. A missing discovery button does not stop either application from opening an ordinary `.show` file.

> [!danger] Missing graphic
> Add an annotated PreViz Rig Editor overview and a same-rig comparison of the principal 2D and 3D renderer views.

## What Architect draws

Every fixture has a body. A fixture package may carry its exact model; otherwise PreViz chooses a deterministic packaged fallback from the profile metadata. Movement uses the profile's physical limits and rates. Ordered wheels visibly cross intermediate slots, lasers and particle effects use bounded package scripts, Crowd Areas populate deterministic silhouettes, and Venue fixtures provide scenery and media surfaces.

The Rig Editor patches fixtures, edits their installed geometry, configures venue and media surfaces, and previews fixture values. It consumes package-owned models, gobos, wheel definitions, laser engines, and effect scripts; it does not author those package assets or fixture profiles.

## Continue

- [Default Models](01-default-models.md); the [Model Catalogue](../99-Appendix/01-model-catalogue.md) is in the Appendix
- [Snapshots and Blender](03-snapshots-and-blender.md)
- [Driving PreViz from the Desk](04-desk-view-control.md)
- [Lasers](05-lasers.md) and [Particle Effects](07-particle-effects.md)
- [Plan a Rig and Move It](06-planning-and-transfer.md)
- [Crowd Areas](08-crowds.md), [Media Surfaces](09-media-surfaces.md), and [Physics-driven Scenery](10-physics-scenery.md)
