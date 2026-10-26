# ToskLight Architect

![ToskLight Architect application icon](../assets/branding/tosklight-architect.png)

ToskLight Architect plans venues and rigs in its editor and shows them in a separate PreViz
renderer. Its `.show` documents also open in Control. A document loaded from or published to a
desk can keep rig edits synchronized; an independent file stays separate.

## Choose a scene source

Architect can render from:

- a running ToskLight Control session, using the desk's show data and real Art-Net or sACN values;
- a show open in the PreViz Rig Editor, using its temporary fixture preview or configured live DMX inputs;
- a directly opened portable show file; or
- a writable copy of the bundled Demo Show.

The Rig Editor's preview does not require a DMX route. A standalone renderer following live desk output does: configure a real Art-Net or sACN route, including explicit loopback unicast when both products run on one computer.

## Open Architect

The editor opens a writable Demo Show on a new installation, otherwise its recent document.
Use **Open Viz** for the separate renderer. The sidebar provides **Show**, **CAD**, **Patch**
and **Media**; **Settings** holds renderer, fixture-library and DMX configuration.
**Open Window** opens another editor window on the same document.

![Architect CAD workspace with the venue, rig views and Placement panel](../assets/screenshots/workflows/architect-cad.png)

Use [Plan a Rig and Move It](06-planning-and-transfer.md) for placement, walls, plan exports and
Control synchronization. Configure a real Art-Net or sACN route for live desk lighting, including
explicit loopback unicast when both applications run on one computer.

## What Architect draws

Every fixture has a body. A fixture package may carry its exact model; otherwise PreViz chooses a packaged fallback from the profile metadata. Movement uses the profile's physical limits and rates. Ordered wheels visibly cross intermediate slots, lasers and particle effects use their packaged simulations, Crowd Areas populate audience figures, and Venue fixtures provide scenery and media surfaces.

The Rig Editor patches fixtures, edits their installed geometry, configures venue and media surfaces, and previews fixture values. It consumes package-owned models, gobos, wheel definitions, laser engines, and effect scripts; it does not author those package assets or fixture profiles.

## Continue

- [Default Models](01-default-models.md); the [Model Catalogue](../99-Appendix/01-model-catalogue.md) is in the Appendix
- [Snapshots and 4K Images](03-snapshots-and-blender.md)
- [Driving PreViz from the Desk](04-desk-view-control.md)
- [Lasers](05-lasers.md) and [Particle Effects](07-particle-effects.md)
- [Plan a Rig and Move It](06-planning-and-transfer.md)
- [Crowd Areas](08-crowds.md), [Media Surfaces](09-media-surfaces.md), and [Physics-driven Scenery](10-physics-scenery.md)
