# Attribute Registry, Activation Groups, and Indexed Presets

## Status

**Specification only.** This plan records future desk attribute metadata, programmer activation behavior, and encoder **Set Value** Indexed Presets. It does not implement registry changes, custom attributes, programmer capture, Record defaults, Cue storage, modal UI, fixture-function merging, control actions, API behavior, OSC behavior, fixture migration, help generation, testing scenarios, or executable tests.

## Goal

Make attribute relationships explicit so a tracking desk can record complete coherent states without storing every resolved value.

Today, changing only Red can leave Green, Blue, UV, CTO, or a color wheel absent from the entire Cuelist. Tracking then has no value to apply for those missing attributes. Similar problems occur when Pan is stored without Tilt or a media Folder changes without its File.

Desk Setup gains configurable **Attribute activation groups**. When the operator changes one member, the desk activates the other applicable members together at their current values so a later Record stores a coherent group.

The same setup area also manages rich metadata for custom attributes, allowing new fixture capabilities to be programmed before ToskLight ships a built-in definition for them.

Encoder **Set Value** also presents a fixture's authored fixed, indexed, and control functions inside the attribute they belong to. These functions appear as **Indexed Presets** in the Set Value modal. They do not become separate programmer attributes, attribute groups, numbered show Presets, or Preset Pool objects.

## Canonical attribute registry

Existing canonical stable string attribute IDs remain the programmer, wire, and show
persistence identity. Fixture packages additionally retain their fixture-facing
attribute name/ID and bind it to a canonical mapping-layer attribute. Existing channels
whose fixture and canonical IDs are identical use an identity mapping. This separates a
manufacturer/profile concept such as Cyan filtration or a legacy Gobo name from the
canonical value programmed and recorded by the desk.

The registry expands its metadata to cover these operator groups:

- Intensity;
- Color;
- Position;
- Beam;
- Shapers;
- Focus;
- Control; and
- Media.

Each built-in or custom descriptor contains at least:

- stable ID;
- operator label;
- attribute group;
- value type;
- display and physical unit where applicable;
- cyclic, bounded, indexed, or control behavior;
- semantic special-value vocabulary where applicable; and
- whether it is eligible for activation groups and recording.

Fixture profiles continue to bind real channels and exact DMX ranges to fixture
attributes/functions, then explicitly map recordable attributes into the canonical
layer. The desk registry must not contain manufacturer-specific raw channel maps.

Continuous, fixed, indexed, and control describe function/value behavior inside an attribute; they are not additional attribute groups. Their encoder presentation belongs to the attribute's **Indexed Presets** workflow.

### Canonical color state

The programmer's canonical mixed-color state is additive RGB plus explicitly supported
additional emitters and white-point controls. A fixture profile may describe physical
CMY filtration, but Cyan, Magenta, and Yellow do not become an independent programmer
color model:

- fixture Cyan maps inversely to canonical Red;
- fixture Magenta maps inversely to canonical Green; and
- fixture Yellow maps inversely to canonical Blue.

Thus canonical `100% Red` means no Cyan filtration and canonical `0% Red` means full
Cyan filtration for a subtractive fixture, subject to the profile's authored range and
inversion. Color conversion, calibration, Highlight, output, Stage visualization,
Presets, Cues, and feedback must use this same mapping. Existing persisted
`color.cyan`, `color.magenta`, and `color.yellow` values require an explicit compatible
migration rather than silently changing meaning on load.

### Encoder groups and pages

Each descriptor also carries a default encoder group, canonical order, and preferred
page/slot within the desk's six encoder positions. This presentation metadata is not
part of fixture packages and does not change the stable programmer attribute identity.

Selecting an attribute group shows the first applicable page. Pressing the already
selected group again cycles its applicable pages and displays, for example,
**Color 1 of 2** and **Color 2 of 2**. Pages are projected from the union of supported
canonical attributes across the current selection. A mixed RGB and color-wheel
selection therefore exposes both the Color Mix and Color Wheel controls. Unsupported
slots remain visibly unassigned rather than causing later controls to change encoder
numbers; pages that contain no supported attribute at all are omitted.

The complete proposed mapping and preferred page/encoder positions live in
[Attribute Reference and Activation Examples](../../help/20-Show-Setup/06-attribute-reference-and-activation.md).
Custom descriptors choose their default group and page/order in Desk Setup and append
after built-in pages unless deliberately assigned to an available built-in slot.

## Desk-configured custom attributes

**Show > Desk Setup > Programmer > Attributes** can create, edit, and retire custom descriptors. A custom attribute uses a collision-resistant stable ID, not its editable label. It can participate in programmer controls, fixture-profile editing, custom activation groups, API/OSC feedback, and persisted Cue values like a built-in attribute.

Custom descriptor configuration is desk-local. Portable fixture profiles and shows retain unknown stable IDs losslessly on another desk. A desk that lacks the descriptor shows the stored ID with generic safe controls and a clear unknown-attribute state; it must not delete, rename, coerce, or remap the data.

Changing a label, unit, or presentation group does not rewrite shows. Changing an existing attribute's value type or semantic meaning is a breaking migration and requires usage inspection plus explicit confirmation. A descriptor referenced by a fixture profile, activation group, programmer value, Preset, Cue, or show revision cannot be destructively removed; it can be retired from new authoring while remaining resolvable.

Built-in IDs cannot be shadowed or redefined by a custom descriptor.

## Activation-group configuration

**Show > Desk Setup > Programmer > Record defaults > Attribute activation groups** lists all built-in and custom attributes by operator group. The operator can create named groups, add or remove members, restore recommended defaults, and see unsupported or retired members.

An attribute belongs to at most one activation group. This avoids hidden transitive or overlapping activation. Single-member groups are allowed but have no linking effect.

Recommended starting groups include:

- **Color mix**: abstract Color plus canonical RGB, additional emitters, white, UV,
  color-temperature, and tint attributes; physical CMY fixture channels participate
  through their inverted RGB mappings;
- **Color wheel**: all configured Color Wheel selectors and rotations;
- **Position**: Pan and Tilt;
- **Media source**: Media Folder and Media File;
- **Media mask source**: Mask Folder and Mask File;
- **Shapers**: Iris, all supported blade positions/angles, and the shared framing or
  barn-door module rotation.

These are editable record defaults, not hard-coded assumptions about every fixture.
Color Mix and Color Wheel remain separate defaults so changing an emitter does not
silently select or capture a wheel slot.

## Activation semantics

When the operator first changes a member for a fixture or logical head:

1. apply the requested value change;
2. find every other group member that the same fixture/head actually supports;
3. retain an already active programmer value for that member, otherwise capture its current authoritative value in the active programming context; and
4. add the requested change and linked captures as one programmer mutation and one Undo step.

“Current” means the resolved value in the operator's active context at that instant: normal live programming uses live resolved output, while Blind or Preload uses its documented context projection. Once captured, the linked value is frozen in the programmer and does not chase later playback changes.

Missing attributes are skipped. The desk does not synthesize fixture defaults, send values to fixtures that lack a member, or activate a different logical head. A multi-head selection resolves support independently per head.

Linked captures are intentional active programmer values. Record, Update where its selected capture policy permits new addresses, Preset storage, Preload storage, Clear, Undo, and programmer ownership must recognize them consistently. Merely selecting fixtures, recalling tracked playback state, or viewing a value does not trigger a group.

Control actions, lamp/reset commands, static channels, raw DMX overrides, and hazardous functions are excluded from activation groups unless a future explicit typed policy makes them safe.

## Set Value Indexed Presets

Opening **Set Value** from an encoder keeps one modal for the selected attribute. Its title bar contains two tabs:

- **Direct input**; and
- **Indexed Presets**.

**Direct input** retains the absolute-value fader, current value and unit, number block, direct numeric entry, and supported `THRU`/spread entry.

**Indexed Presets** shows the fixed, indexed, and control functions authored for that same attribute on the currently selected fixtures or logical heads. It uses a touch-sized list or grid and may group a long set by function kind or fixture scope, but it never moves the choices to a different attribute.

For example, Shutter Open, Shutter Closed, and authored Strobe functions belong under the Shutter encoder; named Gobo slots belong under the applicable Gobo encoder; and a fixture-specific indexed Frost choice belongs under Frost. The operator should not need to find an unrelated **Fixed**, **Indexed**, or **Control** encoder family.

When no selected fixture provides an applicable function, the tab remains understandable and shows an empty explanation rather than an unrelated global list.

## Function ownership

Fixed, indexed, continuous, and control are fixture-channel function behaviors. The function retains:

- its semantic attribute;
- stable function and semantic identity;
- operator-facing name;
- exact per-profile raw value or range;
- function priority;
- recordable or transient behavior; and
- control-action behavior where applicable.

The modal is a projection of each selected fixture's embedded profile revision. It must not infer raw DMX from the function name, use the current desk-library revision instead of the show's embedded revision, or compile manufacturer-specific choices into the UI.

## Combining choices across selected fixtures

ToskLight gathers applicable functions independently for every selected fixture or logical head.

Functions appear as one combined Indexed Preset when they:

- belong to the encoder's selected semantic attribute;
- have the same operator-facing name; and
- have compatible semantic meaning and action behavior.

When every selected fixture/head provides that compatible named function, the row is marked as applying to **All selected fixtures**. Clicking it resolves each fixture's own authored function and applies the corresponding values to every selected fixture/head in one authoritative programmer mutation and one Undo step. Different fixtures may use different channel layouts, raw values, ranges, resolutions, or function priorities.

The name is the operator-facing merge rule, but name equality must not merge incompatible meanings. Two functions both called **Reset** cannot be combined when one is a timed reset action and the other is an unrelated recordable fixed value. Stable semantic IDs and function/action types disambiguate such collisions.

When only a subset provides a compatible named function, the row identifies that exact fixture subset and must not claim to target all selected fixtures. When names differ, the choices remain separate and visibly scoped to their fixture types or fixture IDs; they are never flattened into one misleading row. Clicking a subset row affects only the fixtures named by that row.

If the selection or embedded fixture revision changes while the modal is open, the choice projection refreshes or the stale action is rejected. It must not silently apply a previously displayed function to a different selection.

## Applying fixed and indexed values

A recordable fixed or indexed choice becomes a normal value of the selected semantic attribute for every targeted fixture/head. One click:

1. resolves the authored function for each target;
2. activates that function through the normal programmer arbitration path;
3. applies every target atomically;
4. updates UI, Fixture Sheet, Stage, API/OSC feedback, and physical output; and
5. creates one Undo step.

The operation follows the first-priority [Programmer Relative Encoders, Touch Controls, and Fade-Time Scope](00-programmer-relative-encoders-and-fade-time-scope.md): because it originates from encoder **Set Value**, the live value and output change immediately regardless of Programmer Fade. It must not acquire a persistent `0s` per-value Cue timing override.

Selecting a recordable Indexed Preset is an attribute change for activation groups. Applicable linked attributes activate through the same authoritative grouped operation. A typed transient control action does not activate recordable attribute groups.

## Applying control functions

Control functions remain typed actions rather than being disguised as recordable scalar values.

- A latched recordable mode uses its authored persistent behavior.
- A momentary action uses the documented press/release behavior.
- A timed action executes for its authored duration.
- Multi-channel actions apply all participating channel values atomically.
- Lamp On, Lamp Off, Reset, and other transient or hazardous actions remain non-recordable where their fixture profile says so.

The Indexed Presets list identifies control actions and their scope. An action requiring confirmation, hold, timing, or safety feedback retains that interaction instead of becoming a one-click persistent Preset merely because it appears in the tab.

Applying a combined control row requires compatible action behavior on every target. Otherwise the functions remain separately scoped.

## Relationship to show Presets and Dynamics

Indexed Presets do not create, rename, number, store, or update regular show Presets. They are available because the selected fixtures' embedded profiles already define those functions.

The same touch-list presentation may later be reused for a separate regular-Preset chooser. That future surface must retain normal Preset identities, pool numbers, references, Update behavior, and portability. [Dynamics](../Later/dynamics/README.md) may use regular Presets as values or sources, but it must not reinterpret fixture Indexed Presets as numbered show Presets.

Reusable visual components are welcome; the data models and operator meanings remain separate.

## Interfaces and persistence

The registry and activation configuration are authoritative server/desk state. Software UI, command-line/API, OSC, attached hardware, fixture-profile editing, and Media touch transactions use the same grouped programmer operation rather than recreating group logic locally.

Activation configuration is desk-local and can vary between desks. The resulting programmer and Cue values use stable attribute IDs and remain portable show data. Changing the desk's groups affects future activations only; it never rewrites recorded Cues or current programmer contents.

## Compatibility

Existing built-in IDs retain their meaning. Existing unknown IDs remain custom rather than being collapsed to one shared `custom` identity. Migration seeds recommended activation groups only when the installation has no prior configuration and must not activate or rewrite any show value during startup.

Fixture-schema migration gives every existing channel an identity mapping unless a
specifically tested canonical migration applies. CMY and the named legacy aliases in
the attribute reference require value-preserving migrations of fixture snapshots,
programmer values, Presets, Cues, and revision history. If a legacy ID is ambiguous, it
remains a preserved custom attribute instead of being guessed. Export and import retain
both the fixture-facing ID and its canonical mapping.

The operator-facing registry and proposed vocabulary are documented in [Attribute Reference and Activation Examples](../../help/20-Show-Setup/06-attribute-reference-and-activation.md).

## Documentation and regression coverage

Implementation must update:

- the encoder **Set Value** description in operator help;
- the Fixture Library explanation of fixed, indexed, and control functions;
- the attribute reference's distinction between functions and attribute groups;
- applicable human-readable scenarios under `docs/testing`;
- the coverage catalog under `docs/help/99-Development/`; and
- focused fixture/programmer/frontend tests plus root Playwright coverage.

Indexed Presets tests must use at least two different fixture profiles whose same-named choice maps to different raw values, plus differently named and same-name-but-incompatible functions. Assertions must inspect the authoritative programmer state and resolved DMX output, not only the merged list text.

## Acceptance coverage

1. Desk Setup lists built-in and custom attributes under the eight operator groups with stable IDs and typed metadata.
2. Operators can create, edit, retire, and safely resolve custom descriptors without shadowing built-ins.
3. Unknown custom IDs round-trip through fixtures, shows, Cues, APIs, and exports without loss.
4. Changing Red in a configured Color group activates the current supported Green, Blue, UV, CTO, and other chosen members for each affected head.
5. Changing Pan can activate the current Tilt; changing Media Folder or File can activate the matching pair.
6. Linked values are captured from the correct live, Blind, or Preload context once and do not chase later output.
7. One initiating change and all linked captures form one authoritative mutation and one Undo step.
8. Missing attributes and other logical heads are skipped without synthesizing values.
9. Record, Update policy, Presets, Preload, Clear, Undo, API/OSC, and attached hardware agree on linked active values.
10. Control actions and hazardous functions cannot be linked accidentally.
11. Changing activation configuration affects future programmer operations only and does not rewrite shows.
12. Existing shows and unknown attributes migrate losslessly, and recommended defaults are seeded without creating programmer or Cue data.
13. CMY fixture channels map inversely to canonical RGB programmer values and round-trip
    consistently through output, feedback, Stage, Highlight, Presets, and Cues.
14. The six encoder positions use the documented stable page/slot order; repeated
    presses on the active attribute group cycle only applicable pages and identify the
    current page as `<Group> X of Y`.
15. A mixed selection exposes the union of supported attributes without shifting a
    canonical control to a different encoder, including RGB plus Color Wheel selections.
16. Fixture packages retain their authored fixture-facing attribute identity and an
    explicit canonical mapping; legacy identity mappings and migrated aliases round-trip
    without rewriting an ambiguous custom attribute.
17. Encoder **Set Value** has title-bar **Direct input** and **Indexed Presets** tabs for the selected attribute.
18. Direct input retains its absolute fader, number block, numeric entry, and supported spread behavior.
19. Fixed, indexed, and control functions appear under their semantic attribute and never form an attribute group of their own.
20. Same-named compatible functions across all selected fixtures appear once and apply each fixture's authored value atomically to all selected fixtures.
21. Different raw values, ranges, resolutions, and channel layouts resolve correctly behind one combined choice.
22. Differently named, partially supported, or incompatible functions remain separate and display their exact target scope.
23. Same-name collisions with incompatible semantic or control behavior never merge.
24. A stale selection or fixture revision cannot receive a choice projected for an earlier state.
25. Recordable fixed/indexed choices create one programmer mutation and one Undo step and follow attribute-activation groups.
26. Encoder-originated Indexed Presets change live output immediately regardless of Programmer Fade without recording an explicit `0s` Cue fade.
27. Momentary, timed, latched, multi-channel, hazardous, and non-recordable control actions retain their authored behavior.
28. Indexed Presets remain distinct from numbered show Presets and from the Dynamics use of regular Presets.
29. Help, testing Markdown, coverage catalogs, focused tests, and Playwright permanently cover merging, scoping, action safety, timing, and real output.
