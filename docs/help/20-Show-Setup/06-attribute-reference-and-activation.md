# Attribute Reference and Activation Examples

ToskLight identifies fixture capabilities with stable attribute IDs. Fixture profiles
retain their authored fixture-facing identity and explicitly map recordable channels
into the desk's canonical attribute registry.
Programmer values, Presets, Cues, API/OSC feedback, and Stage visualization use the
canonical ID instead of relying on a manufacturer's channel number or naming scheme.

This reference distinguishes two states:

- **Built in**: present in the current authoritative attribute registry.
- **Custom**: show-owned metadata supplies the label, type, units, encoder position, lifecycle, and activation group for a stable namespaced ID.

The exact raw DMX ranges always come from the fixture profile. Names such as Shutter Open or Strobe Fast describe semantic functions; they do not mean that `0` or `255` is universally safe.

## Built-in attributes

The table below introduces the long-standing core IDs; the default placement table later in this
chapter is the complete encoder and activation reference.

| Stable ID | Operator label | Attribute group | Value type | Default unit | Notes |
|---|---|---|---|---|---|
| `intensity` | Intensity | Intensity | Continuous | Percent | Primary fixture or head intensity. |
| `color` | Color | Color | Abstract color | — | Portable XYZ color request resolved through the fixture's color system. |
| `color.red` | Red | Color | Continuous | Percent | Additive red emitter. |
| `color.green` | Green | Color | Continuous | Percent | Additive green emitter. |
| `color.blue` | Blue | Color | Continuous | Percent | Additive blue emitter. |
| `color.cyan` | Cyan | Color | Continuous | Percent | Normally subtractive cyan filtration. |
| `color.magenta` | Magenta | Color | Continuous | Percent | Normally subtractive magenta filtration. |
| `color.yellow` | Yellow | Color | Continuous | Percent | Normally subtractive yellow filtration. |
| `color.amber` | Amber | Color | Continuous | Percent | Additive amber emitter. |
| `color.white` | White | Color | Continuous | Percent | Additive white emitter. |
| `color.uv` | UV | Color | Continuous | Percent | Ultraviolet/non-visible emitter; not added to abstract white unless explicitly programmed. |
| `color.wheel.1` | Color Wheel 1 | Color | Indexed | — | Profile-defined semantic slots and split colors. |
| `color.wheel.2` | Color Wheel 2 | Color | Indexed | — | Second profile-defined wheel. |
| `pan` | Pan | Position | Continuous | Degrees | Uses the profile's physical Pan range. |
| `tilt` | Tilt | Position | Continuous | Degrees | Uses the profile's physical Tilt range. |
| `focus` | Focus | Focus | Continuous | Percent | Near/far direction comes from the profile's authored physical range. |
| `zoom` | Zoom | Focus | Continuous | Degrees | Beam angle or zoom range. |
| `softness` | Softness | Focus | Continuous | Percent | Primary optical diffusion, frost, or beam-edge softening chosen by the fixture profile. |
| `iris` | Iris | Shapers | Continuous | Percent | Aperture and the first member of the default Shapers activation group. |
| `gobo.1` | Gobo 1 | Beam | Indexed | — | First indexed gobo wheel or selector. |
| `gobo.2` | Gobo 2 | Beam | Indexed | — | Second indexed gobo wheel or selector. |
| `shutter` | Shutter | Intensity | Indexed | — | Profile-defined open, closed, strobe, and related functions. |
| `strobe` | Strobe | Intensity | Continuous | Hz | Continuous strobe rate when the fixture exposes it separately. |
| `control` | Control | Control | Control | — | Generic typed control/function channel. |

## Fixture-facing attributes already seen in shipped fixtures

The shipped fixture packages currently use these additional physical/profile IDs. A profile maps
the generic concepts among them into the canonical registry described below; manufacturer-specific
concepts remain preserved fixture-facing IDs or show-owned custom descriptors.

| Area | Existing fixture-facing IDs |
|---|---|
| Extended color | `color.cold_white`, `color.warm_white`, `color.hue`, `color.saturation`, `color.temperature`, `color.indigo`, `color.lime` |
| Focus and atmosphere | `frost`, `fan`, `fog`, `switch` |
| Gobos | `gobo.fixed_gobo_wheel`, `gobo.gobo_index_rotation`, `gobo.gobo_time`, `gobo.gobo_wheel`, `gobo.rotating_gobo_index_rotation`, `gobo.rotating_gobo_selection`, `gobo.rotating_gobo_selection_speed`, `gobo.rotating_gobo_wheel` |
| Prisms | `prism.prism`, `prism.prism_insertion`, `prism.prism_rotation` |
| Fixture timing and movement | `fixture.auto_speed`, `fixture.blackout_move`, `fixture.mspeed`, `fixture.pan_tilt_speed`, `fixture.pan_tilt_speed_time`, `fixture.pan_tilt_time` |
| Fixture beam effects | `fixture.beam_duration`, `fixture.beam_fx_movement`, `fixture.beam_fx_select`, `fixture.beam_rate`, `fixture.beam_time` |
| Fixture framing and barndoors | `fixture.barndoor_macro_speed`, `fixture.barndoor_macros`, `fixture.barndoor_module_rotation`, `fixture.blade_1`, `fixture.blade_2`, `fixture.blade_3`, `fixture.blade_4`, `fixture.framing_macro`, `fixture.framing_macro_speed`, `fixture.framing_module_rotation` |
| Fixture effects | `fixture.effect_animations`, `fixture.effect_wheel_position`, `fixture.effect_wheel_rotation`, `fixture.effects_movement`, `fixture.effects_speed`, `fixture.fx_crossfade` |
| Fixture color/control | `fixture.colour_macros`, `fixture.colour_mix_control`, `fixture.control`, `fixture.fan_control`, `fixture.function`, `fixture.lamp_control`, `fixture.plus_7_control`, `fixture.power_special_functions`, `fixture.programs`, `fixture.reset`, `fixture.special_control`, `fixture.tint` |
| Fixture plate/pixel effects | `fixture.plate_background_master`, `fixture.plate_flash_duration`, `fixture.plate_flash_rate`, `fixture.plate_fx_movement`, `fixture.plate_fx_select`, `fixture.plate_pixel_master` |
| Package placeholders | `fixture.unused_4`, `fixture.unused_7`, `fixture.unused_8` |

This list describes the current shipped library, not a recommendation to standardize every
`fixture.*` name. Generic Frost, Prism, Pan/Tilt Speed, and Shaper blade concepts map to their
canonical descriptors. Manufacturer-specific macros, reserved slots, and unusual effects remain
typed fixture functions or custom attributes.
The retired generic `beam.effect.1` and `beam.effect.2` placeholders are still readable in older
shows but are not offered for new programming: a source Beam FX channel must map to its actual
mechanism or retain an explicit custom attribute.

## Default attribute vocabulary and encoder placement

The registry separates the desk's canonical mapping layer from names carried by
fixture packages. A package keeps the fixture attribute identity that describes its
physical channel; the mapping layer presents a coherent programmer attribute. The
preferred encoder location uses `P<page>/E<encoder>` across the desk's six encoders.

These are stable preferred slots, not a promise that every page is always visible.
Pages with no supported attribute in the current selection are omitted, while an empty
slot on a visible page remains empty so later controls do not move. Pressing an already
active attribute-group button cycles **Group 1 of N**, **Group 2 of N**, and so on.

| Default desk group | Preferred encoder | Canonical mapping-layer attribute | Fixture-package attribute name and ID | Suggested activation group |
|---|---:|---|---|---|
| Intensity | P1/E1 | `intensity` | **Intensity** (`intensity`) | — |
| Intensity | P1/E1 | `intensity` | **Fog Output** (`fog`) | — |
| Intensity | P1/E2 | `shutter` | **Shutter** (`shutter`) | — |
| Intensity | P1/E3 | `strobe` | **Strobe Rate** (`strobe`) | — |
| Intensity | P1/E4 | `volume` | **Volume** (`media.volume`) | — |
| Intensity | P1/E4 | `volume` | **Fan** (`fan`) when it is a recordable level rather than a typed action | — |
| Color | P1/E1 | `color.red` | **Red** (`color.red`) | Color Mix |
| Color | P1/E1 | `color.red` | **Cyan filtration** (`color.cyan`), inverted | Color Mix |
| Color | P1/E2 | `color.green` | **Green** (`color.green`) | Color Mix |
| Color | P1/E2 | `color.green` | **Magenta filtration** (`color.magenta`), inverted | Color Mix |
| Color | P1/E3 | `color.blue` | **Blue** (`color.blue`) | Color Mix |
| Color | P1/E3 | `color.blue` | **Yellow filtration** (`color.yellow`), inverted | Color Mix |
| Color | P1/E4 | `color.white` | **White** (`color.white`) | Color Mix |
| Color | P1/E5 | `color.amber` | **Amber** (`color.amber`) | Color Mix |
| Color | P1/E6 | `color.uv` | **UV** (`color.uv`) | Color Mix |
| Color | P1/E4 | `color.white` | **Cold White** (`color.cold_white`), identity-mapped while retaining its physical channel name | Color Mix |
| Color | P1/E5 | `color.amber` | **Warm White** (`color.warm_white`), identity-mapped while retaining its physical channel name | Color Mix |
| Color | P2/E3 | `color.lime` | **Lime** (`color.lime`) | Color Mix |
| Color | P2/E4 | `color.indigo` | **Indigo** (`color.indigo`) | Color Mix |
| Color | P2/E5 | `color.mint` | **Mint** (`color.mint`) | Color Mix |
| Color | P2/E6 | `color.temperature` | **Color Temperature / CTO** (`color.temperature`) | Color Mix |
| Color | P3/E2 | canonical RGB color state | **Hue + Saturation** (`color.hue` + `color.saturation`), converted as one paired color system | Color Mix |
| Color | P3/E3 | `color.wheel.1` | **Color Wheel 1** (`color.wheel.1`) | Color Wheel |
| Color | P3/E4 | `color.wheel.1.rotation` | **Color Wheel 1 Rotation** (`color.wheel.1.rotation`) | Color Wheel |
| Color | P3/E5 | `color.wheel.2` | **Color Wheel 2** (`color.wheel.2`) | Color Wheel |
| Color | P3/E6 | `color.wheel.2.rotation` | **Color Wheel 2 Rotation** (`color.wheel.2.rotation`) | Color Wheel |
| Position | P1/E1 | `pan` | **Pan** (`pan`) | Position |
| Position | P1/E2 | `tilt` | **Tilt** (`tilt`) | Position |
| Position | P1/E3 | `pan.continuous` | **Continuous Pan** (`pan.continuous`) | — |
| Position | P1/E4 | `tilt.continuous` | **Continuous Tilt** (`tilt.continuous`) | — |
| Position | P1/E5 | `pan.time` or shared `position.time` | **Pan Time** (`pan.time`) or **Pan/Tilt Time** (`position.time`, migrated from `fixture.pan_tilt_time`) | — |
| Position | P1/E6 | `tilt.time` | **Tilt Time** (`tilt.time`); empty when a shared Pan/Tilt Time occupies E5 | — |
| Position | P2/E1 | `position.speed` | **Pan/Tilt Speed** (`position.speed`, migrated from `fixture.pan_tilt_speed`) | — |
| Position | P2/E2 | `position.mode` | **Position Mode** (`position.mode`) | — |
| Beam | P1/E1 | `gobo.1` | **Gobo 1** (`gobo.1` and migrated first-wheel gobo IDs) | — |
| Beam | P1/E2 | `gobo.1.rotation` | **Gobo 1 Rotation** (`gobo.1.rotation`) | — |
| Beam | P1/E3 | `gobo.2` | **Gobo 2** (`gobo.2` and migrated second-wheel gobo IDs) | — |
| Beam | P1/E4 | `gobo.2.rotation` | **Gobo 2 Rotation** (`gobo.2.rotation`) | — |
| Beam | P1/E5 | `prism.1` | **Prism 1** (`prism.1`, migrated from `prism.prism` or `prism.prism_insertion`) | — |
| Beam | P1/E6 | `prism.1.rotation` | **Prism 1 Rotation** (`prism.1.rotation`, migrated from `prism.prism_rotation`) | — |
| Beam | P2/E1 | `prism.2` | **Prism 2** (`prism.2`) | — |
| Beam | P2/E2 | `prism.2.rotation` | **Prism 2 Rotation** (`prism.2.rotation`) | — |
| Beam | P2/E3 | `animation.1` | **Animation Wheel 1** (`animation.1`) | — |
| Beam | P2/E4 | `animation.1.rotation` | **Animation Rotation 1** (`animation.1.rotation`) | — |
| Shapers | P1/E1 | `iris` | **Iris** (`iris`) | Shapers |
| Shapers | P1/E2 | `shaper.blade.1.position` | **Blade 1 Position** (`shaper.blade.1.position`, migrated from `fixture.blade_1`) | Shapers |
| Shapers | P1/E3 | `shaper.blade.1.angle` | **Blade 1 Angle** (`shaper.blade.1.angle`) | Shapers |
| Shapers | P1/E4 | `shaper.blade.2.position` | **Blade 2 Position** (`shaper.blade.2.position`, migrated from `fixture.blade_2`) | Shapers |
| Shapers | P1/E5 | `shaper.blade.2.angle` | **Blade 2 Angle** (`shaper.blade.2.angle`) | Shapers |
| Shapers | P1/E6 | `shaper.rotation` | **Framing/Barn-door Module Rotation** (`shaper.rotation`, migrated from `fixture.framing_module_rotation` or `fixture.barndoor_module_rotation`) | Shapers |
| Shapers | P2/E1 | `shaper.blade.3.position` | **Blade 3 Position** (`shaper.blade.3.position`, migrated from `fixture.blade_3`) | Shapers |
| Shapers | P2/E2 | `shaper.blade.3.angle` | **Blade 3 Angle** (`shaper.blade.3.angle`) | Shapers |
| Shapers | P2/E3 | `shaper.blade.4.position` | **Blade 4 Position** (`shaper.blade.4.position`, migrated from `fixture.blade_4`) | Shapers |
| Shapers | P2/E4 | `shaper.blade.4.angle` | **Blade 4 Angle** (`shaper.blade.4.angle`) | Shapers |
| Shapers | P2/E5 | `shaper.keystone.x` | **Keystone X** (`shaper.keystone.x`) | Shapers |
| Shapers | P2/E6 | `shaper.keystone.y` | **Keystone Y** (`shaper.keystone.y`) | Shapers |
| Focus | P1/E1 | `focus` | **Focus** (`focus`) | — |
| Focus | P1/E2 | `zoom` | **Zoom** (`zoom`) | — |
| Focus | P1/E3 | `softness` | The profile's primary **Frost** (`frost` or `frost.1`) or **Beam Edge** (`beam.edge`) mechanism | — |
| Control | P1/E1 | `control.mode` | **Fixture Mode** (`control.mode`) | — |
| Control | P1/E2 | `control.speed` | **Fixture Control Speed** (`control.speed`) | — |
| Control | P1/E3 | typed control action | **Fan Auto/Low/High/Max** (`fixture.fan_control`) | Not recordable |
| Control | P1/E4 | typed control action | **Lamp On/Off** (`fixture.lamp_control`) | Not recordable |
| Control | P1/E5 | typed control action | **Reset** (`fixture.reset`) | Not recordable |
| Media | P1/E1 | `media.folder` | **Media Folder** (`media.folder`) | Media Source |
| Media | P1/E2 | `media.file` | **Media File** (`media.file`) | Media Source |
| Media | P1/E3 | `media.mask.folder` | **Mask Folder** (`media.mask.folder`) | Mask Source |
| Media | P1/E4 | `media.mask.file` | **Mask File** (`media.mask.file`) | Mask Source |
| Media | P1/E5 | `media.opacity` | **Layer Opacity** (`media.opacity`) | — |
| Media | P1/E6 | `media.tint` | **Layer Tint** (`media.tint`) | — |
| Media | P2/E1 | `media.play_mode` | **Play Mode** (`media.play_mode`) | — |
| Media | P2/E2 | `media.playback_speed` | **Playback Speed** (`media.playback_speed`) | — |
| Media | P2/E3 | `media.playback_bpm` | **Playback BPM** (`media.playback_bpm`) | — |
| Media | P2/E4 | `media.grayscale` | **Grayscale** (`media.grayscale`) | — |
| Media | P2/E5 | `media.scaling_mode` | **Scaling Mode** (`media.scaling_mode`) | — |
| Media | P2/E6 | `media.rotation` | **Layer Rotation** (`media.rotation`) | — |
| Media | P3/E1 | `media.position.x` | **Position X** (`media.position.x`) | — |
| Media | P3/E2 | `media.position.y` | **Position Y** (`media.position.y`) | — |
| Media | P3/E3 | `media.scale.x` | **Scale X** (`media.scale.x`) | — |
| Media | P3/E4 | `media.scale.y` | **Scale Y** (`media.scale.y`) | — |
| Media | P3/E5 | `media.mask.opacity` | **Mask Opacity** (`media.mask.opacity`) | — |
| Media | P3/E6 | `media.mask.invert` | **Invert Mask** (`media.mask.invert`) | — |
| Media | P4/E1–E4 | `media.effect.1` … `.4` | **Media/Plate Effect 1–4** (`media.effect.1` … `.4`; selected migrated `fixture.plate_fx_…` channels) | — |
| Media | Following pages | `media.effect.<n>.parameter.<n>` | Advertised typed effect parameters | — |
| Control | Direct action, not an encoder value | typed control action | **Reset Layer** (`media.reset`) | Not recordable |

`intensity` is the canonical output quantity for a light-emitting head and for a fog
head: a fog-only fixture's Fog channel therefore behaves like that head's intensity.
`volume` is the canonical audible/airflow quantity, so a media Volume channel and a
recordable continuous Fan channel use the same programmer meaning. A profile must not
map two independently controllable physical channels on the same logical head to one
canonical attribute; it must use the narrower canonical descriptor, another logical
head, or a preserved custom attribute instead.

CMY inversion belongs to the fixture color-system mapping, not to the encoder or Cue.
Canonical Red, Green, and Blue always increase toward more emitted red, green, and blue.
The profile retains the physical Cyan, Magenta, and Yellow channel identities and
authored raw ranges, while programmer state, Presets, Cues, feedback, Highlight, and
Stage use the canonical RGB result.

Cold White and Warm White likewise remain the fixture-facing identities of their physical
channels, but they program canonical White and Amber respectively. Existing show values and Desk
Setup placements migrate to those canonical controls. A fixture profile cannot map a separate
White plus Cold White channel, or a separate Amber plus Warm White channel, on the same logical
head and split: that collision must be resolved by a narrower custom descriptor or another head.

Lamp On, Lamp Off, Reset, Fan Auto, Fan Low, Fan High, and Fan Max already exist as typed control-action meanings. They are actions, not ordinary recordable continuous attributes.

## Semantic special values

A channel function binds one of these meanings to the manufacturer's exact DMX range. Slow/Fast describes direction across the authored range; the fixture profile must record when a manufacturer reverses it.

| Attribute | Useful semantic functions |
|---|---|
| Shutter | **Closed**, **Open**, **Strobe Slow → Fast**, **Strobe Fast → Slow**, **Random Strobe Slow → Fast**, **Random Strobe Fast → Slow**, **Pulse Open**, **Pulse Close**. Highlight always requests Open where available. |
| Color wheel | **Open/White/Clear**, named or measured color slots, split-color slots, **Continuous Scroll CW/CCW Slow → Fast**, and **Random Color** where documented. |
| Gobo | **Open**, named/indexed gobo slot, split slot where meaningful, **Shake Slow → Fast**, indexed angle, and **Rotate CW/CCW Slow → Fast** on an applicable rotation channel. |
| Prism | **Open/No Prism**, named/indexed prism, indexed angle, and **Rotate CW/CCW Slow → Fast**. |
| Animation wheel | **Open/Out**, indexed position, and **Rotate CW/CCW Slow → Fast**. |
| Iris | **Open**, **Closed**, continuous aperture, **Pulse Open**, and **Pulse Close**. |
| Frost | **Open/No Frost**, named Frost flag, and continuous frost amount. |
| Focus / Zoom | Continuous authored physical range; Near/Far and Narrow/Wide labels follow the actual profile direction rather than a universal raw endpoint. |
| Fan | **Auto**, **Off** when documented safe, **Low**, **High**, **Max**, or continuous rate. |
| Lamp / Reset | **Lamp On**, **Lamp Off**, and **Reset** as momentary or timed typed actions; never inferred from an arbitrary Control range. |
| Media source | **Blank/No Source** plus indexed folder/file identities. Folder and file are committed together in touch browsing. |
| Media play mode | **Loop**, **Once**, **Reverse**, **Bounce**, **Synchronized**, **Pause**, **Blackout**, and **Reset** only when the connected server advertises them. |
| Media mask | **No Mask**, mask source, **Normal**, and **Inverted** with the connected server's advertised alpha/luminance behavior. |

Function semantics and attribute identity are related but distinct. A single physical Shutter channel may contain Closed, Open, and several Strobe functions; a fixture with a separate Strobe channel may instead expose `strobe` as a continuous attribute.

## Custom attributes

Imported fixture profiles and show data may contain IDs outside the built-in registry. ToskLight
preserves those strings losslessly. **Show → Desk Setup → Programmer → Attributes** creates and
edits show-owned metadata without changing the stable ID: label, value type, units, cyclic or
bounded behavior, recording eligibility, encoder group/page/slot, lifecycle, and exactly one
activation group. Retiring a descriptor removes it from new authoring while old fixture, Programmer,
Preset, Cue, and revision data remains resolvable.

Use a namespaced ID such as `vendor.feature` or `custom.feature` and avoid reusing a built-in ID for
a different meaning. Manufacturer-specific control ranges should remain fixture functions or custom
attributes rather than being forced into an unrelated built-in. **Restore recommended defaults**
restores the server-projected built-in activation groups while retaining custom-only groups and
giving any formerly mixed custom member a safe single-member group.

When a newer ToskLight version adds built-in attributes, an older saved configuration gains those
new definitions at load time without rewriting the show merely because it was opened. Existing
custom descriptors and activation choices remain intact. If a custom descriptor used a slot that
has since become a built-in's preferred position, the custom descriptor moves to the first free
custom page in the same encoder group.

Schema-v2 fixture snapshots keep their original fixture-facing IDs while unambiguous legacy names
project to the canonical IDs in the table above. Existing Programmer, Preset, Cue, and revision
data remains byte-for-byte portable: the renderer accepts an old fixture-facing value as a fallback
until a canonical value for that channel exists. For CMY, that fallback retains the old physical
filtration amount, while new Red, Green, and Blue values use the inverted canonical mapping. An
old Cold White or Warm White value keeps the same normalized amount as canonical White or Amber.
An ambiguous legacy name stays an identity mapping instead of being guessed.

## Recommended activation groups

Activation groups decide which supported attributes become active together when one member changes. They do not force every listed attribute onto every fixture; missing members are skipped per fixture and logical head.

| Example group | Suggested members | Reason |
|---|---|---|
| Position | `pan`, `tilt` | A Cue that moves one axis usually needs a complete position. |
| Color Mix | `color`, canonical Red, Green, Blue, Amber, White, UV, Lime, Indigo, Mint, and Color Temperature | Captures the supported mixed-color and white-point state. Physical CMY channels participate through their inverted RGB mappings; physical Cold White and Warm White channels participate through White and Amber. |
| Color Wheel | Color Wheel 1/2 and their applicable rotation attributes | Keeps authored wheel selection/rotation coherent without activating the fixture's mixed-color channels. |
| Media source | `media.folder`, `media.file` | Prevents a new folder from being stored with an absent or unrelated file. |
| Media mask source | `media.mask.folder`, `media.mask.file` | Keeps a mask address coherent for the same reason. |
| Shapers | Iris, all supported blade Positions and Angles, and Framing/Barn-door Module Rotation | Captures the complete physical beam-shaping state, including fixtures that expose one combined value per blade. |

Intensity is normally a single-member group. Shutter and Strobe should not automatically activate together merely because they may share a physical channel.
Color Mix and Color Wheel are deliberately separate recommended groups. A production can
reconfigure them, but changing Red should not select or capture a wheel slot by default.
Tint remains an independently recordable green–magenta adjustment, but it is edited in the Color
special dialog only when the selected fixture or logical head supports it. It does not reserve a
permanent encoder slot or activate merely because another Color Mix member changes.
Focus, Zoom, Frost, Gobos, and Prisms remain independent recommended choices. Iris is the
exception here because it belongs to the default Shapers activation group.

Softness is the manufacturer-independent amount of optical diffusion or edge softening. A fixture
profile chooses one primary Frost or Beam Edge mechanism for this canonical control. If a fixture
has a second independently controllable frost, diffusion, or edge mechanism, that channel keeps a
separate custom attribute; it is never silently merged into Softness. Compatibility-only
`frost.2` remains readable in older shows but is not offered as a new built-in control.

When one member changes, linked values are captured once from the authoritative current Normal, Blind, or Preload context and then remain fixed in the Programmer. Changing the Desk Setup grouping affects future activations only and never rewrites recorded Cues.
