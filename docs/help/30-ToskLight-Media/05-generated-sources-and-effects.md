# Generated Sources and Effects

The Media Server can render more than uploaded video and pictures. Addressed text, generated visualizers, masks, and typed effects remain normal library choices that a Desk can store in Cues.

## Text

Text content occupies folders `200`–`249`. Configure the text source and its presentation in the Media administration interface, then recall its numeric folder/file address through the matching Media layer. Keep fonts and other local dependencies available on the production server.

A clock, and any text derived from the clock, follows the server's own UTC offset. Set it once in **Settings > Libraries > Server time** as minutes east of UTC with the number field, its step buttons, or its touch keypad; the value saves automatically, the section heading says whether it was saved, and a value outside ±840 minutes is shown as an error and not saved. The offset is stored configuration and applies to the next drawn frame, so a clock on an output follows an accepted change without a restart. It is deliberately independent of the timezone the host operating system is set to, because a show machine is not always configured for the venue it is standing in. A single clock that has to show another city can carry **Own UTC offset**; every other clock keeps following the server.

A countdown comes in three kinds. **Countdown of a length** starts when the layer showing it becomes visible. **Countdown to a moment** counts to one date and time. **Countdown to a time every day** counts to a local time, such as 21:00, with no date, so the same source works before every show. It counts from midnight to that day's time. Once the time has passed, it does what **After zero** says: hold at zero, go on with a minus sign, or count up. At the next midnight it starts counting to the next day's time. "Local" and "midnight" follow the server's UTC offset, like a clock. That offset is fixed, so a daylight-saving change moves the target only when the offset in **Settings > Libraries > Server time** is changed. Countdowns saved before this kind existed keep counting as they did.

## Generated visualizers

Generated visualizers occupy folders `250`–`255`. Each visualizer has a stable kind and only exposes parameters that affect that kind. Examples include spectra, waveforms, geometric motion, particles, rays, glitch treatments, digital rain, tunnels, landscapes, and nets.

Audio-reactive visualizers follow the beat and three instruments the Media Server hears: the kick, the snare or clap, and the hi-hat. Beat Explosions and Pulsing Circles jump on each kick, Lightning Tendrils cracks with the snare, and Starfield sparkles with the hi-hats. The **Audio** page shows the same instruments as lamps over their levels, so an operator can confirm what the visualizers will react to before the show.

Audio-reactive visualizers depend on the Media Server's configured audio input and analysis path. A visualizer can render correctly in the browser while reacting incorrectly to silence, the wrong device, or an unsuitable signal level; verify the live input on the production machine.

Triangular Net is carried by the beat rather than by a clock: a kick heaves its big swells and raises them into mountains, a snare shoves the mid-scale chop, and a hi-hat ripples the fine detail, each easing out at the rate **Decay** sets. Between songs it holds its shape and stands still, so a still net on a playing input means the instruments are not being heard — check the lamps on the **Audio** page.

Flying Props flies music props through the picture: headphones, cassettes, records, microphones, and beamed notes, drawn in **Colour** with their labels and cushions in **Second colour**. The props ship with the visualizer; operator-supplied models are not supported. Its four Visualizer Parameter channels are **Flight pattern**, **Speed**, **Density**, and **Size**:

| Flight pattern | Motion |
| --- | --- |
| 0 — Fly-through | Props come out of the distance toward the camera and fade before they fill the picture |
| 1 — Drift | Props cross the picture sideways at different depths |
| 2 — Orbit | Props circle the middle of the picture on a tilted carousel |
| 3 — Rise | Props float up from below the picture and out of the top |

Values past 3 repeat the list. **Density** sets how many props fly, from 1 to 48, and **Size** how big each one is. The props keep flying in silence. A held kick swells them and pushes their flight forward, and a snare or the beat flash brightens their accents; **Reactivity** scales both, and a loud input cannot enlarge them further than a full kick does.

A new Media Server configuration assigns Flying Props to `250/025`. A configuration saved before Flying Props shipped keeps its own visualizer assignments; add Flying Props to an empty slot on the **Visualizers** page.

## Effects and masks

Masks are selected independently from content and can be combined with the layer's shaper and
transform. Each layer has exactly two ordered effect banks. **Bank 1** is applied before **Bank 2**;
each bank stores an **Effect Select** value, an **Effect Strength**, and four **Parameters**. Select
**0** is Off and **1–255** resolves the matching preset in the Library's Effects tab. A missing or
unsupported preset is reported and bypassed instead of silently substituting another effect.

The bank's four parameters follow the order the Effects tab lists the selected effect's parameters
in; an effect with more than four keeps the rest at the preset's values. A parameter at **0** plays the value the preset stores; **1–255** sweeps it from its
minimum to its maximum. Choices and counts, such as the Rasterize mode or Kaleidoscope repetitions,
step through their whole values across that range. A parameter the selected effect does not have is
ignored, so a bank can change presets without the leftover bytes doing anything.

The two banks are the only way a layer plays effects, from the desk and from the Media Server's own
layer controls alike. **Blur** is not a channel of its own: select a Blur preset in a bank, and use
the bank's parameters for its amount and type.

## Blend mode and strobe

**Blend mode** decides how a layer combines with the layers below it. Values **0–127** choose
**Normal**, **Add**, **Screen**, **Multiply**, **Overlay**, **Difference**, **Lighten**, and
**Darken** in bands of sixteen. **128–249** strobe the layer from 1 to 25 flashes per second while it
blends Normally; each flash is lit for half its period. **250–255** are Normal without strobe, so a
fader pushed to full never leaves a layer flashing. The layer's dimmer and mask still weight the
blended result, and the isolated layer preview shows the strobe.

## Playback range

**In point** and **Out point** are 16-bit frame counts. The In point counts frames from the clip's
start and the Out point counts frames back from its end, so **0** on both plays the whole clip. Every
play mode stays inside the range: a loop wraps from the Out point back to the In point, a bounce turns
at both, and a single pass ends on the Out point. An In point beyond the clip holds its last frame,
and an Out point that would end the range before the In point plays through to the clip's end. Changing the range while a clip plays
keeps the playhead where it is when it is still inside the range. Stills, text, and visualizers
ignore the range.

The points count frames at the server-wide **Frame rate** in **Settings > Libraries > In and Out
points**, 25 fps by default and 1–120 fps. The rate turns a count into a time in the clip: at 25 fps
an In point of 250 starts 10 seconds in, whatever the clip's own frame rate, and a clip recorded at
that rate starts exactly on frame 250. The setting saves automatically and applies immediately,
also to layers that are playing; a stored configuration from before the setting reads 25 fps. Every
output reports the rate over the Media API, so the desk Media pane and this server's own layer page
both show and take the points as `mm:ss.ff`: the In point from the clip's start, the Out point before
its end, with **End of clip** for an Out point of zero. Touch the value to type a time; there is no
0–65535 fader.

## Visualizer parameters

Every layer carries four dedicated **Visualizer Parameter** channels (slots 52–55 of each 59-slot
layer block), separate from the two effect banks. **FX1** and **FX2** therefore stay ordinary effect
slots whether the layer shows a visualizer or ordinary media.

The visualizer the layer shows defines what each channel means: its name, its range, and its
default. The channels take the first four parameters in the order the Visualizers editor lists
them, so Equalizer Bars reads **Count**, **Size**, **Colour**, and **Second colour**, and
Waveform Oscilloscope reads **Size** (0.005–0.1 of the picture), **Thickness**, **Amount**, and
**Colour**. **0** keeps the default, which is the layer's own tuning when it has one and otherwise the
configured visualizer's value; **1–255** sweeps the parameter across that visualizer's range.
Colours sweep the hue wheel, switches turn on at 128, and variants count up from the first. A
visualizer with fewer than four parameters leaves the remaining channels inert, and a layer showing
library media, text, or nothing ignores all four.

The Media pane's **Effects › Visualizer** tab, ToskLight Control's Media pane, and DMX input all
address these same four bytes, and each names a channel as the shown visualizer does. The
Visualizer tab's settings above the four channels tune the whole visualizer for this layer only;
that tuning belongs to the layer, applies only while the layer shows that visualizer, and
**Reset parameters** returns the layer to the configured values.

Pixel configuration files from before this change are brought forward on load: an effect preset
no longer carries visualizer settings, and the preset keeps its slot, name, and effect. The channel
layout and footprint (158 and 512 slots) are unchanged, so existing patches keep working.

## Output mirroring

The Master has no Flip/mirror channel. **Master scale X** and **Master scale Y** run from −4× to
+4× with **1×** at raw value 40960; a negative axis mirrors the finished output along that axis.

## Effect catalogue

The initial catalogue includes TV/CRT/VHS Simulation, Digital Video/ Glitch Simulation, Blur
(Gaussian, Shape, Radial, Linear, and Axial), Feedback, Beat Move, Beat Scan, Beat Scale & Turn,
Beat form Flash, Kaleidoscope, B/W Rasterize, CMYK Rasterize, Drawn Image Style, and Outline (slot
13). Kaleidoscope
repetitions run from Off through 12. Feedback supports Shake and Tunnel motion and is tuned for a
longer, smoother trail by default.

Every slot in the Library's **Effects** tab shows a thumbnail of its effect type, and the slot
editor's thumbnail follows the **Effect type** you choose before you save. The thumbnails are
pictures rendered ahead of time by the Media Server's own compositor, so browsing the library never
renders anything on the output or interrupts playback. They show each shipped preset on one
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
**Line thickness** is the line width in source pixels (1–8). **Line hue** and **Line saturation**
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

An installation that already had an effect library receives the Outline preset in slot 13 when that
slot is free and the library holds no Outline yet; a slot you use for something else keeps its
preset, and a library you emptied stays empty.

The Master layer has a separate fixed **Effects** section. **Layer Opacity Cycle** advances through
all currently loaded layers whose dimmer is above zero. Its Multiplier / Divider can be Off or beat
divisions/multiples; cycling changes only effective output opacity and never rewrites stored layer
dimmers.

Use the isolated layer preview to identify a bank or mask problem before diagnosing the output
composite. The layer preview and final output use the same bank order and preset settings.

Generated configuration is stored by addressed identity so Desk programming remains stable. Moving or replacing an addressed generated item changes what later recalls produce just as moving uploaded media would.
