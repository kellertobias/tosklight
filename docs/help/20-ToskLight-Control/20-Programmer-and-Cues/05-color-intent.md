# Color Intent

Color Intent stores one color request that each fixture reproduces as closely as it can. [Show color model](32-color-model.md) covers choosing Direct or Color Intent and opening supported shows.

## Programming a colour

In a Color Intent show the Color special dialog programs one colour for the whole selection. Every
selected fixture receives the same colour, including fixtures that cannot show it; they stay
selected and the desk describes what they achieve (see below).

- **A first edit starts from the colour the fixture shows.** When the Programmer holds no colour
  for a fixture, the first Color encoder turn or picker touch edits the colour its output holds,
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

- **Easy**: Red, Green, Blue and White Blend on the first Color encoder page.
- **Easy with Amber and UV**: adds a second page with Amber and UV.
- **Advanced**: adds a second page with Temperature, Duv and the colour wheels.

Each Color encoder shows the selection's programmed value. A fixture the Programmer holds no colour
for shows the colour its output holds, such as the colour of a running Cue or Playback; only a
fixture whose output holds no colour shows open white, Red, Green and Blue at 100%, White Blend at
0%, 6500 K. Either way the encoder shows the colour the fixture's first edit starts from, and the
Color special dialog starts from the same values. Black is a colour: a Cue that holds black reads
0%. Selected fixtures with different values show **Mixed**, and a fixture that holds a Direct
colour shows **Direct** until an edit on page 1 or 2 turns it back into a Color Intent.

Pages 3 and 4 hold the reference head's own colour controls; see [Direct Color](07-direct-color.md).

### The Color special dialog

When the lower encoder area has room, at least 680 × 210 pixels of the area itself, whatever the
screen size, **Special Dialog** opens the compact dialog in place of the encoders:

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
  scrolls when it is longer than the window. Tapping a Fixture Sheet colour information icon or triangle opens the window
  straight on this tab, at that fixture.

To spread a range, hold **Shift** (the desk key, the on-screen **SHIFT** or the attached hardware) and
touch the first value, then the last. The first touch only marks endpoint 1; the range is written
when the last endpoint is touched, in the order touched, so a range may run downwards. The first
selected fixture receives the first value and the last receives the last. Hue takes the shorter way
round the colour wheel, so 350° to 10° passes through red; a range of exactly half the wheel turns
clockwise, towards increasing hue. Touching a control again without Shift returns only that control
to one value; every other range stays.

Each spread can also be entered as one Shift-drag between endpoints. Markers **1** and **2** identify them. Each touch or drag forms one Undo step. Blind, Preview, Preload, Record, and Update follow the ordinary Programmer workflow. With nothing selected, the dialog changes nothing and shows no message.

### Media layers

A selection of Media Server layers uses the same dialog titled **Media color**. The picker sets the
layer's tint and **White Blend** turns the picture towards greyscale under that tint; the second
page shows a preview instead of Temperature and Duv, and the full window's second tab is **Preview**
instead of **Details**. Layer and Master Intensity stay on the
Intensity controls. A selection that mixes lamps and Media layers uses the lamp dialog.

## Color results and presets

The [Color results](30-color-results-and-profiles.md) explain Approximate, Out of gamut, Wheel-limited, Uncalibrated, and Unsupported readouts. The programmed request stays stored even when a fixture cannot reproduce it.

## Universal Color presets

In a Color Intent show, a Color preset recorded from fixtures that all hold the same colour is stored
once, as that colour, rather than once per fixture. Such a preset is **universal**: recalling it
gives that colour to every selected fixture, including fixtures that were never part of it. Its
pool tile shows the number of fixtures currently showing it.

A Color preset recorded from fixtures holding different colours keeps each fixture's own colour and
only ever applies to those fixtures; it is never stretched over the rest of the selection.

Recalling a universal preset with nothing selected quietly leaves the desk unchanged. Updating a universal preset with its own colour keeps it universal; merging a
different colour for a few fixtures keeps the universal colour for everyone else.
