# Show Setup and Patching

Prepare the physical Desk first, then build the portable show. Desk-local configuration includes screens, users, control inputs, network binding, and output behavior. The `.show` file contains fixtures, embedded fixture revisions, patch, stage placement, MVR data, and the programming that travels with the production.

Recommended order:

1. [Configure the Desk](01-desk-setup.md), then assign [Screens and Desktop Layouts](02-screens-and-layouts.md), [Inputs, Extensions, and Network Control](03-inputs-extensions-and-network.md), [DMX Output and Universe Routes](04-dmx-output.md), and [Operators, Sessions, and Recovery](05-users-sessions-and-recovery.md).
2. Create, load, or recover the production under [Shows, Revisions, and MVR](10-shows-revisions-and-mvr.md).
3. Import or create the required [Fixture Types and GDTF](11-fixture-types-and-gdtf.md).
4. [Patch Fixtures and Scenery](12-patch-fixtures-and-scenery.md), including fixture IDs, modes, splits, multi-patch, and deliberately unpatched fixtures.
5. Set [Stage Positions and Scenery](13-stage-positions-and-scenery.md).
6. Use [Attribute Reference and Activation Examples](14-attribute-reference-and-activation.md) when mapping an unfamiliar fixture concept into the Programmer.
7. Connect a follow system under [Tracking with PosiStageNet](15-tracking-with-posistagenet.md) when the show is tracked.

MVR is whole-rig interchange; GDTF describes fixture types inside or alongside that interchange. ToskLight currently exposes MVR import as a new show and MVR export. Merge-into-current-show support is not yet an operator workflow.
