# Fixture Models

A fixture uses the 3D model supplied by its profile. When no model is supplied, Architect chooses
a packaged model from its fixture type and channels. You can choose a specific body under
**Show Setup → Fixture Library → Simulation → Body**; see the
[Model Catalogue](../99-Appendix/01-model-catalogue.md).

## Check the physical rig

Check each fixture's size, hanging point, bracket angle and lens direction against the real rig.
Profile dimensions determine its displayed size. **Bracket** tilts a static lantern about its
bracket bolts while its hanging frame and clamp stay in place. Moving heads turn their head;
scanners keep their chassis fixed and move their mirror.

Models include switchable mounting hardware. Hide the clamp when the fixture stands on a floor
base rather than hangs from a truss.

## Check where the light leaves

The glowing source follows the fixture's lens or individual lamp faces. Multi-lamp blinders,
Sunstrips and LED bars retain their separate faces. The fixture profile supplies their optics and
positions; inspect a suspect model in the fixture editor rather than compensating with an
incorrect rig placement.

The **LED PAR, pizza lamp** body shows red, green and blue diode faces driven by the respective
channels. It still produces one combined beam; choosing that body does not create addressable pixels.

## When a model looks wrong

A fallback cannot infer every physical variant from a channel chart. Choose the correct body and
size in the profile, save a new revision, then explicitly update fixtures that should use it.
Existing shows retain their embedded profile revision. If a build lacks a named packaged body,
it uses its normal fallback instead of refusing the fixture.
