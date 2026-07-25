# Desk-Wide Highlight Look

## Status

**Specification only.** This plan records a future Highlight configuration and ownership change. It does not implement Desk Setup, fixture semantics, output behavior, show migration, UI removal, documentation screenshots, or executable tests.

## Goal

Move operator-facing **Highlight Look** configuration out of individual Show Patch rows and into **Show > Desk Setup > Programmer**. Highlight is normally an operator/desk preference that should identify fixtures consistently across shows instead of requiring raw per-channel values on every patched fixture.

The desk-level look is semantic. The operator chooses visible lighting intent; the fixture profile remains responsible for translating that intent into its exact channels, functions, ranges, color system, resolution, and raw DMX.

## Configurable look

Desk Setup exposes:

| Part | Configuration |
|---|---|
| Intensity | Required normalized value. Highlight always applies this value where the fixture has Intensity. |
| Shutter | Always the semantic **Open** function where a Shutter exists. It is not configured as a raw value and cannot be changed to strobe or closed. |
| Color | **Ignore** or one abstract color choice. |
| Iris | **Ignore** or one normalized value. |
| Zoom | **Ignore** or one normalized/physical value using the attribute's normal unit. |
| Focus | **Ignore** or one normalized/physical value using the attribute's normal unit. |
| Frost | **Ignore** or one normalized value. |

Every optional field defaults to **Ignore** unless an existing installation can be migrated unambiguously. Ignore means Highlight contributes no value for that attribute: the current programmer, playback, default, or lower-priority owner remains visible.

Position, Gobos, Prisms, Shapers, Media, Control actions, lamp commands, reset functions, and every other unlisted attribute remain unchanged by the desk-level Highlight Look.

## Semantic fixture resolution

Correctly authored fixtures identify Shutter Open as a semantic channel function. Highlight selects that function rather than assuming that a raw maximum opens the shutter. A profile without an identifiable Open function produces a visible fixture-authoring warning and retains its safe value; ToskLight must not guess an unsafe raw range.

When Color is enabled, the engine asks for the configured abstract color:

1. use calibrated additive or subtractive color mixing when the fixture provides it;
2. otherwise choose the closest authored semantic or measured slot on a discrete color wheel; and
3. if neither path can represent the color, leave Color unchanged and report that limitation instead of choosing an arbitrary wheel slot.

The same resolution applies to every logical head that owns the relevant attributes. Values are clamped through the normal attribute and physical-range rules.

The fixture library continues to own semantic channel/function definitions, safe defaults, and exact raw conversion. Profile **Highlight raw** values remain a compatibility and fixture-validation seam until migration is complete, but the normal operator must no longer edit a raw per-fixture Highlight map in Show Patch.

## Runtime behavior

The new look changes only the transient HIGH/Highlight contribution. It does not create programmer values, activate attributes for recording, alter Cue tracking, or enter Undo history when Highlight is toggled or stepped.

**Highlight patch selection via DMX** remains a separate Desk Setup switch controlling whether Patch preview selection reaches physical output. When it does, it uses the same desk-wide Highlight Look; it does not introduce a second look editor.

Grand Master, Blackout, hazardous-fixture safety, group-master bypass rules, Highlight ownership, ALL/PREV/NEXT stepping, and logical-head selection retain their established precedence unless a linked plan explicitly changes them.

## Ownership and migration

The configured look is server/installation-wide Desk Setup data rather than user, screen, browser-local, or portable-show data. It applies to every operator desk connected to that ToskLight server. Existing per-fixture raw Highlight overrides cannot be silently discarded. Migration must:

- preserve the original show and its override maps;
- import a common look automatically only when the existing values can be represented unambiguously;
- report fixtures whose raw overrides conflict or cannot be translated; and
- provide an explicit compatibility decision before removing legacy override evaluation.

Newly saved shows must not acquire raw per-fixture overrides merely because Highlight was used.

## Acceptance coverage

1. Highlight Look is configured in Desk Setup and is absent from ordinary Show Patch editing.
2. Intensity is always configured; Color, Iris, Zoom, Focus, and Frost independently support Ignore.
3. Shutter uses an authored semantic Open function and never an assumed raw maximum.
4. A configured abstract Color resolves through additive/subtractive mixing or, when those are absent, through the closest suitable color-wheel slot.
5. Unsupported or unrepresentable optional attributes remain unchanged and produce useful configuration feedback.
6. Position, Gobos, Prisms, Shapers, Media, and Control values remain untouched.
7. The same look works for complete selection, step-through, logical heads, and optional Patch-preview DMX Highlight.
8. Highlight remains transient and cannot be recorded as programmer or Cue data.
9. Desk settings apply consistently across shows but do not modify portable show content.
10. Existing raw override maps are preserved and migrated or rejected explicitly rather than silently lost.
