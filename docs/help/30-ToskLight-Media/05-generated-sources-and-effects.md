# Generated Sources and Effects

The Media Server can render more than uploaded video and pictures. Addressed text, generated visualizers, masks, and typed effects remain normal library choices that a Desk can store in Cues.

## Text

Text content occupies folders `200`–`249`. Configure the text source and its presentation in the Media administration interface, then recall its numeric folder/file address through the matching Media layer. Keep fonts and other local dependencies available on the production server.

A clock, and any text derived from the clock, follows the server's own UTC offset. Set it once in **Settings > Libraries > Server time** as minutes east of UTC; the offset is stored configuration and applies to the next drawn frame, so a clock on an output follows an accepted change without a restart. It is deliberately independent of the timezone the host operating system is set to, because a show machine is not always configured for the venue it is standing in. A single clock that has to show another city can carry **Own UTC offset**; every other clock keeps following the server.

## Generated visualizers

Generated visualizers occupy folders `250`–`255`. Each visualizer has a stable kind and only exposes parameters that affect that kind. Examples include spectra, waveforms, geometric motion, particles, rays, glitch treatments, digital rain, tunnels, landscapes, and nets.

Audio-reactive visualizers follow the beat and three instruments the Media Server hears: the kick, the snare or clap, and the hi-hat. Beat Explosions and Pulsing Circles jump on each kick, Lightning Tendrils cracks with the snare, and Starfield sparkles with the hi-hats. The **Audio** page shows the same instruments as lamps over their levels, so an operator can confirm what the visualizers will react to before the show.

Audio-reactive visualizers depend on the Media Server's configured audio input and analysis path. A visualizer can render correctly in the browser while reacting incorrectly to silence, the wrong device, or an unsuitable signal level; verify the live input on the production machine.

Triangular Net is carried by the beat rather than by a clock: a kick heaves its big swells and raises them into mountains, a snare shoves the mid-scale chop, and a hi-hat ripples the fine detail, each easing out at the rate **Decay** sets. Between songs it holds its shape and stands still, so a still net on a playing input means the instruments are not being heard — check the lamps on the **Audio** page.

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

A bank selected in the Media Server's own layer controls plays on every channel layout. An output
still on an older layout, which has no effect-bank channels on the wire, keeps its directly
configured effect slots until a bank is selected there.

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

## Visualizer parameters

A layer showing a generated visualizer carries four **Visualizer Parameters**. They follow the order
the visualizer's kind lists its parameters in the Visualizers editor. **0** keeps the configured
value; **1–255** sweeps the parameter across its range. Colours sweep the hue wheel, switches turn on
at 128, and variants count up from the first.

## Output mirroring

The Master has no Flip/mirror channel in the current channel layout. **Master scale X** and
**Master scale Y** run from −4× to +4× with **1×** at raw value 40960; a negative axis mirrors the
finished output along that axis. Older channel layouts keep their Flip/mirror channel.

## Effect catalogue

The initial catalogue includes TV/CRT/VHS Simulation, Digital Video/ Glitch Simulation, Blur
(Gaussian, Shape, Radial, Linear, and Axial), Feedback, Beat Move, Beat Scan, Beat Scale & Turn,
Beat form Flash, Kaleidoscope, B/W Rasterize, CMYK Rasterize, and Drawn Image Style. Kaleidoscope
repetitions run from Off through 12. Feedback supports Shake and Tunnel motion and is tuned for a
longer, smoother trail by default.

The Master layer has a separate fixed **Effects** section. **Layer Opacity Cycle** advances through
all currently loaded layers whose dimmer is above zero. Its Multiplier / Divider can be Off or beat
divisions/multiples; cycling changes only effective output opacity and never rewrites stored layer
dimmers.

Use the isolated layer preview to identify a bank or mask problem before diagnosing the output
composite. The layer preview and final output use the same bank order and preset settings.

Generated configuration is stored by addressed identity so Desk programming remains stable. Moving or replacing an addressed generated item changes what later recalls produce just as moving uploaded media would.
