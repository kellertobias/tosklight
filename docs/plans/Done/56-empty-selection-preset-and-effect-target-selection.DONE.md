# Empty-Selection Preset Target Selection

## Status

**Completed.** The finished refactoring record
[`04-empty-selection-object-target-selection.md`](../refactoring/finished/04-empty-selection-object-target-selection.md)
documents the application action, typed transports, shared pool behavior, cross-surface tests,
manual generation, and semantic feature commit.

## Goal

When no programmer selection is active, tapping a populated Preset once selects the fixtures or entities for which that Preset is defined instead of doing nothing or requiring the operator to select fixtures manually first.

This makes reusable Presets discoverable as selection tools. If a Preset was built for a specific rig subset, the operator can tap it with an empty selection and immediately work with that subset.

## Terminology

"Entities" means every addressable fixture target stored by the object, including whole fixtures, fixture heads, split fixtures, logical heads, or other future selectable targets that the programmer can select directly.

Dynamics are explicitly outside this plan. Their settled pool gesture is defined by [Dynamics Window](../refactoring/finished/16-dynamics/01-dynamics-window.md): tapping a populated Dynamic starts or toggles its matching Programmer Dynamic instance even when the programmer selection is empty. A target-bound Dynamic uses its binding; a targetless Dynamic with no selection targets every compatible fixture. Shift-tapping a populated Dynamic edits it.

## Operator behavior

If the current programmer selection is empty and the operator taps a populated Preset:

- the desk selects every entity for which that Preset has stored content;
- the Preset is not recalled into the programmer on that first tap;
- the resulting selection order follows the stored target order when the Preset has one, otherwise the desk's deterministic fixture order; and
- the selection is the normal programmer selection shared by Stage, Fixture Sheet, command line, keyboard, OSC, and hardware-connected UI.

After that first tap creates a selection, tapping the same object again follows the ordinary recall/apply behavior for a non-empty selection.

## Non-empty selection behavior

When a programmer selection is already active, tapping a Preset keeps the existing behavior for applying or recalling that object to the active selection. This feature must not change ordinary programming flow for operators who already selected fixtures.

Selection-modifier gestures, such as Shift extend, Control/Command toggle, or future hardware modifiers, must be specified before implementation if they are intended to combine object-derived target selection with the current selection. Until then, the required behavior is only the plain single tap with no active selection.

## Empty and partial objects

An empty Preset does not create a selection. It should keep the existing empty-slot behavior for the current surface, such as storing when Record is armed or remaining inactive when it is not.

If a Preset references targets that no longer exist, are not part of the active show, or cannot be selected on the current desk, the implementation must skip unresolved targets and surface an unobtrusive, actionable warning when anything was skipped.

Unpatched fixtures are still selectable and must be included when a Preset was defined for them. Unpatched status suppresses DMX output only; it does not remove the fixture from show membership or programmer selection.

## Surface requirements

The behavior must be consistent across:

- Preset pool panes;
- the full Presets window;
- command/API behavior if an object-tap equivalent is exposed there;
- OSC or attached-hardware pool buttons if they expose the same object tap; and
- Stage and Fixture Sheet feedback after the selection changes.

The tap action must produce the same authoritative selection state no matter which surface initiated it. Components must not keep local-only visual selection that differs from the programmer selection.

## Store, Record, Update, and Set modes

Armed workflows take priority over empty-selection target selection:

- with Record or Store armed, tapping a valid target keeps the existing store/overwrite/merge workflow for that object type;
- with Update armed, tapping a valid target keeps the existing update workflow;
- with Set armed, tapping a valid target keeps the presentation or object-settings workflow; and
- empty-selection target selection applies only to the ordinary unarmed tap.

This prevents the new shortcut from blocking existing pool editing workflows.

## Acceptance coverage

1. With no programmer selection active, tapping a populated Color Preset selects every fixture or head for which that Preset has stored content and does not recall the Preset on the first tap.
2. With no programmer selection active, tapping a populated Position Preset selects every fixture or head for which that Preset has stored content and does not recall the Preset on the first tap.
3. With no programmer selection active, tapping a populated Mixed Preset selects the union of all stored Preset targets exactly once.
4. The selection order after the tap is deterministic and follows stored target order where available.
5. A second tap on the same populated Preset after selection exists performs the ordinary recall/apply behavior.
6. With an existing programmer selection, tapping a Preset keeps the ordinary recall/apply behavior and does not replace the selection with the Preset's stored targets.
7. Dynamics do not inherit this plan's empty-selection gesture; their pool tap behavior follows the canonical Dynamics plans.
8. Empty Preset slots keep their existing empty-slot behavior and do not create a selection.
9. Record/Store, Update, and Set armed workflows keep priority over empty-selection target selection.
10. Missing or deleted referenced targets are skipped without selecting a wrong replacement target.
11. Unpatched fixtures referenced by the Preset are included in the selection.
12. Stage, Fixture Sheet, command line, OSC, and hardware-connected UI surfaces observe the same authoritative selection after the object tap.
