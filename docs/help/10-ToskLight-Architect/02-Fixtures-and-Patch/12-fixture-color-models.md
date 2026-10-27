# Configure fixture Color models

Configure the fixture's additive, subtractive, hue/saturation, or wheel color system. Every head
has its own section and its own color system, so each cell or pixel of a multi-head fixture is
configured independently. **Copy to other heads** gives every other head that has the same kind of
channels its own copy, bound to that head's channels; each copy can then be edited on its own.
Heads without matching channels, such as a shared master head, keep what they have.

Each color system also declares its **Color Intent calibration**: **Measured** for data taken with a
colorimeter or spectrometer, **Nominal** for datasheet or typical values, **Uncalibrated** when the
data cannot promise a colour. Raise the **Calibration revision** whenever the colour data changes and
name its **Calibration source**. A **Subtractive CMY** system can hold the **Measured filter
output**: the open beam and the beam with each flag fully in. [Color Intent](../../20-ToskLight-Control/20-Programmer-and-Cues/05-color-intent.md)
uses this data to decide how closely the fixture can show a colour and says so.

For a **Discrete color wheel**, **Fill slots from wheel functions** creates one slot for each
named position of the wheel channel. Colors you already defined are kept. Each slot has a
**display color** that the Visualizer shows while the wheel is in that slot. Choose one in the
picker to store it with the profile; it is marked **Defined color**. A slot without a defined color
shows the color its name describes, marked **From the slot name**: Open or White shows white, Deep
Red a dark red, CTO a warm tint. A slot whose name describes no color leaves the fixture's own
color in the Visualizer. **Steady colour for Color Intent** says whether Color Intent may park the
wheel in that slot: left on **Judge by the slot name**, a split, scroll, rotation or effect position
is never used. Each wheel slot gives every field room for its value; in a narrow window
**DMX from** and **DMX to** move onto their own line rather than shrinking. Then use **Generate
portable presets** in **Control → Special Dialog** when fixed or indexed choices should be added to
the show.

## Physical optical paths

The Color tab also contains **Physical optical paths**, one per head. Use **Create physical path**
to start from the head's existing Color controls. Existing emitter values retain their evidence
quality; a missing source or filter measurement stays Unknown. These settings are saved with the
fixture and its portable package. Live Color matching uses them: a requested colour is fitted to
this head's native controls through its optical path, and the Color dialog's approximation reports
the result with the path's evidence quality.

**Native Color controls** lists the exact profile channels participating in Color. Include all
emitters, filters, wheels and color-mode controls. Function selectors identify the actual native
channel and raw interval, including channels whose programmer aliases overlap. Service/reset
functions cannot become recordable Color values.

Choose an Unknown source, a Fixed lamp source, or Additive emitters. An emitter declares its
visible/UV/IR role, XYZ when known, maximum drive, response exponent and native direction. A
reversed native direction means the beginning of that function is full output and the end is off.
For a fixed source, **XYZ known** starts with zero output; enter the actual source values rather
than accepting an invented white. Manufacturer and Measured quality require a source record.

Add filters in the order light passes through them. CMY flags, correction filters and wheels are
separate stages, each attached to an exact channel function. **Move earlier/later** changes the
order while preserving identity and data. An isolated wheel-color XYZ is not a transmission
measurement and cannot predict its combination with another filter. Missing transmission is
reported explicitly. Spectra and whole-path recipe measurements imported in a `.toskfixture`
remain intact; the current form shows their retained sample counts.

Changing a referenced channel's byte layout is refused until its physical path is reconfigured.
Deleting a referenced channel/function leaves a validation error, so incomplete calibration cannot
silently be saved. Updating the optical model revision pins a new appearance model; it does not
rewrite stored show presets.

## UV emitter data

In a physical optical path, select **UV effect** as the emitter purpose and bind its exact native
UV function. Missing color measurements do not mean the UV control is absent. XYZ describes only
its visible output, which may include violet light; leave it unknown unless supported by data.
A measured zero and an unknown value are different. Spectral measurements remain optional.
UV strength and fluorescent scenery appearance cannot be inferred from the visible color alone.

## Optical wheels

The Simulation tab's **Optical wheels** section assigns gobo artwork and prism appearances to
individual wheel and slot numbers. Wheel 1 matches **Gobo 1** or **Prism 1**, wheel 2 matches
**Gobo 2** or **Prism 2**, and so on. Gobo slot zero is open; later slots may carry a name and
an image mask, where white passes light. Each wheel may reuse another wheel's slot numbers.
Prism slots choose **Radial** or **Linear**, the number of copies, and their spread in degrees
from the beam axis. Slot zero stays open. An empty prism list retains the generic appearance.
Export carries these settings and artwork with the fixture revision.
