# Choose and import fixture types

Choose the exact profile revision and mode for each physical fixture before patching. A show embeds that revision, so later library edits or deletion leave the patched show unchanged.

In Control, open **Desk Setup > Shows & recovery > Open Fixture Library**. In Architect, open **Settings > Library**. Search by manufacturer, fixture, or mode; the list filters as you type. Search Options can restrict the fixture type.

A profile contains the fixture's identity and ordered modes. Modes describe its DMX channels and logical heads. Conventional **Dimmer PAR Can**, **Dimmer Profile**, and **Dimmer Fresnel** are separate profiles, each with 8-bit and 16-bit dimmer modes. Choose the physical lantern first, then its personality.

![Fixture-library manufacturers, modes, footprint, heads, and revision](../../assets/screenshots/workflows/fixture-library.png)

## Transfer fixture packages

Use `.toskfixture` to move a complete ToskLight profile, including its simulation data and assets, to another desk. Shipped fixtures use the same package format as imported and operator-authored fixtures.

1. Select the intended fixture revision and choose **Export fixture**.
2. Copy the resulting package to the other desk.
3. Choose **Import fixture** there and select the package.
4. Check the imported profile and mode before patching it.

Packages retain the stable fixture, mode, head, channel, function, split, and geometry identities. Importing identical content changes nothing. Changed content with the same fixture identity and manufacturer/name creates the next local revision. Reusing an identity for another fixture family is rejected.

When correcting a profile, export a working fixture as the template and retain its identity. Existing shows keep their embedded revision; they do not adopt the correction automatically.

## Import GDTF

1. Choose **Import GDTF** and select a `.gdtf` archive.
2. Review the preview's modes, channel widths, coarse/fine addresses, DMX breaks, functions, defaults, and physical endpoints.
3. Map any unknown attribute to the matching desk attribute. Create a custom attribute first if no suitable one exists.
4. Read the import limitations, then choose **Import fixture** or **Import and remember mappings**.

Importing an existing fixture creates a new library revision. Fixtures already patched in a show keep their selected revision. A failed import leaves the original data untouched and explains the problem in the import dialog or Fixture Library.

The preview distinguishes usable channel data from geometry, optical measurements, and wheel information retained only in the source archive. Unsupported executable relationships, including conditional function ownership or response curves the reader cannot represent, stop the import with an explanation. Retained measurements do not establish that a fixture has been physically calibrated.

![Preview a GDTF import](../../assets/screenshots/workflows/fixture-library-import.png)

## Retained source and MVR export

The imported GDTF archive travels with the profile in `.toskfixture` packages and native shows. Editing the profile retains the archive and its original association.

MVR export reuses the original bytes only while they match the exported profile's recorded import snapshot. An edited profile, an older import without an association, or a profile without source GDTF uses generated GDTF. The export reports its limitations; revision number alone does not establish a match.

Generated GDTF preserves supported functions, directed physical endpoints, units, channel widths, fine bytes, defaults, and supported physical Color data. Native calibration provenance, Position geometry, Zoom opening convention, scanner kinematics, and detailed models remain in ToskLight metadata. Unsupported curves or units produce warnings. The Fixture Library does not export standalone GDTF files.

See [MVR exchange](../../20-ToskLight-Control/10-Show-Setup/18-mvr-exchange.md) before handing a rig to another application. Use [Create and revise fixture profiles](10-edit-fixture-profiles.md) for authoring and [Shipped fixture profiles](16-shipped-fixtures.md) for the bundled library.
