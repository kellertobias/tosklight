# Generated Sources and Effects

Text, generated visualizers, masks and effects use numbered library slots that the desk can recall
in Cues, just like uploaded media. Moving or replacing a slot changes what existing Cues recall.

## Text

Use folders **200–249**. Configure words and appearance on the **Text** page, then recall its
folder/file address on a layer. Keep the chosen fonts installed on the production server.

**Settings > Libraries > Server time** sets the fixed UTC offset used by clocks and countdowns,
in minutes east of UTC (−840 to +840). Changes apply immediately. A clock can use its **Own UTC
offset**. The server offset does not change automatically for daylight saving.

Countdowns can run for a length, to a dated moment, or to a time every day. A daily countdown starts
again at midnight. **After zero** chooses hold at zero, continue negative, or count upward.

## Generated visualizers

Use folders **250–255** and choose a kind on **Visualizers**. The live preview follows saved settings
and incoming audio; settings save as you change them. The selected kind exposes only its relevant
controls. Existing installations keep their assigned slots; add new kinds to unused slots.

Audio-reactive kinds use Pixel's input, tempo, kick, snare/clap and hi-hat detection. Open **Audio**
to verify the instrument lamps and meters before tuning the visualizer.

![Pixel audio analysis shows the signal driving reactive content](../assets/screenshots/workflows/pixel-audio.png)

On macOS, use **Request microphone access** if shown. After a denial, allow Pixel in **System
Settings > Privacy & Security > Microphone**, then **Check microphone access**. **Start audio input**
opens an allowed but inactive stream.

**Audio gain** affects how strongly one visualizer hears levels and spectrum: 0 is silence, 1 is
the monitored input, maximum 8. **Reactivity** instead scales the motion linked to audio. Some kinds
also offer **React to > Beat** or **Spawn stars > On beat** for tempo-driven motion. Check each
kind's live preview rather than assuming every kind reacts to the same instrument.

Useful examples include spectra/waveforms, geometric patterns, tunnels, landscapes and particles.
**Flying Props** offers fly-through, drift, orbit and rise; **Text Tracking** animates up to 128 ASCII
characters; **Black Hole** combines a rotating disc with adjustable beat pulse. New configurations
assign these to 250/025, 250/027 and 250/028 respectively. Galaxy Simulator is retired; its old
assignments become empty without moving other sources.

## Effects and masks

A layer has an independent mask and two ordered effect banks: **Bank 1**, then **Bank 2**.
Configure numbered presets on **Library > Effects**:

1. Select slot **1–255**, choose its effect, name it and set parameters.
2. On a layer, select that slot in an effect bank and raise **Effect Strength**.
3. Tune the bank's four parameter channels as needed.

Effect slot **0** is Off. Editing a library preset changes every bank selecting it. A missing or
unsupported preset is reported and bypassed. **Blur** is an effect-bank preset, not a separate
channel.

For each bank parameter, **0** uses the preset's stored value; **1–255** sweeps its range. The four
channels follow the first four parameters shown in the preset editor; any remaining parameters
stay at their preset values. Counts and choices step through whole values.

Beat effects offer **Detected beat**, **Live beat (tempo pulse)** or an individual instrument under
**React to**. Detected beat follows actual hits; Live beat continues with the tempo through musical
gaps. Silent input produces no detected hits.

### Blur, Outline and Beat Slice Grid

Blur offers Gaussian, Shape, Radial, Linear and Axial styles. **Amount** scales with picture height
so different resolutions soften consistently; zero bypasses it.

**Outline** draws picture edges. Intensity 0 leaves the source unchanged; 0.5 overlays full lines;
1 leaves lines on black. **Line thickness**, hue/saturation and edge sensitivity tune the result.
**Beat depth** and **Beat decay** animate it with music. Its four bank controls are Intensity,
Beat depth, Line thickness and Line hue.

**Beat Slice Grid** interleaves picture slices and text cells. Set Columns, Rows, Beat movement
and Text cells; these are its four bank controls. Choose font, colours and up to eight text entries
in the preset, each up to 512 characters / 16 lines. Text scales proportionally inside each cell.
The selected font must be installed on Pixel; the editor identifies a fallback.

Fresh libraries place Outline at 13 and Beat Slice Grid at 14. Existing occupied slots are kept;
an intentionally emptied library stays empty.

## Blend mode and strobe

Blend mode combines the layer with those below: Normal, Add, Screen, Multiply, Overlay, Difference,
Lighten or Darken. DMX values 0–127 choose these in bands of sixteen. Values 128–249 strobe from
1–25 Hz with Normal blending. Values 250–255 return to Normal without strobe.

The Master's fixed **Layer Opacity Cycle** cycles effective opacity through loaded layers above
zero dimmer at the chosen beat ratio. It does not rewrite their stored dimmers.

## Playback range

**In point** counts from the clip's start; **Out point** counts backward from its end. Zero on both
plays the whole clip. Loop, bounce and single-pass modes stay within that range. Stills, text and
visualizers ignore it.

Enter `mm:ss.ff` using the frame rate in **Settings > Libraries > In and Out points** (default
25 fps, adjustable 1–120). For example, 250 frames at 25 fps means 10 seconds, regardless of the
source clip's frame rate. Changes to this server-wide rate apply immediately to playing layers.

An In point beyond the clip holds its last frame. If Out would precede In, playback continues to
the clip's end. Changing the range retains the current playhead when it remains inside it.

## Visualizer parameters

Each layer has four **Visualizer Parameter** channels, separate from both effect banks. The shown
visualizer supplies their names and ranges. **0** keeps that layer's tuning, or the configured
visualizer value; **1–255** sweeps the range. Unused channels do nothing.

On **Effects > Visualizer**, settings above those channels tune this layer only. **Reset parameters**
restores the configured values. An ordinary media layer ignores all four Visualizer Parameters.

## Mirror the output

Use negative **Master scale X** or **Master scale Y** to mirror the finished picture. Both span
−4× to +4×, with 1× at DMX 40960. There is no separate Master flip channel.
