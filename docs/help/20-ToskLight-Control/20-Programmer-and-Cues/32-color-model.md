# Show color model

A show programs colour in one of two ways, chosen per show:

- **Direct** programs each fixture's own colour channels: Red, Green, Blue, White, Amber, the CMY
  flags, colour wheels, Hue, Saturation, Tint, and so on. The show keeps its selected color model when reopened.
- **Color Intent** programs one colour that does not belong to any fixture. The desk works out,
  fixture by fixture, how to show that colour as closely as each fixture can. An RGBW wash, a CMY
  spot and a wheel-only profile given the same colour each do their best with what they have.

A supported Direct recipe stores fixture-native controls together as one colour. Shows that still hold separate legacy Red, Green, Blue, or other colour-channel percentages cannot be opened. See
[Shows programmed before fixture-independent programming](../10-Show-Setup/05-users-sessions-and-recovery.md#shows-programmed-before-fixture-independent-programming).

## Choosing the model

**Setup → Attributes & encoders → Color model** shows and switches the model of the show that is
open. The model is stored in the show and travels with it.

Switching a show that already holds programming first lists what the switch changes about the
colour it stores:

- Fixture-native colour values recorded under Direct are kept and still play in Color Intent, but
  they can no longer be edited from the Color feature.
- A Direct colour that carries its own brightness (a half-level red, say) is shown at full
  brightness by Color Intent. Its level has to be set with Intensity instead, so it does not come
  back unchanged when the show is switched back.
- A Color Intent colour on a fixture whose profile has no authored colour system cannot be shown by
  Direct, so that fixture loses that colour.

A switch that would lose colour needs **Switch to …** to be pressed in that list. The switch itself
rewrites no stored value.

**Setup → Defaults → New shows** chooses the model copied into each show this desk creates. It is
a starting point only: changing it later never changes a show that already exists.

For controls, see [Color Intent](05-color-intent.md) and [Direct Color](07-direct-color.md).
