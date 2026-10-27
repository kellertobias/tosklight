# Create generated visuals

Pixel's **Visualizers** page creates moving pictures for media layers. These generated visuals are separate from the lighting **Visualizer** used in Control and Architect to inspect a rig.

Select an addressed visualizer slot, choose its kind and adjust the controls while watching its live preview. Recall the same folder/file address from a layer or store it in a Control Cue. Moving or replacing a slot changes what that address recalls.

For lettering and clocks use [Text and countdowns](07-text-and-countdowns.md). For effects applied over content use [Effects and masks](08-effects-and-masks.md).

## Tune a generated visual

Generated visuals occupy folders `250`-`255`. Each visualizer has a stable kind and only exposes parameters that affect that kind. Examples include spectra, waveforms, geometric motion, particles, rays, glitch treatments, digital rain, tunnels, landscapes, and nets.

On the **Visualizers** page the picture above a visualizer's settings is live: the Media Server draws it from the stored settings and the audio it hears, so every change shows at once. Settings save as they change; there is no separate save step. A server that cannot draw the preview shows the visualizer kind's reference picture instead.

Audio-reactive visualizers follow the beat and three instruments the Media Server hears: the kick, the snare or clap, and the hi-hat. Beat Explosions and Pulsing Circles jump on each kick, Lightning Tendrils cracks with the snare, and Starfield sparkles with the hi-hats. The **Audio** page shows the same instruments as lamps over their levels, so an operator can confirm what the visualizers will react to before the show.

Audio-reactive visualizers depend on the Media Server's configured audio input and analysis path. A visualizer can render correctly in the browser while reacting incorrectly to silence, the wrong device, or an unsuitable signal level; verify the live input on the production machine.

Each visualizer has **Audio gain** first on the **Visualizers** page and in a layer's **Effects > Visualizer** group. It scales that visualizer's levels, spectrum, waveform and instrument levels. Adjust it to drive one visualizer harder while holding another back on the same input. `1` hears the input as the **Audio** page shows it, `0` hears silence, and the maximum is `8`. Beats and instrument hits still land at any gain; only how large they read changes. **Reactivity** is different: it scales only the movements a kind chose to tie to audio. Audio gain is not a Visualizer Parameter channel.

Several visualizers can follow tempo-locked beats. Before Pixel has a tempo, kicks supply those beats. Repeated detection within half a beat counts once.

| Visualizer | Beat behavior |
| --- | --- |
| Rotating 3D Shape | **React to** chooses **Audio** (swells with the bass and turns steadily) or **Beat** (pops and turns an eighth further on each beat). **Smoothing** eases the bass it follows and how softly each turn settles. |
| Kaleidoscope | **React to** **Beat** turns the pattern half a segment and flashes it on each beat. |
| Minimalist Shapes | **React to** **Beat** spawns one randomly placed, rotating box or circle per beat while there is room under **Count** (up to 50). Each shape keeps its position and fades away; bass expands it. |
| Starfield | **Spawn stars** **On beat** launches a volley of stars from the vanishing point on each beat, over a sparse field that keeps drifting; **Continuously** keeps the full drifting field. |
| Matrix Digital Rain | Each beat sends **Streaks per beat** (0-8, default 2) bright streaks down their own columns. Louder music speeds the rain up smoothly; it never jumps. |
| City Tunnel | Each beat surges the tunnel's frames one slot nearer and flashes them. |
| Grid Landscape | Each beat sends a wave of light along the street lamps from the nearest to the farthest, turning their heads white as it passes. |

Fractal Morph shades from **Start colour**, where points escape at once, to **End colour** along and inside the set, and its **Smoothing** eases the bass that morphs it; `0` follows the bass exactly. Starfield's fourth Visualizer Parameter channel is **Spawn stars**.

Crossing Lines draws a centre-crossing fan with Rotate, Scale, and Shift variants. Radiating Rays draws finite, constant-width spokes whose length follows bass. Pulsing Circles shares one smoothed bass-driven radius across evenly spaced, progressively fainter rings; **Decay** near 1 makes the radius follow more slowly. Morphing Polygon joins individual low-frequency spectrum vertices with straight edges. Wave Terrain draws a tilted 50-by-50 mesh; **Wireframe** shows its rows and columns, and **Size** sets elevation.

Triangular Net reacts at three scales: kicks raise large swells, snares disturb the middle detail, and hi-hats ripple the fine detail. **Decay** sets how each movement settles. In silence the net holds its shape. Check the instrument lamps on **Audio** if it stays still during music.

Flying Props flies music props through the picture: headphones, cassettes, records, microphones, and beamed notes, drawn in **Colour** with their labels and cushions in **Second colour**. The props ship with the visualizer; operator-supplied models are not supported. Its four Visualizer Parameter channels are **Flight pattern**, **Speed**, **Density**, and **Size**:

| Flight pattern | Motion |
| --- | --- |
| 0 - Fly-through | Props come out of the distance toward the camera and fade before they fill the picture |
| 1 - Drift | Props cross the picture sideways at different depths |
| 2 - Orbit | Props circle the middle of the picture on a tilted carousel |
| 3 - Rise | Props float up from below the picture and out of the top |

Values past 3 repeat the list. **Density** sets how many props fly, from 1 to 48, and **Size** how big each one is. The props keep flying in silence. A held kick swells them and pushes their flight forward, and a snare or the beat flash brightens their accents; **Reactivity** scales both, and a loud input cannot enlarge them further than a full kick does.

A new configuration assigns Flying Props to `250/025`. Existing configurations retain their assignments; select an empty slot on **Visualizers** to add it.

**Text Tracking** turns your **Text** into a typed monospace screen with glyph tracking boxes, connecting lines, a red lock flash and a moving camera. Its movement follows Pixel's audio and beat analysis. Text accepts up to 128 ASCII characters, including line breaks, using a bundled monospace font. **Columns**, **Typing speed**, **Letter spacing** and **Camera motion** tune the presentation. New configurations assign it to `250/027`. Existing configurations keep their own visualizer assignments; add the new kind to a free slot on the **Visualizers** page.

**Black Hole** is a generated visualizer with a dark shadow, glowing photon ring, rotating
accretion disc and a bent arc above the shadow. Each beat swells and brightens the ring and disc;
**Beat pulse** controls the strength and **Pulse decay** how long it takes to settle. Zero Beat
pulse keeps the disc rotating without beat pulses. **Shadow radius**, **Disc speed**, **Disc
inclination**, **Disc colour** and **Ring colour** tune its look. A fresh configuration assigns it
to `250/028`; on an existing installation, add Black Hole to a free slot on **Visualizers**.
Its first four Visualizer Parameter channels are Shadow radius, Disc speed, Beat pulse and Pulse
decay.

## Control a visualizer from a layer

Every layer has four **Visualizer Parameter** controls, separate from its two effect banks. **FX1** and **FX2** stay available for ordinary effects on generated visuals and uploaded content.

The visualizer the layer shows defines what each channel means: its name, its range, and its
default. The channels take the first four parameters in the order the Visualizers editor lists
them, so Equalizer Bars reads **Count**, **Size**, **Colour**, and **Second colour**, and
Waveform Oscilloscope reads **Size** (0.005-0.1 of the picture), **Thickness**, **Amount**, and
**Colour**. **0** keeps the default, which is the layer's own tuning when it has one and otherwise the
configured visualizer's value; **1-255** sweeps the parameter across that visualizer's range.
Colours sweep the hue wheel, switches turn on at 128, and variants count up from the first. A
visualizer with fewer than four parameters leaves the remaining channels inert, and a layer showing
library media, text, or nothing ignores all four.

The Media pane's **Effects › Visualizer** tab, ToskLight Control's Media pane, and DMX input all
address these same four bytes, and each names a channel as the shown visualizer does. The
Visualizer tab's settings above the four channels tune the whole visualizer for this layer only;
that tuning belongs to the layer, applies only while the layer shows that visualizer, and
**Reset parameters** returns the layer to the configured values.

Use [Audio](11-audio.md) to select and check the input before tuning an audio-reactive source. Older Galaxy Simulator assignments load as unassigned; other source addresses keep their assignments.
