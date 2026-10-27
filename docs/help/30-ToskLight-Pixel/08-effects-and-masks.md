# Apply effects and masks

Configure numbered presets on Pixel's **Effects** page, then select them in a layer's **Bank 1** or **Bank 2**. Slot **0** is **Off**; slots **1-255** hold presets. Editing a preset changes every bank that selects its number, including later Cue recalls.

## Select a preset in a bank

1. Select the layer in **Media** and enable **Take over playback** when editing in Pixel.
2. Open **Effects** and choose **Bank 1** or **Bank 2**.
3. Set **Effect Select** to the preset number.
4. Raise **Effect Strength** and adjust its four parameter controls while watching the layer preview.
5. Release takeover before testing playback from Control.

Masks are selected independently from content and can be combined with the layer's shaper and
transform. Each layer has exactly two ordered effect banks. **Bank 1** is applied before **Bank 2**;
each bank stores an **Effect Select** value, an **Effect Strength**, and four **Parameters**. Select
**0** is Off and **1-255** resolves the matching preset in the Library's Effects tab. A missing or
unsupported preset is reported and bypassed instead of silently substituting another effect.

The bank's four parameters follow the order the Effects tab lists the selected effect's parameters
in; an effect with more than four keeps the rest at the preset's values. A parameter at **0** plays the value the preset stores; **1-255** sweeps it from its
minimum to its maximum. Choices and counts, such as the Rasterize mode or Kaleidoscope repetitions,
step through their whole values across that range. A parameter the selected effect does not have is
ignored, so a bank can change presets without the leftover bytes doing anything.

To play **Blur**, select a Blur preset in a bank and use its parameters for amount and type. Blur has no dedicated DMX channel.

## Blend mode and strobe

**Blend mode** decides how a layer combines with the layers below it. Values **0-127** choose
**Normal**, **Add**, **Screen**, **Multiply**, **Overlay**, **Difference**, **Lighten**, and
**Darken** in bands of sixteen. **128-249** strobe the layer from 1 to 25 flashes per second while it
blends Normally; each flash is lit for half its period. **250-255** are Normal without strobe, so a
fader pushed to full never leaves a layer flashing. The layer's dimmer and mask still weight the
blended result, and the isolated layer preview shows the strobe.

## Choose and tune an effect

The effect catalogue includes TV/CRT/VHS Simulation, Digital Video/ Glitch Simulation, Blur
(Gaussian, Shape, Radial, Linear, and Axial), Feedback, Beat Move, Beat Scan, Beat Scale & Turn,
Beat form Flash, Kaleidoscope, B/W Rasterize, CMYK Rasterize, Drawn Image Style, Outline (slot
13), and Beat Slice Grid (slot 14). Kaleidoscope
repetitions run from Off through 12. Feedback supports Shake and Tunnel motion and is tuned for a
longer, smoother trail by default.

Beat Move, Beat Scan, Beat Grid Wave, Beat Scale & Turn, Beat Form Flash, Beat Slice Grid, and Outline with Beat depth have a **React to** setting in their Library effect slot. **Detected beat** follows actual kick or snare hits heard in the audio, even when the song slows down. **Live beat (tempo pulse)** follows the current tempo-aligned pulse, including through gaps in the music. **Bass drum**, **Hi-hat**, and **Snare** follow only that instrument's detected hits. The default for existing and new beat effects is Detected beat. A missing input or a silent instrument produces no hits for that source.

Every slot in the Library's **Effects** tab shows a thumbnail of its effect type, and the slot
editor's thumbnail follows the **Effect type** you choose before you save. Browsing effect thumbnails leaves the output unchanged. They show each shipped preset on one
reference picture; your own parameter values are not reflected in them.

Blur **Amount** scales with the source's height, so a 4K clip and a 720p clip soften alike. At the
default amount a Gaussian blur smooths fine detail completely instead of leaving a ghost of it;
**Shape** gives an even, lens-like disc, **Radial** a zoom smear toward the centre, **Linear** a
horizontal motion smear, and **Axial** a rotational smear. An amount of **0** is an exact bypass.

### Outline

**Outline** finds the edges in the layer's picture and draws them as lines. **Intensity** carries the
whole look:

| Intensity | Result |
| --- | --- |
| 0 | The original picture, unchanged. |
| up to 0.5 | The lines fade in over the picture. |
| 0.5 | Full lines over the fully visible picture. |
| above 0.5 | The picture darkens behind the lines. |
| 1 | Only the lines remain, on black. |

The bank's **Effect Strength** scales Intensity, so a bank at half strength plays half of it.
**Line thickness** is the line width in source pixels (1-8). **Line hue** and **Line saturation**
colour the lines; saturation **0** draws white lines whatever the hue. **Edge sensitivity** decides
how soft a contrast still counts as an edge: raise it for low-contrast footage, lower it to keep only
strong shapes. The edge of a cut-out picture counts too, and its line stays visible over what is
beneath the layer.

Outline can follow the music. **Beat depth** above **0** makes every beat the Media Server detects
push Intensity toward **1** by that amount, and **Beat decay** is how many seconds it takes to fall
back to the stored Intensity. It uses the same audio input and beat detection as the Beat effects,
so it rests at the stored Intensity while no audio device is selected. Beat depth is **0** in the
shipped preset. A bank's four parameters are Intensity, Beat depth, Line thickness, and Line hue;
the other settings come from the preset.

Outline uses slot 13 in a new library. Pixel adds it to an existing library only when slot 13 is free and no Outline preset exists. An intentionally empty library stays empty.

### Beat Slice Grid

**Beat Slice Grid** divides the layer image into rectangular slices and mixes them with text
cells. Its cell boundaries change on the selected beat or instrument and settle between hits.
Set **Text colour**, **Background colour**, and **Font** in the preset. The font must be available
on the Pixel server; the editor reports the resolved font or fallback.

Add up to eight text entries, each with up to 512 characters and 16 lines. Line breaks are kept,
and text cells cycle through the entries across the grid. Each entry stays proportional and
scales to fit inside its cell with padding, including while the cell changes size on a beat.
Colours, font, and all entries belong to the preset, so both effect banks and later recalls use
the same presentation. Older presets keep their existing text as the first entry.

**Columns** and **Rows** set the grid. **Beat movement** controls how far boundaries move; **Text cells** controls the proportion of text panels. These are the bank's four parameters, in that order. **Effect Strength** blends the grid with the original layer.

A new library assigns Beat Slice Grid to slot 14. An existing library receives it only when that slot is free and no Beat Slice Grid preset exists. An intentionally empty library stays empty.

## Cycle loaded layers

The Master layer has a separate fixed **Effects** section. **Layer Opacity Cycle** advances through
all currently loaded layers whose dimmer is above zero. Its Multiplier / Divider can be Off or beat
divisions/multiples; cycling changes only effective output opacity and never rewrites stored layer
dimmers.

Use the isolated layer preview to identify a bank or mask problem before diagnosing the output
composite. The layer preview and final output use the same bank order and preset settings.
