# Storybook component inventory

This inventory covers reusable UI-library components in the shared Controls, Tables, and Window
System / Settings families. The executable component-to-story mapping lives in
`storybook/tests/ui-stories.spec.ts`; its story-index assertion fails when a mapped entry is absent.

## Included controls

| Component entry | Public components demonstrated | Material states in the owning story |
| --- | --- | --- |
| Button | `Button` | variants, active, disabled, loading, compact, icon-only, full-width |
| Text Input | `TextInput`, `TextField` | controlled, clearable, required error, read-only, disabled, modal keyboard |
| Number Input | `NumberInput`, `NumberField` | integer/decimal, bounds, steppers, disabled, modal numpad and fader |
| Multiline Text Input | `TextArea`, `LargeTextInput`, `TextAreaField`, `LargeTextField` | controlled, required error, disabled, multiline modal keyboard |
| Select | `Select`, `SelectField` | controlled options and disabled options/control |
| Checkbox | `CheckboxField` | controlled checked and disabled |
| Radio | `RadioField` | exclusive controlled group and disabled |
| Switch | `SwitchField` | controlled on/off labels and disabled |
| Multi Value Toggle | `MultiValueToggle`, `MultiValueToggleField` | controlled selection and disabled option |
| Cycling Value Toggle | `CyclingValueToggle`, `CyclingValueToggleField` | controlled cycling through meaningful values |
| Grouped Selection Field | `GroupedSelectionField` | grouped choices, descriptions, and clear action |
| File Drop Field | `FileDropField` | idle, loading, success, actionable error |
| Icon Picker Field | `IconPickerField` | controlled picker |
| Color Picker Field | `ColorPickerField` | controlled picker |
| Search Bar | `SearchBar` | ordinary search and optional settings |
| Touch Select | `TouchSelect` | controlled numeric choice |
| Horizontal Fader | `HorizontalFader`, `HorizontalFaderField` | ordinary, disabled, labelled field |
| Form Layout | `FormLayout`, `FormField` | top and side labels, one- and two-column layouts |

The existing dedicated Calendar, encoder, vertical-fader, command, playback, and pool stories remain
their component evidence. They are domain surfaces outside this common-control catalog and are not
folded into the controls above.

`ToskLight/Integration/Form Controls` is retained as a cross-component regression fixture for
scrolling, modal layering, fader/picker geometry, and drag/drop integration. It is deliberately not
used as representative coverage for any catalog component.

`ToskLight/Integration/Input Modal Surfaces` retains low-level keyboard/numpad regression fixtures,
but they are not catalog components. Operator-facing keyboard and numpad paths are demonstrated
through the Text Input, Number Input, and Multiline Text Input component stories.

## Included tables

| Component entry | Public components demonstrated | Material states |
| --- | --- | --- |
| Data Table | `DataTable` | active/selected rows, activation, empty filler and empty table |
| Fixture Sheet Table | `FixtureSheetTableView` | real fixture/logical-head rows and ordered selection |

## Included window system and settings

| Component entry | Public components demonstrated | Material states |
| --- | --- | --- |
| Window Header | `WindowHeader` | ordinary, information, actions, search, active action, Settings |
| Window Settings | `WindowSettings` | modal and anchored/popover presentations with realistic tabs |
| Window Dropdown | `WindowDropdown` | controlled selection, disabled item, menu dismissal behavior |
| Window Frame | `WindowFrame` | composed production window with navigation, information and bottom content |
| Window Scroll Area | `WindowScrollArea` | scrollable populated content and actionable empty state |
| Selection List | `SelectionList` | selected, danger, disabled and empty states |
| Selection Tree | `SelectionTree` | controlled multi-column selection and footer action |
| Button Grid | `ButtonGrid`, `GridButton` | ordinary, active, selected, empty, disabled and store-target cells |
| Modal Layer | `ModalProvider`, `ModalLayer`, `ModalFrame`, `ModalRegistration`, `ModalPortal`, `ModalTitleBar` | nesting, close policies, title configuration, portal and application registration |

## Deliberate exclusions

| Export or helper | Reason |
| --- | --- |
| `Field` | Compatibility alias of `FormField`; the Form Layout story exercises the implementation. |
| `Input` | Compatibility HTML input wrapper; the Number Input story owns the supported number-editing surface. |
| `HorizontalTouchFader` | Alias of `HorizontalFader`; a duplicate entry would present the same component twice. |
| `SelectionCardContent` | Component-owned subcontent of `GroupedSelectionField`, not a meaningful standalone control. |
| `FadedDivider`, `TitleBarSearchDivider` | Structural separators with no standalone interaction or operator state. They remain visible in owning title/form stories. |
| `ModalCaretValue`, `ModalNumberValue`, `ModalNumberInput`, `ModalTextKeyboard` | Input-modal internals. The actual keyboard and numpad stay operable inside Text Input, Number Input, and Multiline Text Input stories instead of becoming forbidden top-level stories. |
| `useWindowSettings` | Context hook rather than a renderable component. |
| `TouchSelect` low-level `SelectField` delegation | The wrapper has an operator-facing numeric contract, so it is retained as a story despite its deliberately small implementation. |

TL-224 owns any future shared title-chrome API or migration. This catalog only demonstrates the
current real components and must follow TL-224 after that work lands; it does not introduce a
parallel title action model.
