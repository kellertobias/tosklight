# Attributes and Activation

Fixture profiles map their channel names to shared desk attributes, so different fixtures use the same Color, Position, and other controls. Raw DMX ranges and safe functions remain defined by the profile.

Open **Show > Desk Setup > Preferences > Attributes & encoders** to arrange encoders and activation groups. The [Appendix](../../99-Appendix/02-default-attributes.md) lists their defaults.

## Fixture mappings

Use the Fixture Library's mapping controls to match an imported capability to its desk control. Lamp On/Off, Reset, and similar functions are actions rather than ordinary recordable values.

## Custom attributes

Choose **Attributes > New custom attribute** for a genuinely new capability and select its label and encoder group. Use **Imported attribute names** when a file merely spells an existing capability differently, to avoid duplicate controls. Those name mappings stay on the desk; existing fixture revisions keep their resolved mapping.

## Activation

An activation group makes linked supported attributes active together. Each attribute belongs to at most one group; missing members are skipped for each fixture/head.

Changing one member captures its partners from the current Normal, Blind, or Preload context. They then remain fixed in the Programmer. Editing group membership changes future activations, not recorded Cues.
