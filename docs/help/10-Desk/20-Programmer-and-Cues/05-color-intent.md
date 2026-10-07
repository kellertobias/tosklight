# Color Intent

A show programs colour in one of two ways, chosen per show:

- **Direct** programs each fixture's own colour channels: Red, Green, Blue, White, Amber, the CMY
  flags, colour wheels, Hue, Saturation, Tint, and so on. Every show created before Color Intent
  existed is Direct and stays Direct.
- **Color Intent** programs one colour that does not belong to any fixture. The desk works out,
  fixture by fixture, how to show that colour as closely as each fixture can. An RGBW wash, a CMY
  spot and a wheel-only profile given the same colour each do their best with what they have.

Earlier builds could store Red, Green, Blue and the other single colour channels as separate
percentages. The desk does not open a show that still holds them. See
[Shows programmed before fixture-independent programming](../10-Show-Setup/05-users-sessions-and-recovery.md#shows-programmed-before-fixture-independent-programming).

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
selected and the desk describes what they achieve (see below).

- **A first edit starts from the colour the fixture shows.** When the Programmer holds no colour
  for a fixture, the first Color encoder turn or picker touch edits the colour its output holds —
  for example the colour of a running Cue or Playback. Only a fixture whose output holds no colour
  starts from open white, the colour it shows at rest.
- **Intensity sets the level.** The colour is only the colour: it has no level of its own, and each
  fixture shows it as brightly as its colour engine allows before its dimmer.
- **White Blend mixes towards white.** At 0% the colour is shown as picked, at 50% it is half way to
  the chosen white, and at 100% the fixture shows only that white. White Blend never changes the
  level.
- **Temperature and Duv choose the white.** Temperature sets the white in kelvin; Duv moves it off
  the black-body line, towards magenta below zero and towards green above it. Both faders are white
  at their centre.
- **UV is separate.** A UV request never changes the visible colour, and White Blend never changes
  UV. A fixture without UV keeps the request stored and simply shows no UV.
- **Native colour channels are not controls.** Red, Green, Blue, White, Amber, CMY, colour wheels,
  Hue, Saturation, Colour Temperature and Tint channels leave the encoders, Fixture Sheet columns and
  channel faders. The Color encoders and the special dialog author the colour instead.

### Easy and Advanced

**Setup → Attributes & encoders → Color model → Color controls on this desk** chooses which Color
controls this desk's encoders offer. It belongs to the desk, not to the show, and switching it never
changes programmed colour:

- **Easy** — Red, Green, Blue and White Blend on the first Color encoder page.
- **Easy with Amber and UV** — adds a second page with Amber and UV.
- **Advanced** — adds a second page with Temperature, Duv and the colour wheels.

Each Color encoder shows the selection's programmed value. A fixture the Programmer holds no colour
for shows the colour its output holds, such as the colour of a running Cue or Playback; only a
fixture whose output holds no colour shows open white — Red, Green and Blue at 100%, White Blend at
0%, 6500 K. Either way the encoder shows the colour the fixture's first edit starts from, and the
Color special dialog starts from the same values. Black is a colour: a Cue that holds black reads
0%. Selected fixtures with different values show **Mixed**, and a fixture that holds a Direct
colour shows **Direct** until an edit on page 1 or 2 turns it back into a Color Intent.

Pages 3 and 4 hold the reference head's own colour controls; see [Direct Color](07-direct-color.md).

### The Color special dialog

When the lower encoder area has room — at least 680 × 210 pixels of the area itself, whatever the
screen size — **Special Dialog** opens the compact dialog in place of the encoders:

- The first page holds the colour picker (hue left to right, saturation bottom to top) and the
  coloured **White Blend** fader, with **White balance** and **Expand** below the fader.
- **White balance** shows the second page, **Temperature** and **Duv**; **Color** returns to the
  first. Pressing **Special Dialog** again also switches between the two pages.
- Tapping the active **Color** tab returns to the encoders on the page they were on.

Where the area is smaller, and after **Expand**, the dialog opens as the full **Color** window with two
tabs in its title bar:

- **Color** is the colour selection: a large hue ring with **Saturation**, **White Blend**,
  **Temperature** and **Duv** beside it. The window always opens on this tab.
- **Details** shows how the rig shows that colour: the per-fixture results and **Direct color**. It
  scrolls when it is longer than the window. Tapping a Fixture Sheet colour triangle opens the window
  straight on this tab, at that fixture.

To spread a range, hold **Shift** (the desk key, the on-screen **SHIFT** or the attached hardware) and
touch the first value, then the last. The first touch only marks endpoint 1; the range is written
when the last endpoint is touched, in the order touched, so a range may run downwards. The first
selected fixture receives the first value and the last receives the last. Hue takes the shorter way
round the colour wheel, so 350° to 10° passes through red; a range of exactly half the wheel turns
clockwise, towards increasing hue. Touching a control again without Shift returns only that control
to one value; every other range stays.

### Media layers

A selection of Media Server layers uses the same dialog titled **Media color**. The picker sets the
layer's tint and **White Blend** turns the picture towards greyscale under that tint; the second
page shows a preview instead of Temperature and Duv, and the full window's second tab is **Preview**
instead of **Details**. Layer and Master Intensity stay on the
Intensity controls. A selection that mixes lamps and Media layers uses the lamp dialog.

## How a fixture shows the colour

For each fixture head the desk:

1. uses its continuous colour engine — the LED emitters, the CMY flags, or a Hue/Saturation engine —
   and makes the nearest colour it can, as bright as that engine goes;
2. uses a fixed colour-wheel slot only when the continuous engine cannot come close enough, or when
   the wheel is the fixture's only colour engine; and
3. never parks a wheel on a split colour, a scroll, a rotation or an effect, unless the fixture
   profile marks that position as usable for a steady colour.

Colour controls that are not part of the chosen engine are held still so they cannot tint it:
colour temperature (CTC), tint, colour point, a colour wheel or colour macro in front of LED
emitters or CMY flags, and a second colour wheel. Each one is set to its neutral position — the
range the profile names *Open*, *No function* or *Off*, otherwise its default value — every time
the colour is output. A fixture that has only such controls, such as a tunable-white fixture, shows
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
or window opens. The Fixture Sheet shows at most one small, steady triangle beside a fixture's
Color value when it cannot show the request as asked; tap or select it to open the full Color window
on that fixture's details.

## Universal Color presets

In a Color Intent show, a Color preset recorded from fixtures that all hold the same colour is stored
once, as that colour, rather than once per fixture. Such a preset is **universal**: recalling it
gives that colour to every selected fixture, including fixtures that were never part of it. Its
pool tile reads **Universal** followed by the number of fixtures currently showing it.

A Color preset recorded from fixtures holding different colours keeps each fixture's own colour and
only ever applies to those fixtures; it is never stretched over the rest of the selection.

Recalling a universal preset with nothing selected quietly leaves the desk unchanged. Updating a universal preset with its own colour keeps it universal; merging a
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
