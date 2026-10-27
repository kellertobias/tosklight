# Calibrate an installed fixture

In **Show Patch**, open the physical fixture row's **Pan / Tilt** editor, then
**Position calibration…**. Each multi-patch copy has its own calibration. Enter signed,
unwrapped **Pan zero offset (°)** and **Tilt zero offset (°)**, quality, source and revision.
Manufacturer and Measured quality require a source. **Clear calibration** returns that physical
instance to missing calibration; the fixture-library profile remains unchanged.

The offsets are stored with the portable show/patch and survive edits in Architect. When Position
is programmed in degrees or as a Target, the desk applies them while converting to Pan/Tilt DMX, so
they change the DMX output and the Stage beam that follows it. Existing Pan/Tilt inversion and
mounting rotation remain separate settings; the resolver applies direction and zero correction
once.

## Installed Color calibration

In **Show Patch**, use **SET** or the context menu on a physical fixture's **Light source** cell. Open its calibration action. With no Stage geometry emitter, choose **Color calibration…** directly. A root fixture and each multi-patch copy hold independent observations.

Add an emitter's **output gain** when its output differs from the profile: 1.0 keeps the profile
output and 0.0 represents no output. This scales the emitter's XYZ and spectral power together;
it does not describe a hue change or invent missing emitter data. For CMY, wheels or other
compound optical paths, add a **whole-path observation** with output XYZ and the complete native
recipe used to measure it, including parked wheels. Recipe defaults are an editing template;
these controls send no commands to a lamp. Record quality, evidence source and revision honestly.
Manufacturer and Measured require a source.

Observations retain the exact profile revision, mode and optical path. If the patched fixture
changes, incompatible observations remain saved but inactive. Clear them before authoring new
ones for that lamp. A physical hardware swap using the same profile also needs a calibration
review. New Architect duplicates start without Color calibration. **Clear Color calibration**
removes observations from only the selected physical instance.

Compatible observations apply to live Color fitting and the installed fixture's physical appearance. Stale observations stay stored but inactive. Stored Preset and Cue Color intent remains unchanged.

## Physical geometry and individual axis calibration

In the Fixture Library's **Geometry** editor, **Declare physical geometry** records the
coordinate convention and the quality of the supporting measurements. Geometry uses local
millimetres with Y up; the neutral beam points along −Y. A bracket can be unknown, fixed, or
an authored hinge with a body, pivot and axis. An unknown pivot must not be labelled measured.
Physical movement paths need identity scale; bake visual model scale into the actual dimensions.

In a mode's **Emitters & Motion** editor, **Configure physical Position** connects a physical
axis to an exact native channel function. Configure the function's angular motion and units
first. Absolute angle uses degrees; continuous rotation speed uses degrees per second.
Multiple turns remain signed angles, and a speed-only axis cannot provide an absolute position.

In Patch, open **Pan / Tilt → Position calibration** for the physical lamp or copy. Individual
axis overrides replace its usual Pan/Tilt zero and direction together. Each override belongs to
that profile, mode and physical geometry. Replacing the lamp or changing its geometry retains
old measurements as inactive; **Clear axis overrides** removes that set without discarding the
family offsets. Copies have independent calibration. New duplicated hardware starts without
installed measurements.

These declarations and corrections drive fixture-independent Position programming: Angles and
Target values reach each lamp's DMX through its own calibrated model, and Stage aims the beam with
the same model.
