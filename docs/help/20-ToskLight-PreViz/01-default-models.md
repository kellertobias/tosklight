# Default Models

A fixture profile may name its own 3D model. Most do not — the shipped library, an MVR
import and a hand-built profile all commonly arrive with nothing but channels — so the
Visualizer chooses one of the models ToskLight ships instead of drawing a box.

The choice never looks at a manufacturer or a product name. It looks at what the profile
says about itself, in two passes.

## First: the declared fixture type

If the profile declares a fixture type, that decides it. A profile that calls itself a
blinder is drawn as a blinder even if its author gave it a colour mixer.

| Fixture type contains | Model |
| --- | --- |
| `hazer`, `haze`, `fogger`, `fog`, `smoke` | `hazer` |
| `laser`, `lasers` | `show-laser` |
| `scanner`, `mirror` | `scanner-mirror-spot` |
| `blinder` | `blinder-4-cell` |
| `strobe` | `led-strobe` |
| `strip`, `sunstrip`, `pixel`, `bar` | `led-strip-rgbcct-1000` |
| `fresnel` | `fresnel-barn-doors` |
| `par`, `parcan`, `acl` | `led-par-x-in-1` with colour mixing, otherwise `par-64-short-nose-black` |
| `wash` | `moving-head-led-wash-400` if it moves, otherwise `led-par-x-in-1` |
| `profile`, `spot`, `ellipsoidal`, `beam` | `moving-head-profile` if it moves, otherwise `profile-spot` |

A laser is decided by its type alone. Nothing in a channel set distinguishes one — a show
laser's chart calls the position of its figure inside the scan field Pan and Tilt, which
reads as a moving head to every rule below — so a projector whose profile does not say it
is a laser is neither drawn nor rendered as one.

## Then: the attributes it has channels for

A profile with no type, or a type nobody recognises, is read from its channel set. The
rules are applied in this order, because the tests are not mutually exclusive — a moving
head with both a gobo wheel and a colour mixer is a profile, and a fixture with a strobe
channel and an RGB mixer is an LED PAR that happens to strobe.

| The mode has | Model |
| --- | --- |
| A fog, haze or smoke channel | `hazer` |
| Pan and Tilt, plus a colour wheel or a gobo wheel | `moving-head-profile` |
| Pan and Tilt, plus colour mixing and no gobo | `moving-head-led-wash-400` |
| Pan and Tilt, and nothing else to go on | `moving-head-wash` |
| Colour mixing, no Pan or Tilt | `led-par-x-in-1` |
| A Strobe channel, no colour mixing, no Pan or Tilt | `led-strobe` |
| Nothing but a level | `fresnel-barn-doors` |

Colour mixing means RGB or CMY: a subtractive head is still a mixing head, and neither is
a gobo spot.

A bare dimmer channel becomes a Fresnel because a dimmer channel is usually feeding a
lantern nobody described, and a Fresnel is the one that looks least wrong standing in for
any of them.

## What a model brings with it

Sizes are real. Each model is authored in metres at the size of the product it represents,
and a fixture profile that declares its own physical dimensions is drawn at those instead —
so a profile whose dimensions disagree with its model is drawn smaller than the rig around
it. Copy the size from the [Model Catalogue](02-model-catalogue.md) into the profile if you
want them to agree exactly.

Every model that flies carries its rigging as one switchable part: a half-coupler at the
hanging point with its safety bond. A fixture the show has on a floor base rather than on
a bar can have that part hidden without touching the rest of the model.

Fixtures also declare where their hardware swivels — the bracket bolts a static lantern
tilts on, and the hinge of each barn-door leaf — so those parts can be aimed rather than
being frozen where they were modelled.

## Where the light leaves the fixture

Light leaves a lamp through a face, and the Visualizer draws that face rather than a glowing
point: a moving head lights up as its front lens, a Fresnel as its lens, a blinder or a
sunstrip as the row of round lamp lenses it is built from, and an LED strip or pixel bar as
the lit front of the strip. The face stands across the aim, so it turns with pan and tilt and
it is seen edge-on from the side, exactly as the real fixture is.

Its shape and size come from the profile's own optics block when the library declares one.
When the profile says nothing — which is the common case — the declared fixture type decides,
bounded by two things it can never exceed: the body of the fixture that carries it, and the
spacing between neighbouring lamps of the same fixture, so a bank of lamps stays a row of
lamps instead of merging into one bright tube.
