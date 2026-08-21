# Generated Sources and Effects

The Media Server can render more than uploaded video and pictures. Addressed text, generated visualizers, masks, and typed effects remain normal library choices that a Desk can store in Cues.

## Text

Text content occupies folders `200`–`249`. Configure the text source and its presentation in the Media administration interface, then recall its numeric folder/file address through the matching Media layer. Keep fonts and other local dependencies available on the production server.

A clock, and any text derived from the clock, follows the server's own UTC offset. Set it once in **Settings > Libraries > Server time** as minutes east of UTC; the offset is stored configuration and applies to the next drawn frame, so a clock on an output follows an accepted change without a restart. It is deliberately independent of the timezone the host operating system is set to, because a show machine is not always configured for the venue it is standing in. A single clock that has to show another city can carry **Own UTC offset**; every other clock keeps following the server.

## Generated visualizers

Generated visualizers occupy folders `250`–`255`. Each visualizer has a stable kind and only exposes parameters that affect that kind. Examples include spectra, waveforms, geometric motion, particles, rays, glitch treatments, digital rain, tunnels, and landscapes.

Audio-reactive visualizers depend on the Media Server's configured audio input and analysis path. A visualizer can render correctly in the browser while reacting incorrectly to silence, the wrong device, or an unsuitable signal level; verify the live input on the production machine.

## Effects and masks

Masks are selected independently from content and can be combined with the layer's shaper and transform. Typed effects retain their own parameter values and apply in the layer's documented order. Use the isolated layer preview to identify an effect or mask problem before diagnosing the output composite.

Generated configuration is stored by addressed identity so Desk programming remains stable. Moving or replacing an addressed generated item changes what later recalls produce just as moving uploaded media would.
