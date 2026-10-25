# Visual lamp response model

TL-867 separates visual lamp inertia from cue timing, DMX output and observer persistence.
Decoded intensity, colour, cell levels and shutter remain the desk's literal targets. Only
renderer-owned source outputs heat and cool; an optical shutter gates that source afterward.
The existing observer persistence preference remains a separate final perception effect.

Explicit installed Tungsten/Halogen selects the nominal response. With ProfileDefault, only
explicit tungsten/halogen/incandescent words in the profile's physical light-source declaration
select it. LED declarations take precedence over emulation wording. Unknown sources, LED,
discharge and other technologies stay instantaneous. A Fresnel/PAR/ACL name, wattage, body shape
or colour temperature alone does not identify its technology. No fixture package is rewritten.

The initial model is an **estimated visual fit**, not measured lamp calibration: exponential
rise with 25 ms time constant; fall with 45 ms time constant above 40% output and 120 ms below.
The 40% knee follows the requested appearance, not a universal physical threshold. The exact
crossing time is integrated; a changed target starts from the current output. Values within
0.0001 of target settle exactly, ending redraw activity. Elapsed-time advancement uses the
whole finite interval, including hidden/backpressured periods, rather than a frame-count delay.
Thermal motion behind a closed optical shutter or a currently black scalar/cell RGB recipe
continues on the CPU without requesting GPU pictures. Eligibility includes a pending positive
target even when output is initially zero. This gate does not claim to detect an installed
black gel; that requires the structural scene's installed-colour transform. Visible observer
persistence tails remain a separate reason for a picture to change.
New/unknown histories initialize at their received equilibrium. Provider replacement retains
local state only for current physical emitter identities; a source/scene reset drops history.
Capture clones resolved outputs before engine selection and never advances the frozen clone.
If the entire process is suspended or packets are unavailable, there is no observed input-edge
history to replay. On resume the model advances its last known target over elapsed time, then
accepts delivered changes at arrival. It does not claim to reconstruct a real lamp's response
to commands it never observed. Standalone capture menu actions freeze the current resolved
picture before the event loop adopts another provider frame; the existing freshness guard stays.
No temperature-dependent red shift or fixture-specific LED smoothing is asserted by this fit.

The evidence supports separating these mechanisms, but does not calibrate the chosen constants:

- [ETC DRd dimmer curves](https://support.etcconnect.com/ETC/Power_Controls/DRd/Dimmer_Curves_for_DRd)
  describe static control-to-RMS-voltage curves and low-level infrared losses. They are not
  temporal filament-response measurements.
- [ETC DimStat manual](https://www.etcconnect.com/workarea/DownloadAsset.aspx?id=10737460821)
  documents load-dependent settings, preheat and separately configurable input/output fades.
- [Torzo, D'Anna and Pecori (2016)](https://cris.unibo.it/handle/11585/593417)
  report filament heat-capacity effects on transient lamp light output.
- [Modelling Miniature Incandescent Light Bulbs (2014), section 2.5](https://link.springer.com/article/10.1007/s10762-014-0130-8)
  reports different heating and cooling responses for a miniature infrared application. Its
  particular bulb temperatures and timing constants must not be copied as visible stage-lamp
  measurements.

Validation must include cold/warm bumps, partial-level reversals, shutter/strobe gating,
different timestep partitions, hidden/backpressure advancement, finite idle settling, multicell
independence, unpatched fixtures, source replacement and frozen Ultra/Ultimate capture. Actual
fixture calibration requires light-sensor traces at declared lamp/dimmer/preheat settings.
