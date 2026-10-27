# Color Intent contract

For fixture developers and integrations: a Color Intent colour is a CIE 1931 2° XYZ value relative to
the D65 white point, with the white at a luminance *Y* of 1 and every component between 0 and 1.2.
It is stored and sent as `{ "x", "y", "z" }` under the `color` attribute. Only its chromaticity is
the colour; a colour of zero luminance is treated as white rather than as black.

Operator controls and capability feedback are documented in [Color Intent](../help/20-ToskLight-Control/20-Programmer-and-Cues/05-color-intent.md).
