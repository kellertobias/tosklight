# Consistent Pool Object Colors

## Status

**Specification only.** This plan records a future UI color-language feature. It does not implement runtime behavior, persistence, UI changes, command/API behavior, OSC behavior, or executable tests.

## Goal

Give show-object pools a consistent default color language across the desk so an operator can recognize object types immediately, while still allowing individual pool items and user settings to override that language deliberately.

Default object-type colors are:

- Dynamics: light blue or cyan;
- Cuelists and Sequences: lime green;
- Macros: dark red once Macros are implemented;
- Groups: pale yellow, leaning toward a soft orange-yellow; and
- Presets: grey for now.

These colors are presentation defaults. They must not change the meaning of the show object, programmer data, cue content, playback state, or output rendering.

## Pool behavior

Every pool tile or button for one of these object types uses the configured object-type color by default. This applies consistently across all pool surfaces, including compact shortcut strips, full pool windows, pane-level pools, playback assignment pools where Cuelists or Sequences are represented, future Dynamics pools, and future Macro pools.

The default color applies to the whole tile or button treatment, not only a tiny marker. Text contrast, selected state, armed Store/Record/Update state, disabled state, empty slot state, and focus indication must remain readable and must not depend on color alone.

Presets remain grey by default in this feature. The existing preset-family model, including Mixed, Intensity, Color, Position, and Beam, remains separate from the type-level default color. A later feature may choose family-specific preset colors, but this plan only requires that Presets can remain grey unless the operator configures otherwise.

## Individual colors mode

Each relevant pool window or pane can be configured to use individual item colors instead of object-type colors. When a pane is in individual-colors mode:

- the base color for tiles in that pane is grey;
- a tile uses a custom item color only when that specific Group, Preset, Cuelist, Sequence, Dynamic, or Macro has an assigned presentation color;
- items without an individual color do not inherit the object-type default while that pane is in individual-colors mode; and
- the pane setting affects presentation only and does not mutate show-object content.

Individual item colors remain useful for operator-defined labeling, for example one Group tile being warm amber, one Preset tile being blue, or one Cuelist tile being white. The operator must be able to distinguish this local item color mode from the normal type-color mode.

## User settings

Settings must let the user customize the default color for each supported object type:

- Groups;
- Macros;
- Dynamics;
- Cuelists;
- Sequences; and
- Presets by specific preset family.

Preset color settings are family-specific, so Mixed, Intensity, Color, Position, and Beam can each have their own configured default even though the initial product default can remain grey for all of them.

The settings UI must expose a clear reset-to-default action for each color and for the whole color set. The chosen colors must be persisted as desk or user presentation preferences, not as portable show content, unless a later product decision explicitly makes color themes portable.

## Surface requirements

The same configured color language must be used by:

- pane-level pools;
- full object pool windows;
- group shortcut surfaces;
- playback or Cuelist selection surfaces where Cuelists or Sequences are shown as objects;
- future Macro and Dynamics pools;
- hardware-connected software layouts that display these pool tiles; and
- manual/help documentation once implemented.

Attached physical hardware does not need to reproduce arbitrary RGB colors unless the hardware surface supports that output. When hardware feedback cannot show the exact color, it must still use compatible object-type identity, labels, or supported indicator colors rather than inventing a conflicting mapping.

## Implementation notes

The implementation should define one shared object-type color token source used by all pool components. Individual components should not hard-code their own cyan, green, red, yellow, or grey choices.

The color resolver should account for:

- object type;
- preset family where the object is a Preset;
- pane/window individual-colors mode;
- item-specific presentation color;
- Store, Record, Update, selection, and focus states; and
- empty or disabled slots.

Existing per-item presentation settings for Presets should either be reused or generalized so Groups, Cuelists, Sequences, Dynamics, and Macros can participate without parallel one-off storage paths.

## Acceptance coverage

1. A Group pool tile uses the default pale yellow/orange-yellow object-type color when the pane uses type colors.
2. A Cuelist or Sequence pool tile uses the default lime green object-type color when the pane uses type colors.
3. A Dynamic pool tile uses the default light blue or cyan object-type color when Dynamics have a pool surface.
4. A Macro pool tile uses the default dark red object-type color once Macros are implemented.
5. Preset pool tiles remain grey by default unless the user changes preset-family color settings or assigns an individual item color.
6. Switching a pane to individual-colors mode makes uncolored items grey and shows only explicitly assigned item colors.
7. Switching a pane back to type-color mode restores object-type colors without deleting individual item colors.
8. Settings can customize defaults for Groups, Macros, Dynamics, Cuelists, Sequences, and each Preset family.
9. Resetting one configured color restores only that color to the product default.
10. Resetting the whole color set restores all product defaults.
11. Store, Record, Update, selected, focused, disabled, and empty states remain visible and readable on every configured object-type color.
12. All pool components resolve colors through the same shared mapping rather than local hard-coded palettes.
