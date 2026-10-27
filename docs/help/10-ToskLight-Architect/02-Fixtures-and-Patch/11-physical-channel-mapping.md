# Calibrate physical channel mapping

Open **Mapping > Details** on a Continuous function to document its physical response.
The function's raw endpoints and physical values define the mapping; decreasing physical values
are valid, such as Zoom running from a wide beam to a narrow one.

Choose **Mapping quality** as Unknown, Estimated, Manufacturer or Measured. Manufacturer and
Measured require a **Mapping source**. Use **Mapping revision** to identify a revised calibration.
A Zoom function explicitly measured in degrees can also declare **Beam angle** or **Field angle**;
leave the convention Unknown when the source does not say which it is.

The default is a straight line between the function endpoints. **Use sampled mapping** adds those
endpoints, then **Add intermediate sample** adds a point on the curve to edit. Raw values must
increase, and physical values must keep moving in the same direction. The first and last samples
must match the function endpoints. Changing an endpoint leaves the samples intact and reports the
mismatch; **Use function endpoints** repairs those two samples explicitly. **Use linear mapping**
removes intermediate samples while retaining quality/source information.

The preview shows raw DMX against physical values and a midpoint readout. Invalid samples stay
visible for correction and block saving. Changing channel resolution keeps the curve's relative
raw positions, unless reducing resolution would merge sample points; that change is refused.
Existing profiles without calibration remain valid and read as Unknown.

The profile and its `.toskfixture` package retain the mapping. Physical Position, Focus, and Zoom resolve through the declared physical data; the editor preview lets you inspect the mapping before saving.
