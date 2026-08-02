# Canonical Attribute Consolidation, Encoder Layouts, and Configurable Control Screens

## Status

**Next priority 5 — living product plan, planning only.** This document consolidates the former
canonical-attribute/encoder plan and configurable-control-screen plan into one story. It does not
authorize a registry, fixture-package, show-data, UI, output, settings, screen, or migration change
yet. Update this plan as the review continues; implementation starts only after the operator's
current-layout review and the compatibility gates below are closed.

This plan follows the fixture-facing-to-canonical mapping seam established by the completed
Attribute Registry, Activation Groups, and Indexed Presets work in the
[major refactoring execution](../Done/major-refactoring-execution.DONE.md).
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

The same work must replace the current fixed-six-encoder attribute settings with an operator-grade
layout editor and let a desk show four or six visible software encoders on the main screen or one
configured secondary screen. Canonical attribute meaning, semantic ordering, settings presentation,
page packing, and the rendered control surface are one product story and must not be implemented as
independent registries.

## Operator review gate for the current settings

Before the final settings design is specified or implementation begins, review the real
**Show > Desk Setup > Programmer > Attributes** and **Screens & playback** surfaces with the
operator. The current Attributes UI is a long sequence of encoder-group lists. Each row mixes the
attribute label, stable ID, fixed page/slot text, activation-group selector, and—only for custom
attributes—inline placement controls. It describes exact six-encoder positions and does not give a
clear visual model of the resulting encoder pages. The operator has already identified that this
does not look good enough.

The review must settle the visual hierarchy and interaction for:

- browsing an encoder group as actual ordered pages and positions rather than a diagnostic list;
- distinguishing canonical built-ins, fixture-provided custom attributes, retired identities, and
  activation-group membership without putting every field at equal visual weight;
- moving or grouping attributes, editing compound push/turn values, and previewing the resulting
  four- and six-encoder layouts;
- creating and editing custom attributes without making the normal built-in view read like a raw
  schema editor;
- showing validation, conflicts, migration consequences, and unavailable fixture capabilities;
  and
- fitting this editor naturally into Programmer settings while Screens & playback owns screen
  placement and visible encoder count.

Do not infer final geometry, drag behavior, modal structure, or labels from the current component.
Record the operator-approved settings layout in this plan before coding it. The resulting UI must
use the shared settings and form primitives, remain touch and keyboard accessible, and make the
saved semantic order visibly correspond to the pages an operator receives.

## Desk Settings information architecture

The current flat **Programmer** page and miscellaneous settings placement are replaced by the
following operator-approved hierarchy. These are operator-facing labels and grouping contracts,
not suggestions.

### Network & Inputs

The page title and sidebar label are exactly **Network & Inputs**. Remove the nested generic
**Inputs** and **Services** wrappers. Each responsibility is one peer settings group, in this order:

1. **ToskLight server connection** — server URL, connect action, and REST/WebSocket connection
   status.
2. **Control inputs** — the current MIDI inputs, OSC, and RTP-MIDI controls. Sound-to-Light audio
   does not appear here. The native-extension migration will soon remove built-in MIDI and
   RTP-MIDI, leaving OSC as the core control input without requiring another page redesign.
3. **Sound input** — Sound-to-Light audio-device selection, microphone permission state,
   **Request microphone access**, refresh, assignment, and actionable errors.
4. **MQTT connection** — the reserved future position for installation-owned MQTT configuration.
   Do not render an empty or non-functional group before MQTT exists.
5. **Matter bridge** — one group containing enablement, server/transport state, commissioning,
   pairing data, assigned Playback count, and limitations. Remove the outer **Services** group and
   rename the inner **Matter playback bridge** surface to **Matter bridge**; it must not appear as a
   group nested inside another group.

This reorganization changes presentation and navigation only. It must not merge control-input and
Sound-to-Light persistence, move browser microphone permission into server configuration, or alter
the server-authoritative ownership of Matter, OSC, MIDI, or RTP-MIDI.

### Preferences

Remove **Programmer** as one long top-level page. **Preferences** becomes a visible sidebar group
with these four child pages shown directly beneath it:

1. **Defaults**
2. **Attributes & encoders**
3. **Highlight**
4. **Others**

If the navigation implementation calls these child routes rather than subpages internally, the
operator-visible hierarchy and labels remain the same. Each page has its own heading, focused
content, loading/error state, save behavior, and keyboard/touch navigation. Moving between child
pages must not discard unsaved changes or create unrelated save operations.

#### Defaults

The **Defaults** page contains:

- **Record defaults**;
- **Update defaults**; and
- **Pool color defaults**, moved from the former standalone Preferences page and no longer hidden
  away from the other default behaviors.

The server-wide pool palette remains separate from per-surface **Type colors** versus
**Individual colors** choices. Moving the default palette changes neither its ownership nor
portable show data.

#### Attributes & encoders

The **Attributes & encoders** page has exactly three tabs in this order:

1. **Encoder groups**
2. **Attribute activation groups**
3. **Custom attributes**

**Encoder groups** is the normal operator view of the canonical layout. It must present actual
ordered encoder groups, logical pages, compound push/turn controls, and four-/six-position results
visually. The current diagnostic list of label, stable ID, fixed page/slot text, activation selector,
and inline custom controls is not an acceptable UX. Stable IDs and migration diagnostics remain
available as secondary detail rather than becoming the primary interface.

**Attribute activation groups** explains that exactly one member is active within a group and that
groups never cross encoder tabs. It shows membership and conflicts as a coherent group editor,
rather than repeating an unexplained activation selector on every ordinary attribute row.

**Custom attributes** owns creating, naming, typing, grouping, ordering, placement, validation,
retirement, imported-attribute mappings, and other custom-only controls. Ordinary built-ins must
not be visually mixed with a long inline custom-attribute form.

An imported GDTF or other fixture-source attribute does not automatically become a new custom
attribute merely because its source identifier differs from ToskLight's canonical identifier. This
tab lists imported source attributes and lets the operator choose **Map to existing attribute**,
targeting either a canonical built-in or an existing custom attribute. For example, source
`GDTF:Gobo` may map to canonical `gobo.1`.

Mapping identity uses the source format plus its stable source attribute identifier, not a
case-insensitive display-name guess. The fixture package retains the original GDTF attribute,
channel functions, ranges, and source metadata while its programming projection targets the chosen
ToskLight attribute. A remembered mapping applies to later imports of that same source identity;
applying it to already imported fixture revisions requires a previewed, explicit remap.

Remembered source mappings are installation/fixture-library import preferences. The resolved target
mapping and any required custom-attribute definition are embedded in the resulting fixture revision
and portable show snapshot, so another desk can use the fixture without inheriting the importing
desk's entire mapping-preference table.

If no suitable target exists, **Create custom attribute** preserves the imported attribute and
requires its type, encoder group, semantic order/placement, activation group where applicable, and
display metadata to be completed. An unmapped attribute is never silently discarded or assigned to
a similar-looking built-in. Two independently controllable channels in one logical head cannot be
mapped onto one scalar target unless an explicit reversible compound mapping resolves the conflict.

All three tabs edit one authoritative attribute configuration. A change made in one tab must be
visible in the others without creating independent copies or allowing conflicting page and
activation-group authority.

#### Highlight

The **Highlight** page contains two peer groups:

- **Highlight look** — the existing semantic intensity, open shutter, color, iris, zoom, focus,
  frost, compatibility, and unsupported-fixture feedback.
- **Highlight patch** — the current Show Patch highlight setting, renamed and expressed with the
  literal choices **Stage only** and **Stage and DMX**.

The Highlight patch setting affects highlighting fixtures selected in Show Patch. Stage feedback
remains available in both modes; the second mode additionally emits the authoritative Highlight
look through DMX. It is Highlight behavior and no longer appears under an unrelated Show Patch
heading.

#### Others

The **Others** page contains the remaining Programmer preferences:

- **Command line timing**; and
- **Preload capture**, including Programmer, physical Playback, and Virtual Playback capture.

Do not add unrelated settings to this page merely because they do not yet have an owner. If this
page grows, give the new behavior an intentional page or group.

### Remove Users & sessions

Remove **Users & sessions** from Desk Settings navigation and remove its current page. Do not move
its user cards or current-operator switcher into Preferences. This change removes the settings
surface only; it does not delete persisted users, session isolation, current-user authority, or
the backend user/session model. A future user-management or operator-switching surface requires its
own explicit product contract.

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
| Shared position speed/time | speed/time: 5 fixtures / 19 modes; speed: 2 / 4; time: 1 / 1 | ROBE DLS changes the same channel between vector speed and seconds using fixture mode. | Use one typed Position Movement encoder that preserves and displays the fixture-authored speed or time representation without universal conversion. |
| Two prisms plus animation | 0 shipped fixtures | ROBE MegaPointe contains two prism wheels and a separate animation/effect wheel. All three mechanisms can contribute to one look. | Keep Prism 1, Prism 2, and Animation as distinct canonical mechanisms. Do not reinterpret Prism 2 as Animation. |
| Prism selection versus rotation | Shipped Sharpy and DLS profiles have separate Prism and Prism Rotation channels | MegaPointe has separate selection channels and indexing/rotation channels for each of its two prism wheels. ROBE T1 likewise separates Prism from Prism Rotation. | Preserve separate internal values, but place selection and rotation on one dual-mode encoder: turn selects/inserts; push-turn indexes or rotates. Use the same interaction for Animation selection/position and rotation. |
| Two Frost controls | only one legacy Frost: 2 fixtures / 5 modes | Current Claypaky fixtures offer independent light 1° and heavy 5° frost filters; ROBE iForte also identifies Frost 1 and Frost 2 mechanisms. | The mechanisms are physically distinct, but the default desk exposes one canonical Softness control. A profile with additional independently controllable frost mechanisms registers those additional controls as custom attributes. |
| Beam Edge | 0 shipped fixtures | Claypaky fixtures expose beam-edge softening; Vari-Lite exposes Edge separately from Zoom, Diffusion, and Beam. | Map the fixture's chosen primary Frost or Edge mechanism to canonical Softness. Preserve a simultaneously controllable additional Edge or Frost mechanism as a custom attribute. |
| Generic Beam | 0 shipped fixtures | Vari-Lite profiles contain separate Edge and Beam channels, so a source label `Beam` is not automatically a duplicate of Focus or Edge. | Remove the undefined scalar `beam` from canonical built-ins. Preserve an imported source attribute by explicitly mapping it to the correct existing attribute or creating and placing a custom attribute. |
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
| abstract `color` | The complete visible color selected in the existing color picker and stored as canonical RGB/color-engine state; it is not a fixture channel or physical emitter | Keep in Set Value/color-picker UI; no dedicated rotary encoder |
| `color.hue` + `color.saturation` | Paired physical coordinates used by fixtures with native HS/HSI-style control; map them through the canonical whole-color color-wheel/picker representation rather than indexed physical Color Wheel 1/2 attributes | Accepted; keep distinct from physical UV |
| `color.uv` | Physical ultraviolet/non-visible emitter, programmed directly where a fixture provides it | Keep distinct from Hue and from visible whole-color matching |
| `pan` + `pan.continuous` | One Pan control with absolute and continuous modes | Proposed; compatibility proof required |
| `tilt` + `tilt.continuous` | One Tilt control with absolute and continuous modes | Proposed; compatibility proof required |
| `pan.time`, `tilt.time`, `position.speed`, `position.mode` | One typed Position Movement control that displays and edits the representation authored by the fixture: speed percentage or move time. Do not make a universal conversion between them. | Accepted |
| `position.rotation` | Head/element Rotation in Position | Keep; media rotation may map here only after collision audit |
| Gobo 1/rotation, Gobo 2/rotation | Two selection/rotation pairs | Accepted |
| Prism 1/rotation, Prism 2/rotation | Keep two mechanisms and their separate internal values, but use one dual-mode encoder per prism: turn selection/insertion, push-turn indexing/rotation | Accepted |
| Animation 1/rotation | Keep separate from both prisms, but use one dual-mode encoder: turn selection/position, push-turn rotation | Accepted |
| Beam Effect 1 and 2 | Remove the ambiguous fixed attributes. Vendor “Beam FX” can mean a prism or, on GLP JDC1, segmented strobe-pixel macros. Preserve the real mechanism or a custom fixture attribute instead. | Accepted direction |
| generic scalar `beam` | The literal continuous percentage attribute named `beam`, not the Beam encoder group containing Gobos, Prisms, and Animation. No shipped fixture currently maps this scalar. | Remove as a canonical built-in. An imported source `Beam` must map explicitly to a meaningful existing attribute or become a placed custom attribute. The Beam group remains unchanged. |
| four blades, each Position + Angle | Eight independent controls across two pages | Accepted |
| `shaper.keystone.x/.y` | Remove from Shapers; future Media/Projection positioning if justified | Accepted direction |
| Focus, Zoom | Independent controls | Accepted |
| Frost 1, Frost 2, Beam Edge | One default canonical **Softness** control. The fixture profile chooses which physical mechanism maps to it; additional simultaneous mechanisms remain custom attributes. | Accepted direction; intentionally simplifies uncommon multi-mechanism fixtures |
| `control.mode`, `control.speed`, `control` | One Control attribute plus typed/indexed functions; feature-specific speed belongs to that feature | Proposed |
| `media.opacity` | Canonical Intensity for a media layer | Accepted direction; collision audit required |
| `media.tint`, `media.grayscale` | Canonical color-engine operations, not dedicated Media encoders | Accepted direction |
| `media.scaling_mode` | Indexed function on the Control encoder page, not a continuous Media encoder | Accepted |
| `media.rotation` | Application-specific Position Rotation on the media logical head | Accepted; it does not replace physical head rotation |
| `media.position.x/.y` | Application-specific X/Y Position controls on the media logical head | Accepted; a moving projector exposes physical Pan/Tilt and media transforms as separate heads |
| Media Folder/File and Mask Folder/File | Keep both coherent source pairs | Accepted |
| Play Mode, Playback Speed, Playback BPM | Keep their media identity but place them in Control | Accepted direction |
| Mask Opacity | Keep its media identity but place it in Intensity | Accepted direction |
| Mask Invert | Keep on Media with the coherent mask source | Accepted direction |
| Media Effects 1–4 | Not implemented; keep only a deferred reminder to decide their future group and parameter navigation when Media Effects become real | Deferred; no current encoder slots or placeholder controls |

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
| Beam 2 | Animation + Rotation | — | — | — | — | — |
| Shapers 1 | Iris | Blade 1 Position | Blade 1 Angle | Blade 2 Position | Blade 2 Angle | Shaper Rotation |
| Shapers 2 | Blade 3 Position | Blade 3 Angle | Blade 4 Position | Blade 4 Angle | — | — |
| Focus 1 | Focus | Zoom | Softness | — | — | — |
| Control 1 | Control | Play Mode | Playback Speed | Playback BPM | Scaling Mode | — |
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
| Shapers 1 | Iris | Blade 1 Position | Blade 1 Angle | Blade 2 Position | Blade 2 Angle |
| Shapers 2 | Shaper Rotation | Blade 3 Position | Blade 3 Angle | Blade 4 Position | Blade 4 Angle |
| Focus 1 | Focus | Zoom | Softness | — | — |
| Control 1 | Control | Play Mode | Playback Speed | Playback BPM | Scaling Mode |
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
| Shapers 1 | Blade 1 Position | Blade 1 Angle | Blade 2 Position | Blade 2 Angle |
| Shapers 2 | Blade 3 Position | Blade 3 Angle | Blade 4 Position | Blade 4 Angle |
| Shapers 3 | Iris | Shaper Rotation | — | — |
| Focus 1 | Focus | Zoom | Softness | — |
| Control 1 | Control | Play Mode | Playback Speed | Playback BPM |
| Control 2 | Scaling Mode | — | — | — |
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
| Curve 2 — PWM timing | Attack | On/Hold | Decay | Attack interpolation | Decay interpolation | — |
| Curve 2 — keyframe detail | Keyframe | Value/Source | Keyframe Time | Interpolation | Add/Delete action | — |

**Curve Shape** is one compound choice: normal turn chooses the periodic function (Sinus, Cosinus,
Linear +/−, PWM, or Random), while push-turn chooses the configuration method (Keyframes, Max/min,
or Middle/amplitude). The window's visual curve composer may continue to expose the same choices;
it must not become a separate conflicting value owner.

The PWM timing page describes one cycle but exposes only three timing values, never four:

- **Attack** is the rise time from Bottom to Top;
- **On/Hold** is the editable high-side portion;
- **Decay** is the fall time from Top to Bottom;
- **Off** is the derived low-side remainder.

Attack plus On/Hold plus Decay determine the remaining Off portion. Do not show Hold and Off as
independently editable because Off is derived within a fixed cycle. Show the derived Off value
visibly. A possible future compound encoder may choose how On time is calculated, but that mode
selector is not part of the current implementation.

Five- and four-encoder hardware reflows each contextual page using the common packing rule. PWM
interpolation may move to a following page or into the Attack/Decay pushed mode; Attack, the chosen
On/Hold value, and Decay remain together.

### Timecode prototype controls

The visible Timecode editor and encoder deck are currently a Storybook-only product-design
prototype with local fake data. The [Timecode](10-timecode.md) runtime plan remains explicitly not
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
  whether absolute and continuous Pan/Tilt coexist. Physical Pan/Tilt and media X/Y/rotation remain
  separate when a moving projector models them as distinct logical heads.
- Define canonical Softness and its primary-mechanism selection, Position Rotation, and Tint in
  manufacturer-independent physical terms. Hue/Saturation, Position Movement, and media Scaling
  Mode follow the settled decisions above.
- Keep direct whole-color selection in Set Value/color-picker UI without a dedicated rotary
  representation.
- Do not begin schema work while any merge can destroy two simultaneously controllable values.

### 2. Add explicit many-representation canonical mappings

- Extend fixture-package mapping so mutually exclusive physical representations can target one
  canonical attribute with typed transforms and modes.
- Add source-identity mappings for GDTF and other imports. Resolve an imported attribute to an
  operator-selected existing built-in/custom target before offering custom-attribute creation, and
  preserve the original source definition either way.
- Let **Custom attributes** create, inspect, change, preview, and remove those mappings. Remapping an
  existing fixture revision is explicit and versioned; it never silently rewrites a live profile or
  portable show.
- Support indexed, fixed, continuous, rotation, and speed ranges within one physical-mechanism
  attribute where appropriate.
- Reject two independently controlled physical channels mapping to one scalar canonical value in
  the same logical head unless the mapping explicitly defines a reversible compound transform.
- Keep manufacturer names, functions, raw ranges, defaults, Highlight values, and control-action
  safety in the embedded fixture revision.

### 3. Implement calibrated color transforms

- Complete the existing compatible CMY-to-RGB show migration.
- Map a fixture's native physical Hue/Saturation or HSI channels through the canonical whole-color
  engine: the operator uses the color-wheel/picker UI and output converts that selected color back
  to the fixture's authored H/S representation. Do not map these channels to indexed physical Color
  Wheel 1/2 or to UV.
- Map physical CW/WW emitters to canonical White/Amber only after co-occurrence validation.
- Add authored emitter endpoint/chromaticity metadata and prototype White/Amber-to-CCT conversion.
- Test direct CCT, two-emitter CCT, RGB-calculated white, and CCT+Tint fixtures separately.
- Use one authoritative conversion for Programmer values, Highlight, output, feedback, Presets,
  Cues, Stage visualization, and fixture-editor preview.

### 4. Consolidate canonical identities with show compatibility

- Introduce a versioned alias/migration table for every retired stable ID.
- Retire the undefined canonical scalar `beam`. Existing stored uses migrate losslessly to a
  compatibility custom attribute with their values and placement preserved; new imported `Beam`
  source attributes require an explicit existing-target mapping or custom-attribute definition.
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
- Native Hue/Saturation fixtures program through the whole-color picker and render back to their
  physical H/S channels, while UV remains a separate physical emitter value.
- Tint remains an independent green–magenta adjustment in the whole-color editor wherever the
  fixture supports it, without requiring a permanent encoder slot.
- One Shutter/Strobe encoder exposes all authored open, closed, strobe, pulse, and random functions.
- Absolute and endless position modes remain reachable without allocating four default axis
  encoders unless real co-occurrence proves that necessary.
- Dual-frost fixtures expose both filters; single-frost fixtures expose one.
- Keystone no longer appears among physical framing blades unless a fixture package deliberately
  supplies a compatible custom mapping.
- Generic Control does not make reset, lamp, or safety actions recordable scalar values.
- The undefined scalar `beam` is absent from canonical built-ins without removing the Beam encoder
  group or its Gobo, Prism, and Animation controls.
- Mapping `GDTF:Gobo` to existing `gobo.1` imports the physical channel into that canonical
  attribute without creating a redundant custom attribute, while retaining the original GDTF
  identity and channel metadata in the fixture revision.
- An unmapped imported attribute is preserved and requires an explicitly typed, grouped, and placed
  custom attribute before it joins normal encoder pages. Source mappings round-trip through fixture
  revisions and show snapshots without exporting the desk's unrelated import preferences.
- Old shows either migrate deterministically or stop with an actionable conflict; no value is
  silently dropped, inverted twice, or reassigned by label.

## Deferred attribute reminder

When Media Effects are implemented, decide
their attribute group and typed parameter navigation from the real effect model. Do not reserve
Media Effect 1–4 encoder slots now.

## Configurable control-screen contract

This section carries the former Configurable Encoder and Control Screens plan into this canonical
plan. It extends the completed external fixed-pane work without weakening its view-only behavior or
changing existing saved desk data.

### Product decisions

1. The first production choices are exactly **4 encoders** and **6 encoders**. Five remains a useful
   packing-test width but is not selectable in this scope.
2. Encoder count is screen presentation configuration. It does not change fixture capability,
   canonical attributes, Programmer/Cue data, or show portability.
3. The complete interactive lower control surface has exactly one configured on-screen owner per
   desk: the main screen or one optional screen.
4. Moving it moves the command line, keypad, Programmer buttons, encoders, encoder-page controls,
   and lower-section modes as one unit. The first implementation does not split those controls
   across screens.
5. Native windows and authenticated browser pages render the same production components and share
   one server-authoritative desk session. There is no browser-only command or encoder clone.
6. A browser control surface is not the sibling Hardware Controls application. Attached hardware
   continues to use its own reported profile and the established OSC/typed-control contract.

### Screens & playback settings

Extend **Show > Desk Setup > Screens & playback** with one clearly labelled
**Programmer control surface** section:

- **Show controls on:** `Main screen` or one named optional screen, identified by stable screen
  configuration rather than a transient connection or operating-system display index.
- **Visible encoders:** `4` or `6`, stored with the screen that owns the control surface. Existing
  desks migrate to six encoders on the main screen.
- Every non-owner omits the lower control surface and has no invisible interaction targets in its
  place.
- Removing the owner requires explicit reassignment in the same confirmed action.
- A disconnected optional owner produces a visible
  **Programmer controls unavailable — assigned to _Screen name_** state and an explicit
  **Use controls on this screen** action. It must not silently create a second owner.
- The settings must visually preview or otherwise make clear how the chosen encoder count uses the
  semantic layout approved in the Attributes settings review.

Optional-screen base content can be **Desktop**, **Control surface only**, or
**Fixed pane only**. Selecting a screen as the control owner normally selects Control surface only,
but the settings remain separate and incompatible combinations are explained rather than silently
rewritten. Playbacks and Page Controls appear once; a control-only surface must not append a second
playback section beneath the production lower controls.

### Control-surface rendering

A control-only surface contains the shared command line and feedback, keypad and command buttons,
Programmer task/family navigation, active encoder group and derived pages, labels, values, indexed
choices, direct input, push/turn/push-turn/release behavior, lower-section modes, and the connection,
lock, loading, error, and ownership feedback required to operate it safely.

It contains no upper Desktop, pane tabs, Dock, setup navigation, hidden pane targets, or separate
local Programmer, selection, command buffer, undo history, Playback state, or page registry. Touch,
mouse, wheel, keyboard-focus, OSC, and attached-hardware semantics remain authoritative. A browser
surface must not introduce polling, per-client value authority, automatic action retries, or a
second command processor.

### Browser-accessible secondary surface

The server provides a stable authenticated URL for every configured optional screen. Screens &
playback offers **Open in browser** and **Copy browser link**; the URL is not a bearer secret.
Unknown, removed, or unauthorized screen identities show a clear non-interactive state.

Several browser tabs may observe a screen, but only the configured owner renders writable lower
controls. Opening, closing, or reconnecting a browser must not create or replace the desk session.
The native optional-window and browser routes resolve through one `ScreenSurface` composition path;
Tauri alone owns physical-display placement, native bounds, and operating-system fullscreen.

### Fixed pane beside base content

Model a screen as a composition:

- **Base content:** Desktop, control surface, or none.
- **Fixed pane:** none, full, left, or right, using the existing typed and view-only allowlist.
- **Side width:** a bounded saved percentage whose exact range is validated during the settings
  layout review.

`full` preserves today's fixed full-screen behavior. `left` and `right` create a real layout region,
not an overlay, and must not cover lower controls, Playbacks, Page Controls, drag regions,
connection state, or Desk Lock. Invalid minimum-size combinations are rejected visibly. Dock is
unavailable with a full fixed pane and belongs only to a Desktop base when a side pane is used.

### Encoder layout and page identity

The registry owns one semantic order from which every width is derived. Paired or compound
mechanisms stay together; a pair that does not fit moves to the next page; applicability filtering
may omit wholly empty pages; and custom attributes use the same packing contract. Do not persist
independent four- and six-encoder tables.

Shared navigation identifies the active encoder group plus a stable logical page/attribute anchor,
not only a numeric page index. Each surface derives the page containing that anchor for its width.
Application-specific decks such as Dynamics and future Timecode controls follow the same width
contract and may not assume six DOM slots.

### Persistence and compatibility

Screen composition and visible encoder count are desk-local. Preserve stable screen IDs, client
registrations, OSC aliases, display selection, bounds, fullscreen, playback layouts, page modes,
Desktop layouts, and portable show data.

Migrate current desk data as follows:

- Desktop content becomes `base = desktop`, `fixed = none`;
- Fixed full-screen pane content becomes `base = none`, `fixed = full` with unchanged pane settings;
- existing desks gain `visible_encoders = 6`; and
- the Programmer control surface remains on the main screen.

Malformed or legacy data normalizes to one valid owner and one supported encoder count. Management
routes remain typed, sparse, replay-safe object-intent operations; live control actions remain on
the established non-retried authoritative event path.

## Consolidated implementation order

1. Complete the manufacturer evidence review and settle every open canonical-attribute decision.
2. Complete the operator's remaining geometry review for Attributes & encoders and Screens &
   playback within the settled Desk Settings hierarchy above.
3. Rework Desk Settings navigation into Network & Inputs peer groups plus the Preferences child
   pages; move existing settings without changing their authority or persistence.
4. Add explicit many-representation mappings and calibrated color transforms.
5. Consolidate canonical identities with complete show compatibility and conflict handling.
6. Replace fixed page/slot settings with the approved semantic order and compound-control editor.
7. Generate and verify four-, five-, and six-position layouts; expose only four and six as screen
   choices in this scope.
8. Remap and verify every shipped fixture package through the fixture workflow.
9. Add desk-local screen composition, one-owner enforcement, encoder-count migration, and malformed
   data normalization.
10. Rework `ScreenSurface` into shared base-content, fixed-pane, and lower-control regions; add the
   authenticated browser surface and native-window adapter.
11. Update help and focused scenarios, then verify the exact software, browser, native-window,
    attached-hardware, OSC, persistence, migration, and real desktop paths.

## Additional acceptance criteria for settings and screens

1. The operator-approved Attributes settings layout makes semantic order, pages, positions,
   compound controls, activation groups, custom attributes, conflicts, and both supported screen
   widths understandable without reading raw stable IDs as the primary interface.
2. A new or migrated desk starts with six visible encoders and the Programmer controls on the main
   screen.
3. Screens & playback offers exactly four and six production encoder choices.
4. Four-encoder mode renders exactly four usable positions and deterministically repaginates every
   ordinary and application-specific group without losing a control.
5. Six-encoder mode preserves current behavior except where this plan deliberately changes the
   canonical grouping.
6. The complete lower control surface can move main → named optional screen → main with exactly one
   writable on-screen owner.
7. A control-only screen contains the real lower controls and no Desktop, upper panes, Dock, setup
   navigation, or duplicate Playback section.
8. The same saved surface works in a native Tauri window and authenticated browser through the
   same state and components.
9. Removed, unknown, unauthorized, closed, or reconnecting screens never reset desk state or expose
   an unintended interactive surface.
10. A disconnected owner is visibly reported and explicitly recoverable without silent dual
    ownership.
11. Existing fixed full-screen panes migrate unchanged; allowed fixed panes can also occupy a
    bounded left or right region without overlaying interactive regions.
12. Dock, Playbacks, Page Controls, fullscreen, minimum-size, and fixed-pane constraints remain
    valid for every supported composition.
13. Existing desk data migrates without modifying portable shows, screen identities, OSC aliases,
    playback layouts, or display assignments.
14. Attached hardware and OSC retain parity; changing visible software width never reclassifies
    physical hardware.
15. The page and sidebar label are **Network & Inputs**, with peer groups ordered as ToskLight
    server connection, Control inputs, Sound input, the future MQTT position, and Matter bridge.
16. Control inputs contain current MIDI/OSC/RTP-MIDI controls without Sound-to-Light audio;
    removing built-in MIDI and RTP-MIDI later leaves OSC without another navigation redesign.
17. Sound input owns audio-device selection and microphone access, and Matter bridge is one group
    rather than a Matter playback card nested under Services.
18. Preferences visibly contains Defaults, Attributes & encoders, Highlight, and Others as sidebar
    child pages with the exact contents and tab order specified above.
19. Encoder groups, Attribute activation groups, and Custom attributes edit one authoritative
    configuration while presenting distinct, understandable workflows; Custom attributes can map
    a stable GDTF/source attribute to an existing built-in or custom target before creating another
    attribute.
20. Highlight patch offers exactly **Stage only** and **Stage and DMX** beside Highlight look.
21. Users & sessions is absent from Desk Settings without deleting backend users, session
    isolation, or persisted current-user authority.

## Additional verification for settings and screens

- Component tests cover Network & Inputs group labels/order, Preferences navigation and tab
  contents, removal of Users & sessions, the approved Attributes editor, Screens & playback labels
  and previews, incompatible combinations, browser links, and disconnected-owner recovery.
- Encoder and fixture-import tests assert exact four/six visible positions, stable semantic
  navigation, pair/compound packing, applicability filtering, source-to-existing mappings,
  unmapped custom-attribute creation/placement, retained source metadata, and Dynamics pages.
- Rust and API tests cover legacy migration, one-owner enforcement, deleted-screen reassignment,
  invalid counts, typed sparse updates, and desk/session scope.
- `ScreenApp` tests cover Desktop, control-only, fixed-full, fixed-left, and fixed-right composition,
  including Dock/Playback/Page Controls geometry.
- Focused Playwright coverage opens the real browser route and operates keypad, buttons, and
  touch/turn/set-value encoder paths while verifying shared command-line and Programmer state.
- Native-window coverage exercises the same saved screen and representative side-pane layouts at
  minimum and ordinary screen sizes.
- Final proof uses `npm run open`, readiness and runtime logs, followed by an operator pass through
  the approved settings layout and a main → secondary → main control move.

## Consolidated out of scope

- Selecting five visible encoders in production.
- Replacing the Hardware Controls application or changing its OSC protocol.
- Independent Programmers for different browser tabs on one desk.
- Making the fixed-pane allowlist interactive.
- Browser control of physical monitor placement, native bounds, or operating-system fullscreen.
