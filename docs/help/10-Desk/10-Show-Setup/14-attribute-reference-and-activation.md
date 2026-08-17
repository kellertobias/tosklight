# Attributes and Activation

ToskLight uses stable canonical attribute IDs for programming, Presets, Cues, feedback, and Stage
visualization. Fixture profiles retain their authored channel names and map them to those desk
attributes. The fixture profile remains authoritative for raw DMX ranges and safe functions.

The built-in default encoder assignments and activation groups are generated from the Desk code and
listed in the [Appendix](../../99-Appendix/02-default-attributes.md). They are defaults, not a
requirement: a show can arrange encoders and activation groups differently in **Show → Desk Setup
→ Programmer**.

## Fixture mappings

The mapping layer lets fixtures with different physical channel names program the same desk control.
For example, Cyan, Magenta, and Yellow filtration map inversely to canonical Red, Green, and Blue;
Cold White and Warm White map to White and Amber. The physical fixture-channel identity and its
authored range are retained. A fixture must not map two separately controllable channels on the same
logical head to one canonical attribute.

Some functions are typed actions rather than recordable values: Lamp On, Lamp Off, Reset, and
documented Fan actions are examples. Older attribute IDs remain readable for show compatibility,
but new fixture definitions use the current canonical IDs.

## Custom attributes

Use **Show → Desk Setup → Programmer → Attributes** to add a show-owned attribute for a capability
the built-in registry does not cover. Give it a namespaced ID such as `vendor.feature` or
`custom.feature`, then choose its label, value type, encoder placement, lifecycle, and activation
group. Retiring it removes it from new programming while preserving existing show data.

Use **Imported attribute names** when a fixture profile names an existing desk attribute differently.
Map that name to the existing attribute rather than creating a duplicate. These mappings are
Desk-local; a fixture revision keeps the mapping it already resolved.

## Activation

An activation group makes the supported members active together when one member changes. Missing
members are skipped for each fixture and logical head. A member belongs to one group only.

When a member changes, the Desk captures the linked values once from the current Normal, Blind, or
Preload context. They remain fixed in the Programmer. Changing the group layout affects future
activations only; it never rewrites recorded Cues.
