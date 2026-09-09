# Generated Sources and Effects

The Media Server can render more than uploaded video and pictures. Addressed text, generated visualizers, masks, and typed effects remain normal library choices that a Desk can store in Cues.

## Text

Text content occupies folders `200`–`249`. Configure the text source and its presentation in the Media administration interface, then recall its numeric folder/file address through the matching Media layer. Keep fonts and other local dependencies available on the production server.

A clock, and any text derived from the clock, follows the server's own UTC offset. Set it once in **Settings > Libraries > Server time** as minutes east of UTC; the offset is stored configuration and applies to the next drawn frame, so a clock on an output follows an accepted change without a restart. It is deliberately independent of the timezone the host operating system is set to, because a show machine is not always configured for the venue it is standing in. A single clock that has to show another city can carry **Own UTC offset**; every other clock keeps following the server.

## Generated visualizers

Generated visualizers occupy folders `250`–`255`. Each visualizer has a stable kind and only exposes parameters that affect that kind. Examples include spectra, waveforms, geometric motion, particles, rays, glitch treatments, digital rain, tunnels, and landscapes.

Audio-reactive visualizers depend on the Media Server's configured audio input and analysis path. A visualizer can render correctly in the browser while reacting incorrectly to silence, the wrong device, or an unsuitable signal level; verify the live input on the production machine.

## Effects and masks

Masks are selected independently from content and can be combined with the layer's shaper and
transform. Each layer has exactly two ordered effect banks. **Bank 1** is applied before **Bank 2**;
each bank stores an **Effect Select** value and an **Effect Strength**. Select **0** is Off and
**1–255** resolves the matching preset in the Library's Effects tab. A missing or unsupported
preset is reported and bypassed instead of silently substituting another effect.

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
