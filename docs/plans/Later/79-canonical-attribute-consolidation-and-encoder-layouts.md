# Canonical Attribute Consolidation and Encoder Layouts

## Status

**Later — living product plan, planning only.** This document records the current consolidation
direction while the encoder pages are reviewed attribute by attribute. It does not authorize a
registry, fixture-package, show-data, UI, output, or migration change yet. Update this plan as the
review continues; implementation starts only after the unresolved decisions and compatibility
gates below are closed.

This plan follows the fixture-facing-to-canonical mapping seam established by
[Attribute Registry, Activation Groups, and Indexed Presets](../refactoring/finished/29-attribute-registry-and-activation-groups.md).
Manufacturer channels and their exact DMX functions remain lossless in the fixture package even
when several mutually exclusive fixture concepts map to one operator-facing attribute.

## Goal

Reduce the canonical Programmer and hardware encoder surface to controls that describe genuinely
independent things an operator may need at the same time. Do not allocate separate encoder slots
merely because different manufacturers use different channel names, different emitter names, or
different functions on one physical mechanism.

The same ordered attribute model must generate layouts for six-, five-, and four-encoder hardware.
Changing hardware width changes pagination, not fixture capability, Programmer meaning, show
portability, or the number of values that can be recorded.

## Consolidation rule

Consolidate two fixture concepts when all of the following are true:

1. they control the same physical or visual result;
2. a fixture is very unlikely to need both independently at the same instant;
3. one canonical value can be transformed to every supported fixture representation without
   discarding an independently controllable degree of freedom; and
4. a fixture that does expose an exceptional second mechanism can retain it through a numbered or
   custom canonical attribute.

Do not infer redundancy from the shipped library alone. Absence from the current library is a
reason to research the market, not permission to delete a capability.

## Current evidence baseline

The repository audit on 2026-08-01 covered all 48 shipped `.toskfixture` packages and all 260 modes.
The numbers below count distinct fixture profiles and modes containing the named physical/legacy
attribute; they are a bounded ToskLight-library sample, not a claim about the whole market.

| Question | Shipped-library evidence | External primary evidence | Working conclusion |
|---|---:|---|---|
| Separate color-wheel rotation | `color.wheel.1`: 9 fixtures / 27 modes; separate wheel-rotation attributes: 0 | The ROBE DLS virtual wheel includes fixed colors and rainbow-speed ranges in one channel. | Do not reserve a separate rotation encoder. Model selection, indexing, and spin as functions/modes of the numbered wheel attribute unless a surveyed fixture proves two simultaneous controls. |
| CMY as canonical controls | Cyan: 1 fixture / 4 modes; Magenta and Yellow: 0 in the shipped packages | Subtractive CMY is the inverse representation of additive RGB already defined by Plan 29. | Keep CMY fixture-facing and map it inversely to canonical RGB. Remove CMY from recommended encoder pages after compatible show migration. |
| White plus Amber | White: 10 fixtures / 32 modes; Amber: 3 / 10; W+A coexist in 2 Generic profiles / 6 modes | RGBWA fixtures genuinely expose both emitters. | Keep canonical White and Amber independently controllable. |
| Cold plus Warm White | CW and WW: 2 Generic fixtures / 24 modes each; they always coexist with one another and never with canonical White or Amber in the shipped library | Tunable-white products expose a CCT range and may also expose a separate green/magenta adjustment. | Map fixture CW to canonical White and fixture WW to canonical Amber, subject to a broader co-occurrence audit and authored emitter metadata. |
| Explicit Temperature | 7 fixtures / 24 modes | ETC Studio/HSIC and current ARRI products expose CCT directly. | Keep Temperature. A CCT-only fixture is a real and useful case, not another RGB encoder page. |
| Tint | legacy `fixture.tint`: 1 fixture / 1 mode, alongside Temperature | ETC and ARRI define Tint as a green–magenta adjustment independent of CCT. | Preserve Tint as an independent axis inside the canonical color engine and whole-color editor, but do not spend a default encoder on it. Map media tinting there where lossless; do not derive fixture Tint from White/Amber. |
| Continuous Pan/Tilt | 0 shipped fixtures | Current Claypaky Arolla Aqua M-LT has endless Pan and Tilt; other current fixtures have endless Pan only. | Preserve continuous motion, but first try to make absolute and continuous operation modes of the same Pan or Tilt attribute instead of four encoders. Prove channel co-occurrence before migration. |
| Separate Pan Time and Tilt Time | 0 shipped fixtures | ROBE DLS uses one shared Pan/Tilt speed-or-time channel. | Remove the separate `pan.time` and `tilt.time` proposal. Keep one shared movement timing/speed concept pending the semantic decision below. |
| Shared position speed/time | speed/time: 5 fixtures / 19 modes; speed: 2 / 4; time: 1 / 1 | ROBE DLS changes the same channel between vector speed and seconds using fixture mode. | Use one Position Movement encoder. Decide whether its canonical meaning is time, speed, or a typed compound value before implementation. |
| Two prisms plus animation | 0 shipped fixtures | ROBE MegaPointe contains two prism wheels and a separate animation/effect wheel. All three mechanisms can contribute to one look. | Keep Prism 1, Prism 2, and Animation as distinct canonical mechanisms. Do not reinterpret Prism 2 as Animation. |
| Prism selection versus rotation | Shipped Sharpy and DLS profiles have separate Prism and Prism Rotation channels | MegaPointe has separate selection channels and indexing/rotation channels for each of its two prism wheels. ROBE T1 likewise separates Prism from Prism Rotation. | Preserve separate internal values, but place selection and rotation on one dual-mode encoder: turn selects/inserts; push-turn indexes or rotates. Use the same interaction for Animation selection/position and rotation. |
| Two Frost controls | only one legacy Frost: 2 fixtures / 5 modes | Current Claypaky fixtures offer independent light 1° and heavy 5° frost filters; ROBE iForte also identifies Frost 1 and Frost 2 mechanisms. | The mechanisms are physically distinct, but the default desk exposes one canonical Softness control. A profile with additional independently controllable frost mechanisms registers those additional controls as custom attributes. |
| Beam Edge | 0 shipped fixtures | Claypaky fixtures expose beam-edge softening; Vari-Lite exposes Edge separately from Zoom, Diffusion, and Beam. | Map the fixture's chosen primary Frost or Edge mechanism to canonical Softness. Preserve a simultaneously controllable additional Edge or Frost mechanism as a custom attribute. |
| Generic Beam | 0 shipped fixtures | Vari-Lite profiles contain separate Edge and Beam channels, so `beam` is not automatically a duplicate of Focus or Edge. | Do not delete it yet, but remove it from the recommended default page until `beam` has a precise cross-manufacturer definition and at least one verified mapping. |
| Keystone X/Y | 0 shipped fixtures | Christie Pandoras Box documents dynamic keystone correction for media/projection, and projectors expose H/V or corner correction. | Keystone is a Media/Projection positioning concept, not a framing-shutter control. Remove it from Shapers; keep it only in a future media/projector model if required. |
| Shutter and Strobe | Shutter: 12 fixtures / 48 modes; Strobe: 2 / 3; no shipped mode has both attributes | ROBE DLS and many current fixtures put shutter states and strobe ranges on one channel. | Consolidate to one Shutter/Strobe attribute and encoder with indexed open/closed plus continuous/pulse/random functions. |
| Generic Control variants | `control.mode`, `control.speed`, and canonical `control`: 0 shipped; legacy generic control: 2 fixtures / 5 modes; many named special-function channels exist | Manufacturer controls mix reset, lamp, fan, personality, timing, and safety semantics. | There is only one current generic `control`, not “Control 1” and “Control 2.” Fold mode choices into indexed Control functions; route speeds to the feature they time; keep hazardous/transient actions typed and non-recordable. |

Primary examples used in this pass:

- [ROBE Robin DLS Profile manual and DMX protocol](https://www.robe.cz/res/downloads/user_manuals/User_manual_Robin_DLS_Profile.pdf)
- [Claypaky Arolla Aqua M-LT — endless Pan/Tilt and dual frost](https://www.claypaky.it/products/arolla-aqua-m-lt/)
- [Claypaky Arolla Aqua LT — light and heavy frost](https://www.claypaky.it/products/arolla-aqua-lt/)
- [Vari-Lite VL1100 LED DMX map — separate Edge, Diffusion, Beam, and timing](https://www.vari-lite.com/b-dam/vari-lite/products/vl1100-led/guides-and-manuals/VariLite_VL1100_LED_QuickStartGuide.pdf)
- [ETC Source Four LED Series 2 documentation](https://www.etcconnect.com/products/entertainment-fixtures/source-four-led-series-2/documentation.aspx)
- [ARRI SkyPanel S60-C — independent CCT and green/magenta adjustment](https://www.arri.com/en/lighting/led-spotlights/discontinued/s60-c)
- [Christie Pandoras Box help — DMX and dynamic keystone correction](https://www.christiedigital.com/globalassets/resources/public/pandorasbox/pandoras-box-help-rev-5771.pdf)

## Working decision ledger

These entries reflect the current product review and should be edited in place as the review
continues.

| Current attribute(s) | Future operator-facing treatment | State |
|---|---|---|
| `color.cyan`, `.magenta`, `.yellow` | Physical aliases transformed inversely to Red, Green, Blue | Accepted direction; migration required |
| `color.cold_white` | Physical alias of White | Accepted direction; broaden hardware audit |
| `color.warm_white` | Physical alias of Amber | Accepted direction; broaden hardware audit |
| `color.temperature` | Temperature remains independently available | Accepted |
| `color.tint` | Color-family green/magenta axis inside the whole-color editor; not another emitter, not CCT, and not a dedicated default encoder | Accepted clarification |
| `color.wheel.<n>.rotation` | Functions/modes inside Color Wheel `<n>` | Accepted direction; survey exceptional fixtures |
| abstract `color` | Whole-color picker/state, not a dedicated rotary encoder | Proposed |
| Hue/Saturation fixture modes | Convert through the abstract color engine; do not treat a virtual Hue channel as a mechanical wheel without an explicit decision | Needs clarification of the spoken “U/Hue” example |
| `pan` + `pan.continuous` | One Pan control with absolute and continuous modes | Proposed; compatibility proof required |
| `tilt` + `tilt.continuous` | One Tilt control with absolute and continuous modes | Proposed; compatibility proof required |
| `pan.time`, `tilt.time`, `position.speed`, `position.mode` | One Position Movement control; fixture mode becomes an indexed function or profile/configuration choice | Direction accepted; canonical unit unresolved |
| `position.rotation` | Head/element Rotation in Position | Keep; media rotation may map here only after collision audit |
| Gobo 1/rotation, Gobo 2/rotation | Two selection/rotation pairs | Accepted |
| Prism 1/rotation, Prism 2/rotation | Keep two mechanisms and their separate internal values, but use one dual-mode encoder per prism: turn selection/insertion, push-turn indexing/rotation | Accepted |
| Animation 1/rotation | Keep separate from both prisms, but use one dual-mode encoder: turn selection/position, push-turn rotation | Accepted |
| Beam Effect 1 and 2 | Remove the ambiguous fixed attributes. Vendor “Beam FX” can mean a prism or, on GLP JDC1, segmented strobe-pixel macros. Preserve the real mechanism or a custom fixture attribute instead. | Accepted direction |
| generic `beam` | No default encoder until defined | Research hold |
| four blades, each Position + Angle | Eight independent controls across two pages | Accepted |
| `shaper.keystone.x/.y` | Remove from Shapers; future Media/Projection positioning if justified | Accepted direction |
| Focus, Zoom | Independent controls | Accepted |
| Frost 1, Frost 2, Beam Edge | One default canonical **Softness** control. The fixture profile chooses which physical mechanism maps to it; additional simultaneous mechanisms remain custom attributes. | Accepted direction; intentionally simplifies uncommon multi-mechanism fixtures |
| `control.mode`, `control.speed`, `control` | One Control attribute plus typed/indexed functions; feature-specific speed belongs to that feature | Proposed |
| `media.opacity` | Canonical Intensity for a media layer | Accepted direction; collision audit required |
| `media.tint`, `media.grayscale` | Canonical color-engine operations, not dedicated Media encoders | Accepted direction |
| `media.scaling_mode` | Indexed Position or Control function, not a continuous Media encoder | Accepted direction; owner unresolved |
| `media.rotation` | Position Rotation | Accepted direction; collision audit required |
| `media.position.x/.y` | Pan/Tilt-style Position controls | Accepted direction; define unit transform and collision handling |
| Media Folder/File and Mask Folder/File | Keep both coherent source pairs | Accepted |
| Play Mode, Playback Speed, Playback BPM | Keep their media identity but place them in Control | Accepted direction |
| Mask Opacity | Keep its media identity but place it in Intensity | Accepted direction |
| Mask Invert | Keep on Media with the coherent mask source | Accepted direction |
| Media Effects 1–4 | Keep their media identity but place them in Beam; do not merge them with ambiguous vendor Beam FX attributes | Accepted direction |

## White, Amber, and color-temperature mapping

Fixture packages retain the manufacturer's physical emitter identity and add calibrated metadata:

- emitter chromaticity or spectral data where available;
- nominal warm and cold endpoint CCT for tunable-white pairs;
- usable intensity/range and inversion; and
- whether the pair is calibrated as a white-light system or merely consists of two emitters.

The canonical surface exposes White and Amber. A physical cold-white emitter maps to White and a
physical warm-white emitter maps to Amber. This preserves two independently controllable emitter
levels without spending another two encoder slots.

Temperature also remains a canonical Color control because some fixtures expose a direct CCT
channel with no separate emitter control. For a calibrated two-emitter pair, prototype the
following deterministic conversion in reciprocal color temperature rather than interpolating
Kelvin directly:

1. derive the warm fraction from the normalized White/Amber ratio;
2. interpolate between authored cold and warm endpoints in mireds;
3. convert the result back to Kelvin for the canonical Temperature value; and
4. keep total white-light intensity separate from the ratio.

If both emitter levels are zero, endpoint metadata is absent, the requested point is out of gamut,
or the fixture has additional emitters that materially affect the white point, do not invent a
Kelvin value. Use the profile's explicit direct-CCT transform or report the value as unavailable.

Tint is a separate green–magenta axis. It may coexist with Temperature on a tunable-white fixture
and cannot be recovered from only White and Amber. The color engine may calculate a physical Tint
channel from the requested canonical color when the profile provides a calibrated transform, but
Tint must not be silently equated with Temperature, Amber, saturation, or a media grayscale value.

## Width-independent ordered pages

The registry owns one semantic order per encoder group. A layout generator packs that order into
the connected desk's slot count. Paired mechanisms stay adjacent; if a pair would straddle a page,
the generator leaves the trailing slot unused and starts the pair on the next page. Unsupported
attributes leave their stable slot visibly unassigned, and wholly empty pages are omitted, as in
Plan 29.

The tables below are the current suggested pages after the accepted/proposed consolidations. A
blank means intentionally unused capacity, not an attribute deletion.

### Six encoders

| Group/page | E1 | E2 | E3 | E4 | E5 | E6 |
|---|---|---|---|---|---|---|
| Intensity 1 | Intensity | Shutter/Strobe | Mask Opacity | Volume | — | — |
| Color 1 | Red | Green | Blue | White | Amber | UV |
| Color 2 | Lime | Indigo | Mint | Temperature | Color Wheel 1 | Color Wheel 2 |
| Position 1 | Pan | Tilt | Position Movement | Rotation | — | — |
| Beam 1 | Gobo 1 | Gobo 1 Rotation | Gobo 2 | Gobo 2 Rotation | Prism 1 + Rotation | Prism 2 + Rotation |
| Beam 2 | Animation + Rotation | Media Effect 1 | Media Effect 2 | Media Effect 3 | Media Effect 4 | — |
| Shapers 1 | Iris | Blade 1 Position | Blade 1 Angle | Blade 2 Position | Blade 2 Angle | Shaper Rotation |
| Shapers 2 | Blade 3 Position | Blade 3 Angle | Blade 4 Position | Blade 4 Angle | — | — |
| Focus 1 | Focus | Zoom | Softness | — | — | — |
| Control 1 | Control | Play Mode | Playback Speed | Playback BPM | — | — |
| Media 1 | Folder | File | Mask Folder | Mask File | Mask Invert | — |

### Five encoders

| Group/page | E1 | E2 | E3 | E4 | E5 |
|---|---|---|---|---|---|
| Intensity 1 | Intensity | Shutter/Strobe | Mask Opacity | Volume | — |
| Color 1 | Red | Green | Blue | White | Amber |
| Color 2 | UV | Lime | Indigo | Mint | Temperature |
| Color 3 | Color Wheel 1 | Color Wheel 2 | — | — | — |
| Position 1 | Pan | Tilt | Position Movement | Rotation | — |
| Beam 1 | Gobo 1 | Gobo 1 Rotation | Gobo 2 | Gobo 2 Rotation | — |
| Beam 2 | Prism 1 + Rotation | Prism 2 + Rotation | Animation + Rotation | — | — |
| Beam 3 | Media Effect 1 | Media Effect 2 | Media Effect 3 | Media Effect 4 | — |
| Shapers 1 | Iris | Blade 1 Position | Blade 1 Angle | Blade 2 Position | Blade 2 Angle |
| Shapers 2 | Shaper Rotation | Blade 3 Position | Blade 3 Angle | Blade 4 Position | Blade 4 Angle |
| Focus 1 | Focus | Zoom | Softness | — | — |
| Control 1 | Control | Play Mode | Playback Speed | Playback BPM | — |
| Media 1 | Folder | File | Mask Folder | Mask File | Mask Invert |

### Four encoders

| Group/page | E1 | E2 | E3 | E4 |
|---|---|---|---|---|
| Intensity 1 | Intensity | Shutter/Strobe | Mask Opacity | Volume |
| Color 1 | Red | Green | Blue | White |
| Color 2 | Amber | UV | Lime | Indigo |
| Color 3 | Mint | Temperature | Color Wheel 1 | Color Wheel 2 |
| Position 1 | Pan | Tilt | Position Movement | Rotation |
| Beam 1 | Gobo 1 | Gobo 1 Rotation | Gobo 2 | Gobo 2 Rotation |
| Beam 2 | Prism 1 + Rotation | Prism 2 + Rotation | Animation + Rotation | — |
| Beam 3 | Media Effect 1 | Media Effect 2 | Media Effect 3 | Media Effect 4 |
| Shapers 1 | Blade 1 Position | Blade 1 Angle | Blade 2 Position | Blade 2 Angle |
| Shapers 2 | Blade 3 Position | Blade 3 Angle | Blade 4 Position | Blade 4 Angle |
| Shapers 3 | Iris | Shaper Rotation | — | — |
| Focus 1 | Focus | Zoom | Softness | — |
| Control 1 | Control | Play Mode | Playback Speed | Playback BPM |
| Media 1 | Folder | File | Mask Folder | Mask File |
| Media 2 | Mask Invert | — | — | — |

Fixture selection still projects only applicable pages. An RGB fixture therefore does not force
the operator through empty extended-color pages. A fixture with one Frost or Edge mechanism maps
that mechanism to Softness. If it has multiple independently controllable softening mechanisms,
the profile chooses the primary Softness mapping and registers the remaining mechanisms as custom
attributes instead of adding permanent default encoders for an uncommon configuration.

## Application-specific encoder pages under review

Not every encoder page edits fixture attributes. Dynamics and the exploratory Timecode surface
temporarily replace the ordinary attribute deck with controls for the selected application object.
They still need the same four-, five-, and six-encoder packing rules, understandable labels, and
software/hardware parity.

### Dynamics terminology and current behavior

The current Dynamics surface mixes two selectors with three live overrides. Their meanings are:

| Current label | What it actually does | Saved where | Working UI treatment |
|---|---|---|---|
| Dynamic instance | Chooses one exact running copy of a Dynamic. This matters when the same targetless Dynamic is running from different selections, Cues, or Playbacks. Merely changing this selector changes no output. | Navigation only | Keep in the toolbar/dropdown, not a permanent encoder. Rename the surrounding task **Running Dynamic** so “instance” need not carry the explanation alone. |
| Dynamic lane | Chooses one scalar attribute lane inside the Dynamic, such as Intensity, Pan, or Red. Lane-definition edits below apply to this selected lane. Merely selecting a lane changes no output. | Editor selection only | Keep in the lane list/dropdown, not a permanent encoder. |
| Instance Size | Scales this running copy's excursion away from the ordinary static value. `0%` removes the Dynamic contribution, `100%` uses the authored result, and `200%` doubles the excursion subject to attribute bounds. It does not rewrite Top/Bottom, Middle/Amplitude, or keyframes. | Live override on this exact running instance/controller | Keep, with help text **Amount of this running Dynamic**. |
| Instance Speed | Multiplies the complete running copy's clock after the Dynamic's saved base speed and per-lane multipliers. `2×` runs every lane twice as fast; `0.5×` runs it at half speed. It does not edit the reusable Dynamic. | Live override on this exact running instance/controller | Keep, with help text **Overall speed of this running Dynamic**. |
| Instance Phase | Offsets this running copy around its cycle. `90°` moves it one quarter-cycle ahead without changing speed or the definition's fixture phase spread. | Live override on this exact running instance/controller | Keep, with help text **Move this running Dynamic forward/back in its cycle**. |
| Dynamic Off | Stops only the selected running instance/controller rather than every use of the pool Dynamic. | Runtime/Programmer operation | Keep as a press action. |

**Lane Speed** is different from **Instance Speed**. Lane Speed is saved in the reusable Dynamic
and changes only the selected lane relative to the base cycle. Instance Speed is a temporary/live
multiplier over every lane in one running copy. For example, a Pan lane saved at `0.5×` and a
running instance at `2×` produces an effective Pan speed of `1×`; another lane saved at `1×`
produces `2×` in that same instance.

**Curve Width** compresses the selected non-PWM curve into the middle of its cycle without changing
its value range. At `50%`, the curve traverses its complete authored shape during the middle half
of the cycle and holds its boundary values outside that interval. PWM deliberately uses its own
Attack/On/Decay/Off envelope instead of Curve Width.

### Proposed Dynamics pages

Remove the duplicated Dynamic-instance and lane selectors from encoder slots. They already belong
to the toolbar and lane list. The running-instance page then fits every supported hardware width:

| Running Dynamic | E1 | E2 | E3 | E4 | E5 | E6 |
|---|---|---|---|---|---|---|
| Six encoders | Instance Size | Instance Speed | Instance Phase | Dynamic Off | — | — |
| Five encoders | Instance Size | Instance Speed | Instance Phase | Dynamic Off | — | — |
| Four encoders | Instance Size | Instance Speed | Instance Phase | Dynamic Off | — | — |

Reorganize lane editing into one stable structural page followed by one contextual detail page.
The exact scalar labels change with the selected lane mode, but slots do not move within that mode:

| Lane page | E1 | E2 | E3 | E4 | E5 | E6 |
|---|---|---|---|---|---|---|
| Curve 1 — structure | Curve Shape | Top or Middle | Bottom or Amplitude | Curve Width | Lane Speed | Keyframe selector/count when applicable |
| Curve 2 — PWM timing | Attack | On | Decay | Off | Attack interpolation | Decay interpolation |
| Curve 2 — keyframe detail | Keyframe | Value/Source | Keyframe Time | Interpolation | Add/Delete action | — |

**Curve Shape** is one compound choice: normal turn chooses the periodic function (Sinus, Cosinus,
Linear +/−, PWM, or Random), while push-turn chooses the configuration method (Keyframes, Max/min,
or Middle/amplitude). The window's visual curve composer may continue to expose the same choices;
it must not become a separate conflicting value owner.

The PWM timing page describes one cycle:

- **Attack** is the rise time from Bottom to Top;
- **On** is the total high-side portion of the cycle, including Attack under the current model;
- **Decay** is the fall time from Top to Bottom;
- **Off** is the remaining low-side portion and is currently derived as `100% − On`.

Before implementing this page, decide whether operators should see On and Off as complementary
values or whether the stored model should change to four explicit sequential durations. Do not show
four apparently independent encoders while silently rewriting one behind the operator's back.

Five- and four-encoder hardware reflows each contextual page using the common packing rule. On
four encoders, PWM interpolation moves to a following page or into the Attack/Decay pushed mode;
the four primary envelope times remain together.

### Timecode prototype controls

The visible Timecode editor and encoder deck are currently a Storybook-only product-design
prototype with local fake data. The `Later/13-timecode.md` runtime plan remains explicitly not
implementable, and these controls must not be treated as settled persistence or execution proof.

The prototype label is **Beat behavior**, not “Beep behavior.” It appears only for a Speed Group
automation point and currently offers:

- **Keep running** — apply the new BPM/value while preserving the Speed Group's current beat phase;
  and
- **Restart · Beat 1** — apply the point and reset the Speed Group to the beginning of its beat
  cycle so subsequent beat-synchronized behavior realigns there.

Rename this control to **Beat alignment**, with choices **Preserve phase** and **Restart at Beat 1**.
The Timecode plan must still decide whether that restart affects only the Speed Group clock or also
restarts already-running beat-synchronized consumers; the prototype currently proves neither.

Consolidate **Loop Start** and **Loop End** into one dual-value push-turn encoder:

- normal turn edits Loop Start;
- pushed turn edits Loop End;
- the display always shows the complete `start → end` range; and
- clamping preserves `start < end` by at least one frame.

This frees one Timecode timeline slot without hiding either boundary. Absolute Set Value must let
the operator choose Start or End explicitly rather than guessing from the last physical press.

## Implementation phases

### 1. Close the evidence and semantics gaps

- Survey a deliberately broad manufacturer set for every proposed consolidation, including at
  least moving profiles, wash/beam/effect fixtures, tunable-white studio lights, RGB+CCT fixtures,
  moving projectors, and media-server layers.
- Record physical-channel co-occurrence, not only product feature lists. In particular, prove
  whether absolute and continuous Pan/Tilt or media position and physical Pan/Tilt can coexist.
- Define `beam`, canonical Softness and its primary-mechanism selection, Position Movement,
  Position Rotation, media scaling, Hue, and Tint in manufacturer-independent physical terms.
- Decide whether a direct whole-color value appears only in Set Value/color-picker UI or needs a
  rotary representation.
- Do not begin schema work while any merge can destroy two simultaneously controllable values.

### 2. Add explicit many-representation canonical mappings

- Extend fixture-package mapping so mutually exclusive physical representations can target one
  canonical attribute with typed transforms and modes.
- Support indexed, fixed, continuous, rotation, and speed ranges within one physical-mechanism
  attribute where appropriate.
- Reject two independently controlled physical channels mapping to one scalar canonical value in
  the same logical head unless the mapping explicitly defines a reversible compound transform.
- Keep manufacturer names, functions, raw ranges, defaults, Highlight values, and control-action
  safety in the embedded fixture revision.

### 3. Implement calibrated color transforms

- Complete the existing compatible CMY-to-RGB show migration.
- Map physical CW/WW emitters to canonical White/Amber only after co-occurrence validation.
- Add authored emitter endpoint/chromaticity metadata and prototype White/Amber-to-CCT conversion.
- Test direct CCT, two-emitter CCT, RGB-calculated white, and CCT+Tint fixtures separately.
- Use one authoritative conversion for Programmer values, Highlight, output, feedback, Presets,
  Cues, Stage visualization, and fixture-editor preview.

### 4. Consolidate canonical identities with show compatibility

- Introduce a versioned alias/migration table for every retired stable ID.
- Before merging, inspect a show for simultaneous old and new values on the same fixture/head and
  define an explicit conflict choice; never silently select one value.
- Preserve unknown/custom IDs losslessly and do not capture a custom attribute merely because its
  label resembles a built-in attribute.
- Migrate activation groups and preferred placements together with values.
- Test old active shows, named revisions, exported shows, Cue/Programmer/Preset/Dynamics values,
  malformed migrations, startup recovery, Undo, and another desk importing the show.

### 5. Generate four-, five-, and six-encoder layouts

- Replace six-slot assumptions in presentation code with a hardware capability describing slot
  count; keep six as the current default only.
- Generate pages from one semantic order and the pair-aware packing rule above.
- Keep global physical encoder numbers, pushed/alternate values, Set Value, page labels, OSC,
  attached hardware, touch, keyboard, and software-only surfaces in parity.
- Persist semantic placement/order rather than width-specific page numbers wherever possible so a
  show remains portable between four-, five-, and six-encoder desks.
- Define deterministic handling for desk-authored custom placement when the show opens on narrower
  hardware.

### 6. Remap and verify the fixture library

- Rebuild every shipped package through the repository fixture workflow rather than editing ZIPs.
- Add representative profiles that exercise the rare retained cases: dual frost and separate
  Edge/Diffusion with explicit primary Softness plus custom secondary controls, endless Pan and
  Tilt, direct CCT+Tint, two CCT emitters, separate Beam/Edge, and projection/media positioning.
- Validate every documented mode, round-trip package import/export, defaults, Highlight, physical
  values, color systems, geometry, and output.
- Run focused unit/package tests, API/UI acceptance tests for each hardware width, then the real
  desktop and attached-hardware path.

## Acceptance criteria

- A fixture loses no independently controllable physical degree of freedom because two names were
  consolidated.
- The same show produces the same semantic Programmer and output result on four-, five-, and
  six-encoder desks; only page boundaries differ.
- CMY fixtures program through RGB, CW/WW fixtures through White/Amber, and direct-CCT fixtures
  through Temperature with calibrated and documented transforms.
- Tint remains an independent green–magenta adjustment in the whole-color editor wherever the
  fixture supports it, without requiring a permanent encoder slot.
- One Shutter/Strobe encoder exposes all authored open, closed, strobe, pulse, and random functions.
- Absolute and endless position modes remain reachable without allocating four default axis
  encoders unless real co-occurrence proves that necessary.
- Dual-frost fixtures expose both filters; single-frost fixtures expose one.
- Keystone no longer appears among physical framing blades unless a fixture package deliberately
  supplies a compatible custom mapping.
- Generic Control does not make reset, lamp, or safety actions recordable scalar values.
- Old shows either migrate deterministically or stop with an actionable conflict; no value is
  silently dropped, inverted twice, or reassigned by label.

## Open decisions for the continuing review

1. Confirm whether the spoken “U” example meant **Hue**, **UV**, or another color control. Hue is a
   coordinate in a color model; UV is a physical emitter and must not be mapped to a mechanical
   color wheel by accident.
2. Choose the canonical operator meaning and unit for **Position Movement**. A speed percentage and
   an absolute move time are not universally interchangeable without distance and calibrated motor
   data.
3. Define the manufacturer-independent meaning of generic **Beam** and whether it remains built-in,
   is renamed, or becomes a custom/fixture-specific attribute.
4. Decide whether media Scaling Mode belongs to Position Indexed Presets or Control.
5. Decide how media Position X/Y and Rotation coexist with physical Pan/Tilt/Head Rotation on a
   moving projector or digital luminaire.
6. Define the typed effect-parameter navigation that follows Media Effect 1–4 on the Beam group.
