# Color Intent

## Purpose

Prove that a show can program one device-independent colour instead of fixture-native colour
channels. Every fixture then reproduces that colour as closely as its own colour engine allows,
with Intensity alone setting the level. Prove the operator is told whenever a fixture shows the
colour only approximately or not at all. Prove that a Color preset holding one shared colour
applies to every selected fixture, while a preset of different colours never spreads. Prove that
existing shows are left exactly as they were.

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

- The Color dialog has no **Brightness** control and no **Tint** control. Picking pure red stores
  one whole colour on each of the five fixtures and no Red, Green, Blue, White or CMY value.
- DMX: RGB 1 and RGB 2 output red 255, green 0, blue 0. CMY 4 outputs cyan 0, magenta 255,
  yellow 255. Dimmer 5 keeps its colour channels untouched.
- Intensity 50% halves the output level — the dimmer channel, or on a fixture without one all of
  its colour channels together — and keeps the colour's proportions. Picking a darker shade of the
  same red changes nothing.
- The encoders, Fixture Sheet columns and channel faders show no Red, Green, Blue, White, Amber,
  CMY, colour-wheel, Hue, Saturation or Tint control. Setting a fixture-native colour channel
  through a value action is refused with a message naming Color Intent.
- A media-server fixture with a Grayscale level still shows **Grayscale** in the Color dialog.

## COLORINTENT-003 — Visible resolution results

With the red above still programmed:

- The Color dialog lists RGB 1, RGB 2, RGBW 3 and CMY 4 as **Uncalibrated** and Dimmer 5 as
  **Unsupported**. No fixture is listed as exact.
- On a fixture whose profile authors a measured RGB colour system, pure red is not listed and the
  dialog reads **Every selected fixture shows this colour exactly** when it is the only selection.
- A saturated spectral cyan outside that fixture's gamut lists it as **Out of gamut** with its
  Δu′v′.
- On a wheel-only fixture with measured slots, an orange between two slots lists it as
  **Wheel-limited**; the wheel never parks on a Rainbow, Scroll or split position to make it.

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
- Recalling Color 1 with nothing selected selects nothing and says a universal Color preset
  applies to the selection.
- Color 1 recalled through the Preset pool, the command line `AT COLOR PRESET 1`, and
  `FixAT COLOR PRESET 1` produces the same DMX on every selected fixture.

## COLORINTENT-005 — Switching a programmed show

- In a Direct show whose Color preset stores a half-level red and a White channel value,
  **Setup → Attributes & encoders → Color model → Color Intent** first lists both: the White value
  is kept and still plays, and the half-level red will play at full brightness. **Keep Direct**
  leaves the show Direct.
- **Switch to Color Intent** switches the show without rewriting the stored preset.
- Switching an Intent show whose colours sit on uncalibrated fixtures back to Direct warns that
  those fixtures lose that colour before it switches.
