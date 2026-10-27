# Color results and fixture profiles

## How a fixture shows the colour

For each fixture head the desk:

1. uses its continuous colour engine (LED emitters, CMY flags, or Hue/Saturation) to make the nearest
   colour it can, as bright as that engine goes;
2. uses a fixed colour-wheel slot only when the continuous engine cannot come close enough, or when
   the wheel is the fixture's only colour engine; and
3. never parks a wheel on a split colour, a scroll, a rotation or an effect, unless the fixture
   profile marks that position as usable for a steady colour.

Colour controls that are not part of the chosen engine are held still so they cannot tint it:
colour temperature (CTC), tint, colour point, a colour wheel or colour macro in front of LED
emitters or CMY flags, and a second colour wheel. Every time the colour is output, each control is set to its neutral position.
The desk uses the range the profile names *Open*, *No function* or *Off*, or its default value when
no such range exists. A fixture that has only such controls, such as a tunable-white fixture, shows
white. On a fixture with several cells under one main head, colour controls of the main head stay at
their defaults while each cell shows the colour.

A Hue/Saturation engine is driven through a nominal hue and saturation table, so its colour is
typical rather than measured.

The same colour on the same fixture always gives exactly the same DMX, whether it comes from the
Programmer, a Preset, a Cue, the command line, OSC or an attached control surface.

### What the desk tells you

The full Color window lists each selected fixture head with the colour it shows in the output that
was actually sent, never a separate estimate. The visible colour and UV are reported separately:

| Label | Meaning |
|---|---|
| **Approximate** | Close to the colour; the fixture's colour data is typical rather than measured. |
| **Out of gamut** | The fixture cannot make this colour and shows the nearest one it can. |
| **Wheel-limited** | The fixture can only choose the nearest wheel slot. |
| **Uncalibrated** | The fixture profile has no colour calibration; the colour is a best guess from its channel names. |
| **Unsupported** | The fixture's colour controls cannot be described, so it keeps its colour as it is. The entry names the reason, for example LED emitters layered over a Hue/Saturation engine. |

An entry also names the colour controls that are held at their neutral position, and says when a
fixture can show only white.

The UV column reads **UV applied**, **UV limited by the emitter** or **UV unavailable on this
fixture**. Each visible entry shows the remaining difference as Δu′v′, the distance between the two
colours on the CIE 1976 chromaticity diagram; about 0.004 and below is not visible on stage. A
fixture that nothing programs a colour for is not listed.

These are normal capability results, not errors. Nothing interrupts programming: no message, sound
or window opens. The Fixture Sheet shows a steady information icon for an uncalibrated estimate. A request the fixture cannot provide, such as UV without a UV emitter, keeps its quiet triangle. Tap either mark beside Color to open that fixture's Details in the full Color window.

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

## Color names and Preset references

The Fixture Sheet names a recalled Color Preset only while the value retains its explicit Preset reference. Otherwise it shows the nearest known color name in angle brackets, such as `<Red>`. A matching numerical color does not create a Preset reference.
