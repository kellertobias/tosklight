# ToskLight PreViz

ToskLight PreViz combines two surfaces: the **PreViz Renderer**, which draws the live 3D picture, and the **PreViz Rig Editor**, which prepares a portable show and venue before a desk is involved. The Desk and Rig Editor use the same `.show` format and its embedded fixture-profile revisions. Transferring a show between them copies the document; it does not create two simultaneous writers.

## Choose a scene source

PreViz can render from:

- a running ToskLight Desk, using the Desk's show data and real Art-Net or sACN values;
- a show open in the PreViz Rig Editor, using its temporary fixture preview or configured live DMX inputs;
- a directly opened portable show file; or
- a writable copy of the bundled Demo Show.

The Rig Editor's preview does not require a DMX route. A standalone renderer following live desk output does: configure a real Art-Net or sACN route, including explicit loopback unicast when both products run on one computer.

## Open PreViz

Start the PreViz Rig Editor. On a new installation it opens a writable copy of the Demo Show; otherwise it reopens the recent document. Use **Open Viz** in the editor to launch the renderer against that planning document. Use **Load from Desk** when a discovered Desk should be copied into the editor, or **Load from Visualizer** on the Desk to copy the editor document back.

Discovery uses ToskLight's local-network show service. It is optional, and it is separate from the CITP service used for Media Server libraries and previews. A missing discovery button does not stop either application from opening an ordinary `.show` file.

## What PreViz draws

Every fixture has a body. A fixture package may carry its exact model; otherwise PreViz chooses a deterministic packaged fallback from the profile metadata. Movement uses the profile's physical limits and rates. Ordered wheels visibly cross intermediate slots, lasers and particle effects use bounded package scripts, Crowd Areas populate deterministic silhouettes, and Venue fixtures provide scenery and media surfaces.

The Rig Editor patches fixtures, edits their installed geometry, configures venue and media surfaces, and previews fixture values. It consumes package-owned models, gobos, wheel definitions, laser engines, and effect scripts; it does not author those package assets or fixture profiles.

## Continue

- [Default Models](01-default-models.md) and the [Model Catalogue](02-model-catalogue.md)
- [Snapshots and Blender](03-snapshots-and-blender.md)
- [Driving PreViz from the Desk](04-desk-view-control.md)
- [Lasers](05-lasers.md) and [Particle Effects](07-particle-effects.md)
- [Plan a Rig and Move It](06-planning-and-transfer.md)
- [Crowd Areas](08-crowds.md), [Media Surfaces](09-media-surfaces.md), and [Physics-driven Scenery](10-physics-scenery.md)
