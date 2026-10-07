# Color Intent

## Purpose

Prove that a show can program one device-independent colour instead of fixture-native colour
channels. Every fixture then reproduces that colour as closely as its own colour engine allows,
with Intensity alone setting the level. Prove the operator is told whenever a fixture shows the
colour only approximately or not at all. Prove that a Color preset holding one shared colour
applies to every selected fixture, while a preset of different colours never spreads. Prove that
existing shows are left exactly as they were.

The Color dialog wording of COLORINTENT-002 and 003 describes the semantic Color dialog
([36-semantic-color-controls.md](36-semantic-color-controls.md)), which production uses since the
programming-contract cutover (TL-552). The executable cases in `tests/112-color-intent.spec.ts`
open it as the full modal and read its per-fixture approximation. Colour is programmed as one whole
colour on `color`; a single colour-channel percentage (Red, White, …) is legacy programming that
the desk refuses before storing it.

Profiles without an authored colour physical model (the Generic LED profiles of this rig) are
driven through a model derived from their colour channels when the show is compiled: sRGB
primaries and a D65 white, CMY flags as ideal sRGB complements, UV as an emitter of unknown visible
output. With no authored colour system that model is uncalibrated, so these fixtures output the
colour and the report reads **Uncalibrated**, never **Exact**. Colour controls outside the chosen
engine (colour temperature, tint, a colour wheel or macro in front of emitters, a second wheel) are
parked at their neutral position and named in the report; a mode whose colour cannot be described
reports **Unsupported** with the reason instead of being left out. A `FixAT` of a whole colour
renders the same way whether or not a static colour sits underneath it.

Fixtures used throughout:

- **RGB 1** and **RGB 2**: `Generic › RGB LED`, uncalibrated (no authored colour system)
- **RGBW 3**: `Generic › RGBW LED`
- **CMY 4**: `Generic › CMY LED`
- **Dimmer 5**: `Generic › Dimmer`

## COLORINTENT-001 — Existing and new shows, and the desk default

- A show created before this feature, or any show created while **Setup → Defaults → New shows →
  Color programming model** reads **Direct**, opens as **Direct**. Its stored attribute
  configuration has no `color_model`, and its Programmer, Preset, Cue and DMX behaviour is
  unchanged.
- With the desk default set to **Color Intent**, a new blank show opens in Color Intent:
  **Setup → Attributes & encoders → Color model** reads **Color Intent**. A show that already
  existed still reads **Direct**.
- Setting the desk default back to **Direct** leaves the Color Intent show in Color Intent.

## COLORINTENT-002 — One colour on every fixture, level from Intensity

In a Color Intent show with all five fixtures patched and selected at Intensity 100%:

- The Color dialog has no level control: its picker, **White Blend**, **Temperature** and **Duv**
  author the colour only. Picking pure red stores one whole colour on each fixture that has colour
  (RGB 1, RGB 2, RGBW 3, CMY 4) and no Red, Green, Blue, White or CMY value. A first pick on a
  fixture that holds no colour yet starts from open white. Dimmer 5 has no colour and takes none.
- DMX: RGB 1 and RGB 2 output red 255, green 0, blue 0. CMY 4 outputs cyan 0, magenta 255,
  yellow 255. Dimmer 5 keeps its colour channels untouched.
- Intensity 50% halves the output level — the dimmer channel, or on a fixture without one all of
  its colour channels together — and keeps the colour's proportions. Picking a darker shade of the
  same red changes nothing.
- The encoders, Fixture Sheet columns and channel faders show no Red, Green, Blue, White, Amber,
  CMY, colour-wheel, Hue, Saturation or Tint control. Setting a fixture-native colour channel
  through a value action is refused with a message naming Color Intent.
- A selection of media-server layers opens **Media color**: the picker sets the tint and **White
  Blend** is the greyscale. A selection mixing lamps and layers opens the lamp dialog.

## COLORINTENT-003 — Visible resolution results

With the red above still programmed and output at least once:

- The full Color window lists RGB 1, RGB 2, RGBW 3 and CMY 4 from the output frame that was sent:
  the uncalibrated profiles read **Uncalibrated**, never **Exact**. Dimmer 5 has no colour engine
  and nothing to report, so it is not listed; neither is any fixture nothing programs a colour for.
- On a fixture whose profile authors a measured RGB colour system, pure red reads **Exact** and the
  summary reads **Every selected fixture shows this colour exactly** when it is the only selection.
- A saturated spectral cyan outside that fixture's gamut lists it as **Out of gamut** with its
  Δu′v′.
- On a wheel-only fixture with measured slots, an orange between two slots lists it as
  **Wheel-limited**; the wheel never parks on a Rainbow, Scroll or split position to make it.
- UV is a separate column: a UV request on RGB 1 reads **UV unavailable on this fixture** while its
  visible result is unchanged.

## COLORINTENT-004 — Universal and fixture-specific Color presets

In the Color Intent show:

- RGB 1 and RGB 2 set to the same blue, recorded as **Color 1**, store one universal colour: the
  tile reads **Universal · 2** and the stored preset has `universal_values` and no per-fixture
  values.
- Selecting RGBW 3 and CMY 4 only, which were never part of Color 1, and recalling Color 1 gives
  both of them that blue.
- RGB 1 red and RGB 2 green recorded as **Color 2** keep each fixture's own colour. Recalling
  Color 2 with RGB 1, RGB 2 and RGBW 3 selected gives RGB 1 red and RGB 2 green and leaves RGBW 3
  unchanged.
- Recalling Color 1 with nothing selected quietly changes nothing, including context and Undo.
- Color 1 recalled through the Preset pool, the command line `AT COLOR PRESET 1`, and
  `FixAT COLOR PRESET 1` produces the same DMX on every selected fixture.

## COLORINTENT-005 — Switching a programmed show

- In a Direct show whose Color preset stores a half-level red and a fixture-native colour-wheel
  value (on a wheel fixture; a White channel percentage can no longer be stored since programming
  contract 1), **Setup → Attributes & encoders → Color model → Color Intent** first lists both:
  the wheel value is kept and still plays, and the half-level red will play at full brightness.
  **Keep Direct** leaves the show Direct.
- **Switch to Color Intent** switches the show without rewriting the stored preset.
- Switching an Intent show whose colours sit on uncalibrated fixtures back to Direct warns that
  those fixtures lose that colour before it switches.
