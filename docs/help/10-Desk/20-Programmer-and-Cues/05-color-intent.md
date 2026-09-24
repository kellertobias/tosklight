# Color Intent

A show programs colour in one of two ways, chosen per show:

- **Direct** programs each fixture's own colour channels: Red, Green, Blue, White, Amber, the CMY
  flags, colour wheels, Hue, Saturation, Tint, and so on. Every show created before Color Intent
  existed is Direct and stays Direct.
- **Color Intent** programs one colour that does not belong to any fixture. The desk works out,
  fixture by fixture, how to show that colour as closely as each fixture can. An RGBW wash, a CMY
  spot and a wheel-only profile given the same colour each do their best with what they have.

## Choosing the model

**Setup → Attributes & encoders → Color model** shows and switches the model of the show that is
open. The model is stored in the show and travels with it.

Switching a show that already holds programming first lists what the switch changes about the
colour it stores:

- Fixture-native colour values recorded under Direct are kept and still play in Color Intent, but
  they can no longer be edited from the Color feature.
- A Direct colour that carries its own brightness — a half-level red, say — is shown at full
  brightness by Color Intent. Its level has to be set with Intensity instead, so it does not come
  back unchanged when the show is switched back.
- A Color Intent colour on a fixture whose profile has no authored colour system cannot be shown by
  Direct, so that fixture loses that colour.

A switch that would lose colour needs **Switch to …** to be pressed in that list. The switch itself
rewrites no stored value.

**Setup → Defaults → New shows** chooses the model copied into each show this desk creates. It is
a starting point only: changing it later never changes a show that already exists.

## Programming a colour

In a Color Intent show the Color special dialog programs one colour for the whole selection. Every
selected fixture receives the same colour, including fixtures that cannot show it; they stay
selected and say so (see below).

- **Intensity sets the level.** The colour is only the colour: the dialog has no Brightness control,
  and each fixture shows the colour as brightly as its colour engine allows before its dimmer.
- **Native colour channels are not controls.** Red, Green, Blue, White, Amber, CMY, colour wheels,
  Hue, Saturation, Colour Temperature and Tint leave the encoders, Fixture Sheet columns and channel
  faders. Setting one from the command line or another surface is refused with a message.
- **Media levels remain.** A media-server layer that has its own Grayscale level keeps that control;
  it is a picture setting, not a lamp colour.

## How a fixture shows the colour

For each fixture head the desk:

1. uses its continuous colour engine — the LED emitters, the CMY flags, or a Hue/Saturation engine —
   and makes the nearest colour it can, as bright as that engine goes;
2. uses a fixed colour-wheel slot only when the continuous engine cannot come close enough, or when
   the wheel is the fixture's only colour engine; and
3. never parks a wheel on a split colour, a scroll, a rotation or an effect, unless the fixture
   profile marks that position as usable for a steady colour.

The same colour on the same fixture always gives exactly the same DMX, whether it comes from the
Programmer, a Preset, a Cue, the command line, OSC or an attached control surface.

### What the dialog tells you

Below the picker, the Color dialog lists every selected fixture that does not show the colour
exactly:

| Label | Meaning |
|---|---|
| **Approximate** | Close to the colour; the fixture's colour data is typical rather than measured. |
| **Out of gamut** | The fixture cannot make this colour and shows the nearest one it can. |
| **Wheel-limited** | The fixture can only choose the nearest wheel slot. |
| **Uncalibrated** | The fixture profile has no colour calibration; the colour is a best guess from its channel names. |
| **Unsupported** | The fixture has no colour engine and keeps its colour as it is. |

When every selected fixture matches, the dialog says so. Each entry shows the remaining difference as
Δu′v′, the distance between the two colours on the CIE 1976 chromaticity diagram; about 0.004 and
below is not visible on stage.

## Universal Color presets

In a Color Intent show, a Color preset recorded from fixtures that all hold the same colour is stored
once, as that colour, rather than once per fixture. Such a preset is **universal**: recalling it
gives that colour to every selected fixture, including fixtures that were never part of it. Its
pool tile reads **Universal** followed by the number of fixtures currently showing it.

A Color preset recorded from fixtures holding different colours keeps each fixture's own colour and
only ever applies to those fixtures; it is never stretched over the rest of the selection.

Recalling a universal preset with nothing selected selects nothing and tells you to select
fixtures first. Updating a universal preset with its own colour keeps it universal; merging a
different colour for a few fixtures keeps the universal colour for everyone else.

## Fixture profiles

A profile's colour data decides how well a fixture follows the colour. Each colour system in a
profile can declare its calibration as **measured**, **nominal** or **uncalibrated**, with a revision
number that changes whenever the data changes. CMY engines can carry the measured colour of the open
beam and of each flag fully in, and every wheel slot can be marked steady or not. Profiles without
this data still work: they are treated as nominal, and fixtures whose profile authors no colour
system at all are resolved from their channel names and reported as uncalibrated.

A show keeps the profile revision each fixture was patched with, so updating a fixture profile
changes how that fixture shows the colour only when the operator updates the fixture; the colour
stored in the show never changes.

## The colour itself

For fixture developers and integrations: a Color Intent colour is a CIE 1931 2° XYZ value relative to
the D65 white point, with the white at a luminance *Y* of 1 and every component between 0 and 1.2.
It is stored and sent as `{ "x", "y", "z" }` under the `color` attribute. Only its chromaticity is
the colour; a colour of zero luminance is treated as white rather than as black.
