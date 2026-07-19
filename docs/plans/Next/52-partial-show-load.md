# Partial Show Load

## Status

**Specification only.** This plan records a selective show-loading feature. It does not implement runtime behavior, persistence changes, UI changes, or executable tests.

## Goal

Let an operator choose another show and load only selected parts of it into the current show. The load can include whole feature areas, such as Effects, Presets, Groups, Cuelists, Playbacks, Macros, Dynamics, Stage layout, and Fixture Patch, or narrower slices such as only Color Presets, only Position Presets, only selected Cuelists, only selected Macros, only the complete patch, or only one patch layer.

The operator must choose whether the selected content is loaded by replacement or by addition. In both modes, references between imported and existing objects must resolve deterministically so Cues, Presets, Groups, Macros, Dynamics, fixture references, patch layers, and future show objects continue to point at the intended targets.

## Operator workflow

From the Show menu, the operator chooses a show from the show library or a valid `.show` file. Before the current show is changed, the desk opens a partial-load preview that lists importable sections with counts, dependencies, conflicts, and reference effects.

The preview must support selecting:

- Fixture Patch, with the option to load all layers or only specific layers;
- Stage geometry and scenery that belong to the selected fixture or patch layer scope;
- Groups;
- Presets by family: Mixed, Intensity, Color, Position, and Beam;
- Effects and Dynamics;
- Cuelists and Cues;
- Playbacks and page assignments;
- Macros and scheduled Macros once those show objects exist; and
- any future portable show object type that participates in the general selective-import workflow.

The confirmation step must clearly show whether the operation is **Replace by position** or **Add to end**. It must also show what references will be preserved, rewritten, left unresolved, or skipped before the operator commits the load.

## Load modes

### Replace by position

Replacement load maps selected imported objects onto the same numbered position, pool slot, page slot, fixture number, patch layer, or other documented positional address in the current show. Existing objects at those positions are replaced by the imported objects. Existing objects outside the selected scope are preserved.

References must follow the replaced position. For example:

- existing Cues that referenced Color Preset 1 must reference the new imported Color Preset 1 after Color Preset 1 is replaced;
- imported Cues that reference Position Preset 3 must resolve to the object that lands at Position Preset 3 in the destination show;
- imported Groups that reference Fixture 12 must resolve to the destination Fixture 12 when the operation is positional; and
- Playbacks loaded by page/button position must use the resulting Cuelist or special object at the loaded position.

If a selected imported object references an object type or position that is not selected for replacement, the preview must identify whether it will bind to the existing destination object at that position, pull in the dependency, remain unresolved, or block confirmation.

### Add to end

Additive load appends selected imported objects after the current destination section. New pool slots, Cuelists, playback pages or buttons, patch layers, Macros, Effects, and other selected sections receive destination positions that do not overwrite existing objects.

References inside the imported set must be rewritten to the newly appended imported objects. For example:

- an imported Cue that referenced imported Color Preset 1 must reference the appended copy of that Color Preset, not the destination show's existing Color Preset 1;
- an imported Macro that calls an imported Cuelist must reference the appended Cuelist;
- imported Groups and Presets must target the appended or selected imported fixtures when the corresponding patch content is loaded; and
- appended Playbacks must point to appended Cuelists, Speed Masters, Group Masters, Macros, or other imported targets.

When an imported object intentionally references a destination object outside the imported set, the preview must make that binding explicit. Silent fallback to same-number destination objects is not allowed in additive mode.

## Fixture Patch and layer handling

Partial Fixture Patch loading must preserve fixture identity, fixture numbers, labels, profiles, modes, logical heads, split fixtures, multipatch placement, address state, Stage geometry, patch-layer membership, and unpatched-fixture semantics within the selected scope.

Loading only a specific patch layer must include every fixture, geometry record, and patch-layer relation needed for that layer to remain coherent. References from selected Groups, Presets, Cues, Effects, Macros, or Stage data to fixtures outside the selected layer must be previewed as explicit dependencies or conflicts.

Replacement by fixture number or layer position must not accidentally delete fixtures outside the selected layer. Additive layer load must allocate non-conflicting fixture numbers or require an operator-approved renumbering plan before confirmation.

## Dependency and conflict preview

The preview is part of the feature contract, not an optional convenience. It must show:

- selected object counts by section and family;
- dependencies required by the selection;
- destination conflicts by number, position, stable identity, page slot, fixture number, layer, and name where applicable;
- duplicate or identical objects that can be skipped;
- references that will be preserved by position;
- references that will be rewritten to appended imported copies;
- references that will bind to existing destination objects; and
- unresolved references that block the import or require an explicit operator choice.

The final operation must apply atomically as one show revision. A failed validation, migration, compile, persistence write, or runtime activation must leave the current show unchanged.

## Reference rules

The import service must treat show objects as typed objects with stable identities and documented positional addresses. It must not rely on display names alone.

Replacement mode is position-first: imported references resolve through the destination positions produced by the replacement map unless the preview marks a dependency as unresolved or explicitly bound elsewhere.

Additive mode is copy-first: imported references resolve to the copied imported objects produced by the append map. Binding an imported reference to an existing destination object requires an explicit preview row and deterministic rule.

Objects not selected for import must not be partly created just because another selected object mentions them. Dependencies are either automatically included by a documented rule, explicitly selected by the operator, bound to an existing destination object, or reported as a blocking conflict.

## Surface and compatibility requirements

Partial Show Load should use the general selective-import workflow described by the major refactor plan rather than separate copy paths for Presets, Macros, Dynamics, patch data, or Cuelists.

The Show menu, File Manager show picker, command/API surface, and any future OSC or hardware action that starts a partial load must use compatible vocabulary for selected sections, replacement mode, additive mode, dependency preview, conflict state, and confirmation.

Existing whole-show load remains available for operators who want to switch shows rather than merge content. Partial Show Load changes the current show by importing selected content into it; it is not a show switch.

## Acceptance coverage

1. The Show menu can choose another show and open a partial-load preview without mutating the active show.
2. The preview can select and deselect Effects, Presets by family, Groups, Cuelists, Playbacks, Macros when available, complete Fixture Patch, and individual patch layers.
3. Replacement load of Color Presets replaces selected preset positions, and existing Cues that referenced those positions use the new imported presets.
4. Replacement load of imported Cues resolves their preset references through the destination positions created by the replacement map.
5. Additive load of Presets and Cues appends the imported presets and rewrites imported Cue references to those appended presets rather than existing same-number presets.
6. Additive load of Macros or Effects rewrites references to appended imported dependencies and does not silently bind to same-number destination objects.
7. Loading only one patch layer imports or replaces only that layer's fixtures and layer relations, while fixtures outside the selected layer remain unchanged.
8. Patch-layer import previews every selected Group, Preset, Cue, Effect, or Macro reference to fixtures outside the selected layer as a dependency, explicit destination binding, or conflict.
9. Identical destination objects can be skipped only when stable identity and content comparison prove they are identical for the selected object type.
10. A reference that cannot be resolved, rewritten, or explicitly bound blocks confirmation with an actionable preview message.
11. The confirmed partial load is one atomic show revision and one Undo step.
12. A failed import validation or runtime activation leaves the active show, output, and persisted file unchanged.
