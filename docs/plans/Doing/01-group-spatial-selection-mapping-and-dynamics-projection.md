# Groups and Dynamics Spatial Mapping, Settings, and SET Routing

## Status

**Doing.** Claimed on 2026-08-02 after completing Plan 00A. This is the sole active implementation
contract for Group settings and interaction, spatially ordered Group selection, Playback-owned
Group Masters, and Dynamics projection.

This was the first item in the current [Next plan order](../Next/README.md). The earlier dependency on
Macros has been explicitly removed by the operator's queue reorder. The completed supported-scale,
output-isolation, and warm-UI work remains its architectural prerequisite. The goal's explicit
`Next` to `Doing` claim protocol supersedes this specification's older instruction to leave the file
in `Next`; the retired refactoring `doing` folder remains outside this queue.

## Progress

- [x] Claimed from `docs/plans/Next` in numeric order.
- [x] Audit existing Group, Dynamic, Layout, SET-routing, Playback Group Master, persistence, wire,
  help, and acceptance-test seams.
- [x] Add the portable projection, shape, override, and ranked-selection model with deterministic
  pure-domain evaluation and validation.
- [ ] Implement the authoritative source, projection, shape, ranking, inheritance, and migration
  domain model with focused tests.
- [ ] Implement rank-aware programming and Dynamic evaluation plus explicit SET/Playback routing.
- [ ] Replace the retired Layout and Group context-menu surfaces with the specified Group and
  Dynamics settings workflows.
- [ ] Update generated contracts, help/manual, human acceptance scenarios, and focused Playwright
  coverage through repository workflows.
- [ ] Run focused checks, required major suites, migration/recovery proof, and the real desktop path.

## Implementation decisions

- Preserve the integrated plan as one ownership contract, but commit coherent domain, persistence,
  routing, UI, documentation, and verification steps independently.
- Treat the literal operator grammar, three-tab modal, absence of Layout authority, shared Group
  Master ownership, and backward-compatible migration as acceptance requirements rather than
  design suggestions.
- Keep Stage-authored 2D positions, projections, and regeneration intact. The retired authority is
  specifically the Layout built-in/pane, Group `grid`, Programmer selection-grid configuration and
  gestures, and their desk-layout identities.
- Preserve legacy per-lane Dynamic spatial ordering through a backward-compatible read/runtime
  shim until it can be normalized deliberately. Existing PerLane definitions can contain different
  lane orderings, so silently choosing one definition-level override would violate the required
  output-preserving migration; new authoring will expose only the definition-level mapping.
- Treat portable Dynamic decoding as the compatibility boundary, not only show compilation. Dynamic
  definitions also occur in Cue fallbacks, physical and virtual Playbacks, persisted output-runtime
  instances, Programmer/Preload values, and Undo/session snapshots.

## Implementation verification

- The audit found current legacy authority in `GroupDefinition.fixtures`, `derived_from`, `grid`,
  `master`, and `playback_fader`; spatial `PhaseOrdering` on both definition and lanes; the registered
  Layout built-in/pane plus persisted `layoutGroupId`; the Programmer selection-grid actions; two
  obsolete Group management surfaces; text/regex-derived SET assignment; and frontend-only Dynamic
  preview based on incidental current selection.
- Existing reusable seams include revisioned Group management, Playback `configureSlot` with
  concrete page/object revision guards, explicit Playback Configuration targets, Group live/frozen
  selection actions, and independent overlapping Group-Master HTP coverage.
- Commit `3d6fff28` adds portable `f64` 3D projection and Grid/Radial/Radar rank evaluation, named
  Top/Front/Back/Left/Right presets, preset-hint reconciliation, exact shared ranks, missing-position
  warnings with individual fallback ranks, Dynamic-only Random configuration, explicit inherit/
  replace stages, finite-field validation, zero-vector rejection, degree normalization, and negative
  zero canonicalization.
- `cargo test -p light-dynamics --lib` passed all 40 tests; `cargo clippy -p light-dynamics
  --all-targets -- -D warnings` and `git diff --check` passed.
- Commit `dc0927ea` adds the canonical explicit/reference Group source, ordered multi-reference
  evaluation with first-occurrence deduplication, legacy `fixtures`/`derived_from` fallback,
  complete cycle-path diagnostics, local/inherited/mixed mapping provenance, and live Stage-position
  ranking. All 120 `light-programmer` library tests, formatting, and the focused diff check passed.
  Strict package Clippy currently also reports a pre-existing `type_complexity` warning in untouched
  `registry.rs`; rerunning with only that unrelated lint allowed passed with warnings otherwise
  denied.
- Commit `45ab0755` adds the pure desk/show/surface-scoped SET state machine and typed Group and
  current-page/explicit-page Playback identities. The six explicit routing intents do not inspect
  selection or other incidental UI state. Its two focused Vitest files passed all 16 tests, desktop
  TypeScript typecheck passed, Biome passed, and `git diff --check` passed.
- Commit `25ac7991` adds a serde-defaulted definition-wide Dynamic spatial override while retaining
  legacy uniform and per-lane `PhaseOrdering` unchanged as a compatibility-only read/runtime path.
  Absent and explicit inherit/inherit states canonicalize to omission; explicit projection and
  Random replacement round-trip. All 43 `light-dynamics` tests and warning-clean Clippy passed.
  Commit `8fbf25c9` updates the remaining Rust/benchmark definition fixtures for the new field.
- Commit `852afbac` makes Group recording write canonical explicit/reference sources, materialize
  merge/subtract in first-occurrence order, preserve empty Groups, block deletion through canonical
  references, and stop copying the retired selection grid. Its nine focused tests passed, and the
  agent's full `light-programmer` run passed all 121 tests.
- Commit `85c5d1f2` accepts canonical source-only Group bodies, preserves canonical-over-legacy and
  unknown-field losslessly, and imports/remaps explicit fixture IDs plus every live Group reference.
  The active-show object suite passed 19 tests and selective import passed 44 tests.
- Commit `8b7ec32e` validates canonical reference rules, reference resolution, and spatial mapping at
  the engine snapshot boundary. All 94 `light-engine` tests passed. Strict Clippy exposed six
  existing lints in untouched Playback, Programmer, profile-projection, and test files; rerunning
  with only those lints allowed passed with warnings otherwise denied.
- Commit `591a3266` propagates canonical sources through Update, frozen refresh, detach/materialize,
  and the legacy headless record-command path without authoring new legacy derivation authority.
  Thirteen Group-management tests, the focused Update test, and the real headless record-command
  test passed; package formatting and diff checks passed.
- Commit `041bae00` gives standalone canonical `group_id` references stable occurrence-aware
  lossless-array identity, so unknown fields follow duplicate references through reordering and are
  removed with deleted references. All 72 `light-show` tests, warning-clean strict Clippy, package
  formatting, and the focused diff check passed.
- Commit `77ed974c` migrates legacy explicit and derived Group sources at startup while preserving
  frozen membership, empty Groups, retired grid data, unknown fields, canonical precedence, object
  revisions, and byte-stable idempotence. The seven object-migration tests and focused migration
  proof passed. Commit `feea8e9f` contains only the subsequent guard simplification.
- Commit `89231433` preserves full Stage X/Y/Z coordinates for fixture and logical-head spatial
  identities while legacy 2D layouts remain X/Z with Y defaulted to zero. All 44 dynamics tests,
  the 15 show-compiler tests, and the isolated Stage-position test passed.
- Commit `9f1073e6` makes Programmer and Cue spreads consume authoritative spatial ranks, preserves
  exact-rank peers in parallel, and invalidates Playback compilation when Stage positions change.
  All 96 engine tests and 11 focused application value-action tests passed, and the headless crate
  checked successfully. Strict Clippy passed with only the repository's existing unrelated lint
  classes explicitly allowed; an unqualified strict run still reports those pre-existing findings.

## Remaining work

- Complete every unchecked progress and acceptance item, then add a truthful `## Result` before
  moving this plan to `docs/plans/Done`.
- Retire the legacy Group source fields from live authority only after all remaining readers and
  recovery paths have been audited. Recording, mutation, validation, selective import, startup
  migration, structural lossless identity, and idempotence proof are integrated; tolerant legacy
  reads remain deliberately available until the final compatibility pass.
- Feed the committed Group ranks through Dynamic inheritance/override evaluation and migrate every
  persisted legacy `PhaseOrdering` surface without changing existing output.
- Move Group Master level authority out of `GroupDefinition.master`/`playback_fader` and into the
  shared Playback-target runtime, with deterministic legacy reconciliation and migration proof.
- Connect the typed SET state machine to the command line, Group tiles, Playback controls, and
  revision-guarded application commands; its committed pure model is not runtime proof.

## Decision and superseded behavior

ToskLight must **not** provide a built-in **Layout** screen or pane for editing or displaying 2D
selection geometry. Spatial selection mapping belongs to Group settings and, for a local override,
the Dynamics editor. This contract applies to Dynamics now and may later be reused for timing
offsets; that later use is not part of this plan.

The completed Plan 26 implementation predates this decision. Its `Layout` pane/window, desk-local
selection-grid state, and grid-reordering gestures must not become a second authority for this
feature. During implementation, retire those operator surfaces and reconcile their persisted data
as described below. Do not extend `LayoutWindow`, the current `GroupDefinition.grid`, or the
current per-selection grid into the new canonical model.

The ordinary ordered fixture sequence remains a compatibility fallback when no canonical mapping
exists. There is no show-global or desk-global selection layout, and selecting loose fixtures does
not silently create or persist one.

The current Group/Playback SET path and Group context menu also predate this contract. Today a
Playback SET can rely on the incidental last selected Group, and Group settings expose a
Group-owned Master slider plus membership/selection actions. The desired contract below removes
that ambiguity and makes the settings modal, selection gestures, assignment source, and Group
Master owner explicit.

## Product intent

A Group has two independent concerns:

1. a **live fixture source**, consisting of explicitly stored fixtures or live reference(s) to
   another Group; and
2. an optional **canonical spatial selection mapping**.

The mapping is a two-stage pipeline:

1. project every resolved fixture's current 3D Stage position into a 2D coordinate; then
2. transform those 2D coordinates into ranked, shaped selection positions.

The ranked result is authoritative for operations whose meaning depends on selection order or
position, including `THRU` value spreads and Dynamics phase distribution. Fixtures remain members
even when unpatched; patch state only controls DMX output.

The model must preserve three distinct things:

- source membership and its live reference graph;
- an effective mapping, inherited or local; and
- the evaluated ranked selection for current positions.

Neither the UI nor persistence may flatten those three into a private fixture list plus an
unexplained order.

## Terminology

- **Source order**: the stable ordered fixture sequence obtained from explicit membership or live
  Group-reference evaluation before spatial mapping.
- **Projection**: stage one, converting `(x, y, z)` Stage coordinates to `(u, v)`.
- **Shape**: stage two, converting `(u, v)` into a sortable spatial key.
- **Rank**: one position in the shaped selection. More than one fixture may have the same rank.
- **Canonical mapping**: the effective Group projection plus shape after inheritance and override
  resolution.
- **Local mapping**: a mapping stored on the Group itself rather than inherited from a source.
- **Dynamics override**: a Dynamic-owned replacement for the projection, the shape, or both. It
  never writes Group settings.

## Data model

### Live fixture source

Introduce one canonical Group source representation in the Group domain rather than inferring
source authority from a mixture of `fixtures`, `derived_from`, and cached resolved membership:

```rust
enum GroupFixtureSource {
    Explicit {
        fixture_ids: Vec<FixtureId>,
    },
    References {
        references: Vec<GroupReference>,
    },
}

struct GroupReference {
    group_id: String,
    rule: SelectionRule,
}
```

`fixture_ids` and `references` are ordered. Resolution removes later duplicates without moving the
first occurrence. A reference applies its existing ordered `SelectionRule` after resolving its
source. Multiple references concatenate their results in stored reference order and then apply the
same first-occurrence rule. Intentionally empty explicit Groups and live references that currently
resolve empty remain valid and distinct from absent Groups.

The first implementation may expose only the currently supported single-reference recording flow,
but the persisted/domain model and evaluator must correctly support multiple references so it does
not need another compatibility rewrite.

### Spatial mapping

Add a portable, show-owned model shared by Groups, Dynamics, value spreading, and future timing
offset consumers:

```rust
struct SpatialSelectionMapping {
    projection: SpatialProjection,
    shape: SpatialSelectionShape,
}

struct SpatialProjection {
    anchor: Position3d,
    view_direction: Vector3,
    rotation_degrees: f64,
    preset: Option<ProjectionPreset>,
}

enum ProjectionPreset {
    Top,
    Front,
    Back,
    Left,
    Right,
}

enum SpatialSelectionShape {
    Grid {
        angle_degrees: f64,
        direction: RankDirection,
    },
    Radial {
        center_u: f64,
        center_v: f64,
        direction: RadialDirection,
    },
    Radar {
        center_u: f64,
        center_v: f64,
        start_angle_degrees: f64,
        sweep: RadarSweep,
    },
}
```

`GroupDefinition` stores `mapping: Option<SpatialSelectionMapping>`. `None` is meaningful: use
source order, with one rank per fixture. A preset is an operator convenience that writes a complete
direction/rotation configuration; manual editing changes the visible state to **Custom**. The
anchor remains independently editable when a preset is selected.

Coordinates follow the established Stage convention: X is left/right, Y is depth into the room,
and Z is vertical. Presets have literal views:

- **Top** looks down along `-Z`, with `+X` to the right and `+Y` toward the top of the plane;
- **Front** looks from the audience toward the Stage along `+Y`, with `+X` right and `+Z` up;
- **Back** looks along `-Y`, with screen-right chosen so the view is a true rear view and `+Z` up;
- **Left** and **Right** look along `+X` and `-X` respectively, with `+Z` up.

Persist the resulting normalized vector and rotation, not a camera matrix. The preset tag is a
presentation hint and must never disagree with the numeric values; decoding clears it to Custom if
they do not match the preset definition.

### Dynamics mapping override

Add this to `DynamicDefinition` and its embedded fallback snapshot:

```rust
struct DynamicSpatialMappingOverride {
    projection: OverrideStage<SpatialProjection>,
    shape: OverrideStage<DynamicSelectionShape>,
}

enum OverrideStage<T> {
    Inherit,
    Replace(T),
}

enum DynamicSelectionShape {
    Grid { /* same fields as the canonical shape */ },
    Radial { /* same fields as the canonical shape */ },
    Radar { /* same fields as the canonical shape */ },
    Random { seed: u64 },
}
```

An absent override decodes as `Inherit` for both stages. **Random** is deliberately position
independent: it uses the resolved target set and stable fixture identity plus the stored seed (and
loop index where the existing each-loop behavior is selected), and it does not evaluate or require
a projection. Random is a Dynamics shape only; it is not a Group canonical mapping.

The implementation may represent the two `Inherit` values more compactly on disk, but generated
wire types must expose their meaning explicitly. Do not use `null` to mean both “inherit” and
“invalid/missing”.

## Projection and ranking algorithm

All evaluation is authoritative server/domain behavior. The frontend sends intent and previews
the returned configuration/result; it does not independently decide fixture ranks used for output.

### Stage 1: 3D to 2D

For each resolved fixture or selectable logical head:

1. Read its resolved current Stage position. Multipatched physical outputs do not create extra
   selection items. An unpatched fixture remains eligible.
2. Subtract the mapping anchor from the position.
3. Normalize `view_direction`. Reject a zero, non-finite, or effectively zero-length vector.
4. Build a deterministic orthonormal plane basis. Use global `+Z` as the preferred up vector;
   when it is parallel to the view direction, use global `+Y`. Construct screen-right and
   screen-up with a fixed cross-product order, then apply `rotation_degrees` around the normalized
   view direction.
5. Compute `u` and `v` as dot products against the rotated right/up vectors.
6. Canonicalize negative zero to positive zero before comparison and persistence-facing preview.

Use finite `f64` calculations for projection and ranking even if Stage storage remains `f32`.
Equality is exact after the deterministic calculation and zero canonicalization; do not introduce
a hidden distance tolerance. If a future operator-configurable tolerance is wanted, it requires a
separate explicit contract.

Fixtures with no valid 3D position remain in the result. Append them after all positioned ranks in
source order, one rank per fixture, and return a visible warning in the evaluation preview. They
must not be discarded, made unselectable, or collapsed into one shared unknown-position rank.

### Stage 2: shaped ranks

Evaluate a scalar key for every positioned fixture:

- **Grid**: dot `(u, v)` against the configured grid angle. Ascending or descending direction is
  explicit. Fixtures on the same projected line share a rank.
- **Radial**: Euclidean distance from `(center_u, center_v)`. **Outward** orders centre to edge;
  **Inward** reverses it. Equal radii share a rank.
- **Radar**: normalized polar angle around `(center_u, center_v)`, measured from the configured
  start angle in the configured clockwise/counter-clockwise sweep. Equal angles share a rank.

Sort by the scalar key with source order as the deterministic tie-breaker for display and later
operations that still need a fixture sequence. Assign a new rank only when the scalar key changes.
Therefore fixtures at the same projected `(u, v)` always share the same rank and value; Radial and
Radar may also intentionally group different coordinates with equal radius or angle.

The evaluation result is a transient value such as:

```rust
struct RankedSelection {
    ordered_fixture_ids: Vec<FixtureId>,
    rank_by_fixture: HashMap<FixtureId, usize>,
    rank_count: usize,
    warnings: Vec<SpatialMappingWarning>,
}
```

Do not persist projected coordinates, evaluated ranks, or cached fixture lists as authority. Cache
them only behind dependency keys that include Group/source revisions and Stage-position revision,
and invalidate them on any relevant change.

## Group-reference evaluation and precedence

Resolve one Group with a recursion stack and return membership, source order, effective mapping,
ranked selection, provenance, and warnings. Cycles are invalid and must name the complete reference
path. Missing Group references remain invalid; deletion stays blocked while a live dependent exists.

Apply this precedence from lowest to highest:

1. An explicit source provides its stored order and no inherited mapping.
2. A referenced source provides its current resolved membership, effective mapping, and ranked
   order. Membership and position changes flow live.
3. A Group-local mapping replaces the inherited mapping and recomputes ranks from the fully
   resolved membership. It never freezes or copies membership.
4. A Dynamics-local stage override replaces the corresponding effective Group stage for that
   Dynamic only. It never mutates the Group or any source Group.

For one reference, inheritance is literal: a derived Group with no local mapping uses the source's
effective mapping and selection order. A local mapping starts from the source's current resolved
fixture membership, replaces inherited mapping/order, and remains local when source membership or
mapping later changes. Source membership changes still update the derived Group's contents.

For multiple references:

- if every non-empty referenced result has the same effective mapping, inherit that mapping and
  evaluate it once over the combined resolved membership;
- if mappings differ, or some non-empty references have a mapping while others do not, there is no
  inherited canonical mapping. Preserve the concatenated source results and show **Mixed source
  mappings — source order** in Group settings;
- a Group-local mapping always resolves that mixed state by replacing all inherited mapping/order
  for the combined membership;
- empty references do not create a false mapping conflict, but their provenance remains visible.

Nested references apply these rules recursively. A local override at any source level becomes that
source Group's effective mapping for descendants. Editing a nested source mapping immediately
updates descendants that inherit it, but never changes descendants with their own local mappings.

## Selection-sensitive value semantics

When the authoritative selection expression is exactly one live Group, selection-order-sensitive
operations use that Group's current `RankedSelection`. This includes command-line, software,
keyboard, OSC, HTTP, and attached-hardware paths because all reach the same server-side action.

`0 THRU 100` distributes across **rank count**, not fixture count. All fixtures in one rank receive
the same sampled value in one mutation and one Undo step. For a top-down outward Radial mapping,
fixtures at the centre therefore receive 0 and fixtures at the outer rank receive 100. An
intermediate equal-radius ring shares one intermediate value.

If the Group has no effective mapping, preserve the current ordered-membership behavior with one
rank per fixture. Existing multi-point `THRU` validation counts ranks when a mapping exists and
fixtures otherwise. Frozen/dereferenced operations resolve the ranks once and store/apply the
resulting per-fixture values; later membership or mapping edits do not redistribute those frozen
values. Stored live Group spreads retain their control points and re-resolve against current live
membership, mapping, and positions at recall/output resolution.

An additive, subtractive, ranged, Stage, Fixture Sheet, or otherwise direct selection is not
secretly associated with the last selected Group. It uses its explicit current selection order,
one rank per fixture. A future explicit command may derive a new Group or timing map from it, but
this plan adds no global selection mapping.

## Group selection, SET, and Playback grammar

Selection is live Programmer state, not assignment intent. No Playback configuration or assignment
path may read `selectedGroupId`, selected fixtures, a remembered Highlight source, the last opened
Group, or Playback contents as a fallback assignment source.

The normative command/control grammar is:

| Intent | Command/control sequence | Result |
| --- | --- | --- |
| Select Group live | `[GRP] <group> [ENTER]` or plain Group click | Select the Group's current live membership and retain its live reference. |
| Select Group frozen | `[GRP][GRP] <group> [ENTER]` or double quick press | Select the Group's current membership as individual frozen fixtures. |
| Open Group settings | `[SET] [GRP] <group> [ENTER]` | Open that Group's settings modal. `ENTER` is significant on the command line. |
| Assign Group Master | `[SET] [GRP] <group> [PLAYBACK] <address> [ENTER]` | Assign that explicit Group as that explicit Playback's Group Master target. |
| Open Playback settings | `[SET] [PLAYBACK] <address> [ENTER]` | Open Playback Configuration for that Playback regardless of any fixture or Group selection. |

`<address>` preserves the established difference between current-page Playback addressing and an
explicit page/playback address. A direct pool/button interaction that supplies the concrete
Playback identity does not require Enter: SET, Group source, then Playback assigns; SET followed
directly by Playback opens Playback Configuration.

For Group tiles, plain click selects live, double quick press selects frozen, and completed
SET-click plus right-click both open the same Group settings modal. Right-click suppresses the
browser context menu. The direct-interaction router must distinguish the typed intents
`SelectGroupLive`, `SelectGroupFrozen`, `OpenGroupSettings`, `OpenPlaybackSettings`,
`ChooseGroupMasterSource`, and `AssignGroupMaster`; it must not dispatch an ordinary selection and
then infer the later operation from global state.

The command parser/control router uses an explicit state machine:

```text
Idle
  SET -> SetArmed

SetArmed
  Group N -> GroupSourcePending(N)
  Playback P -> OpenPlaybackSettings(P) -> Idle

GroupSourcePending(N)
  Enter -> OpenGroupSettings(N) -> Idle
  Playback P -> AssignGroupMaster(group=N, playback=P) -> Idle
  Clear/Cancel/scope change -> Idle
```

Direct Group-settings and frozen/live-selection gestures may route straight to their terminal
typed intents. A direct Playback after a Group source routes straight to `AssignGroupMaster` and
does not need to synthesize command text. While pending, the visible command line shows the source,
for example `SET GROUP 4`.

A pending Group source exists only inside that active SET interaction. Clear/Cancel, loss of the
originating surface, show or desk replacement, or an unrelated target discards it. Do not use a
timeout to guess whether the operator meant settings or assignment. Software-latched and
attached-hardware-held SET may differ physically, but must produce the same typed outcomes.

Reject incomplete, missing, stale-page, stale-revision, or scope-mismatched targets without partial
mutation. Bare SET + Enter never consults selection. If the legacy
`SET GROUP <group> AT <page> . <slot>` form remains as an input alias, normalize it to the same
typed `AssignGroupMaster` application command and omit it from new operator help.

Membership replacement is not a Group-settings action. It remains the explicit
`[REC]` → Group → **Override** workflow; merge/subtract and reference semantics otherwise retain
their existing commands.

## Group Master ownership

A Group Master exists only as a Playback whose target is `Group { group_id }`:

- the Group ID identifies one shared master level and runtime contribution across every Playback
  assignment targeting that same Group;
- each Playback assignment retains its own fader/button layout, held Flash state, name/color
  presentation, configuration revision, and physical-fader pickup state;
- the Group owns membership and Group configuration, but no master value or assigned-Playback
  pointer;
- SET + explicit Group + explicit Playback creates or replaces that Playback target through the
  existing Playback Configuration/application service;
- choosing another Playback function or **None**, or deleting an assignment, removes the Group
  Master only when it was the final Playback targeting that Group ID; the Group itself remains;
- deleting a referenced Group follows the explicit reference-integrity rule and cannot leave a
  hidden master;
- different Group IDs that are assigned as Group Masters retain independent levels and resolve
  overlapping eligible fixture intensity by HTP. Several Playbacks targeting the same Group ID are
  controls for one shared Group Master, not independent HTP contributors; and
- a Group with no Playback assignment has no master contribution and does not affect output merely
  because fixtures belong to it.

A Group Master Playback's explicit Select action may select its Group, but that later selection is
never assignment authority. Running, selecting, paging, or releasing one cannot change the result
of a future bare SET + Playback settings gesture.

## Dynamics evaluation semantics

### Target and inheritance rules

- A Dynamic whose target binding is one live Group resolves that Group at evaluation time and
  inherits its effective projection and shape by default.
- Group membership, nested-reference changes, mapping changes, and Stage-position changes flow
  into the next authoritative Dynamic evaluation without rewriting the Dynamic definition.
- A Dynamic override starts from the Group's resolved fixture membership. Replacing projection
  keeps the inherited shape; replacing shape keeps the inherited projection; replacing both uses
  both local stages.
- If the Group has no effective mapping, **inherit group mapping** resolves to source order. A
  spatial override must provide both missing stages before it can be applied. Random remains valid
  without a projection.
- Frozen targets and targetless/direct selections have no Group mapping to inherit. Their default
  is their stored/current explicit order. A spatial override must be complete; Random may be used
  directly.
- Changing the currently selected Group while editing an existing Dynamic does not retarget it or
  change its inheritance. The saved `target_binding` is authoritative. Retargeting is a separate,
  explicit edit.

Resolve the effective mapping once per Dynamic instance/evaluation snapshot, then feed its ranks
into the existing phase distribution (`offset`, `span`, anchors, block size, repeats, and wings).
Block and repeat logic counts spatial ranks, so parallel fixtures stay parallel. Per-lane phase
mode changes phase values but does not create a different target mapping unless a later plan
explicitly introduces per-lane mapping.

The current `PhaseOrdering::{Selection, GridLinear, RadialOut, RadialIn, Axial,
RandomEachLoop}` representation must migrate to the new separation. `GridLinear` maps to Grid,
`RadialOut`/`RadialIn` map to Radial, `Axial` maps to Radar, `RandomEachLoop` maps to Random, and
`Selection` maps to both stages inherited. Existing definitions without the new override must keep
their current observable phase order after migration; where an old Dynamic stored a spatial
ordering, encode it as a Dynamics-local override rather than silently switching it to the Group.

### Editing and preview

The preview must use the Dynamic's saved target binding plus the unsaved editor draft. It must not
use an unrelated current programmer selection except while creating a new targetless Dynamic from
that explicit selection. Draft edits are local until Apply; Close discards them. Apply is one
idempotent object-intent update and one show revision.

If live membership or Stage positions change while editing, refresh the preview against the draft,
show that the source changed, and retain the draft override fields. A stale revision on Apply must
reload the authoritative Dynamic and let the operator deliberately reapply the draft; it must not
silently overwrite concurrent changes.

## Operator UI

### Group settings

Replace the current Group context-menu management content with one Group settings modal. The modal
contains exactly three tabs, in this order:

1. **General** — editable Group name, icon, and color only.
2. **Projection** — the canonical Stage-1 3D-to-2D projection, including inheritance/local-
   override state, source provenance needed to understand inheritance, projection preset, anchor
   X/Y/Z, direction X/Y/Z, rotation/angle, a projected-position preview, and applicable warnings.
3. **Phaser** — the canonical Stage-2 2D-to-phaser/order mapping, including **Grid**, **Radial**, or
   **Radar**, only the selected shape's controls, ranked preview, coincident/parallel fixture
   identities, and applicable warnings.

The title bar has a simple **X** close control. There is no footer, Apply action, or management tab.
Each accepted field change is an immediate revisioned object-intent edit; X merely closes the
modal and does not perform another mutation.

Projection owns the mapping-level inheritance actions: **Use inherited mapping**, **Create local
mapping**, **Copy inherited values as local**, and **Remove local mapping**, as applicable. It shows
**Mapping: None**, **Inherited from Group N**, **Local override**, or **Mixed source mappings —
source order**. Choosing or removing local ownership applies to the complete canonical
projection-plus-Phaser mapping so the two tabs cannot silently have different Group inheritance
owners. **Use inherited mapping** removes only the Group-local mapping. It does not alter membership
or a source Group. **Copy inherited values as local** detaches both current stage configurations
while membership remains live.

Do not include a Master slider, Select live/frozen actions, Replace membership with selection,
Undo membership/programming, or other current context-menu management content in this modal. Plain
click selects the live Group; double quick press selects its current membership frozen; SET-click
and right-click open this same modal. Membership replacement remains the separate
`[REC]` → Group → **Override** workflow. Group Master assignment remains exclusively Playback-owned
through the explicit SET grammar above.

Do not add a Layout pane, Layout window, or general selection-layout screen. All modal controls need
touch-sized interaction, keyboard focus, exact units, and non-color-only state.

### Dynamics editor

Add a dedicated **Projection** tab alongside the existing **Phaser** tab. Projection owns target
ordering/mapping controls; Phaser retains phase offset/span/anchors, blocks, repeats, wings, and
uniform/per-lane phase behavior.

Projection begins in an explicit **Inherit group mapping** state for a live Group target. The tab
shows the inherited Group/provenance and preview. The operator can override **Projection**,
**Shape**, or both, and can return either stage to **Inherit** independently. Choosing **Random**
explains that fixture positions and the projection stage are ignored.

For direct/frozen/targetless targets, label the base state **Selection order (no Group mapping)**,
not “inherit group mapping”. Disable incomplete spatial overrides with an explanation. The editor
must never imply that changing a Dynamic updates Group settings.

## Persistence and backward compatibility

This plan does **not** declare a pre-v1 persisted-show break. Follow `docs/acceptance-criteria.md`.

### Show data

- Add optional/version-tolerant fields for canonical Group source/mapping and Dynamic override.
- Legacy Groups with `fixtures` and no `derived_from` decode as `Explicit` in their stored order.
- Legacy Groups with `derived_from` decode as one `References` entry with the existing rule.
- Preserve unknown fields through the active-show object boundary.
- Legacy `GroupDefinition.grid` is not promoted automatically to a canonical mapping because it
  was explicitly separate from Group order and could not affect `THRU`. Automatically promoting
  it would change show output. Preserve or consume it only for migration diagnostics, then default
  the new mapping to `None` and retain legacy ordered membership.
- Migrate old Dynamic phase ordering as specified above so output remains stable. Include embedded
  fallback definitions stored in Cues/Playbacks and any demo/benchmark fixtures.
- Empty Groups, unpatched fixtures, logical heads, and unknown future fields remain lossless.
- Remove `GroupDefinition.master` and legacy `playback_fader`/assigned-Playback pointers from live
  Group authority. Existing Playback Group targets remain authoritative; assignments for one Group
  ID migrate to one shared master level while retaining their local layout, behavior, and pickup.
- Before removing legacy fields, prove from representative shows whether an assigned Group Master
  level currently resides on the Playback, Group, or both. If several Playbacks referencing one
  Group contain different legacy levels, apply one documented deterministic precedence rule to
  choose the initial shared Group-ID level, preserve the original file for recovery, and report the
  reconciliation rather than retaining divergent live levels.
- A legacy Group master/pointer without an assigned Playback loads tolerantly but does not create a
  Group Master. Saving the migrated show writes Playback-owned state only.

### Desk layouts and retired Layout state

On desktop-layout load, remove obsolete Layout pane/window entries without failing the rest of the
layout. Preserve neighboring pane geometry as far as the window manager permits and show one
actionable migration notification: spatial ordering now lives in Group settings and Dynamics
Projection. Do not replace Layout with another pane by guesswork.

Retire the desk-local/global selection-grid state and its Shift grid/reorder gestures. Restore each
affected key to its otherwise documented behavior or leave the shifted combination unassigned;
never redirect it to edit a Group without an explicit Group-settings interaction. Remove obsolete
help and screenshots through the documented generation workflow when implementation occurs.

Migration failure must preserve the original file, allow application startup, report an actionable
error, and offer a separate empty show as required by the acceptance criteria. Verify both legacy
load and new-show initialization through real server startup.

## API, events, and ownership

Use the existing object-intent routes and bring every touched call site into compliance with
`docs/engineering/api-rules.md`:

- extend the typed Group update intent carried by `POST /api/v2/groups/manage` (or replace that
  operation with the compliant `POST .../{object}/{id}/update` form in the same chunk if the
  touched route still lacks object-intent semantics);
- extend `POST /api/v2/dynamics/{id}/update` with typed, field-scoped mapping-override intents;
- require request identity and show revision/object revision guards for edits;
- accept and server-log unknown fields without logging values;
- return clear 4xx validation paths for invalid vectors, non-finite numbers, cycles, missing
  references, or incomplete overrides; and
- publish authoritative Group/Dynamic object events after commit so all visible settings and
  previews reconcile without broad reloads.

Opening Group or Playback settings is navigation/read intent and never mutates the show. Group
Master assignment is one idempotent, revision-guarded object-intent update to the explicitly
addressed Playback. The server resolves the explicit Group and Playback; the client never submits a
mutated whole Group or selection-derived source. Assignment publishes the affected Playback event,
not a Group object event, because Group data did not change. All surfaces call the same application
commands for settings and assignment.

Reads return whole object snapshots. Stage-position, Group, and Dynamic changes invalidate only
the relevant capability/object/view subscriptions. Live programming actions, including `THRU`,
remain ordered WebSocket actions with their HTTP action equivalent; clients submit the selected
Group/value intent and never pre-expand ranks or fixture values.

Generated Rust/TypeScript declarations and JSON schemas are updated from their source definitions,
not hand-edited. The UI may calculate a non-authoritative draft preview for immediacy only if it is
reconciled against the server result and cannot drive output or persisted rank decisions.

## Validation

Reject an edit atomically when:

- anchor, direction, rotation, centre, angle, or any numeric shape field is non-finite;
- view direction is zero/effectively zero after normalization;
- a Group reference is absent, self-referential, or creates a direct or nested cycle;
- ordered source IDs/references contain invalid identifiers;
- a Dynamic spatial override lacks an effective projection or shape;
- Random is requested as a Group mapping; or
- the edited Dynamic target and override combination cannot resolve deterministically.

Also reject Group-Master assignment when its explicit Group/Playback/page identity is absent,
stale, invalid, or no longer belongs to the active show/desk context. Failure must not alter
selection, Group data, Playback configuration, output, or Undo history.

Normalize degrees to one documented half-open range, normalize vectors once, canonicalize negative
zero, and place explicit upper bounds on reference depth, reference count, and fixtures evaluated
per request consistent with the supported-scale plan. Validation must not mutate the current show,
runtime Dynamic, programmer values, or Undo history on failure.

## Edge cases

- An intentionally empty Group evaluates to zero ranks and remains selectable/storable.
- An absent Group is an error; a missing ID in a Group-number range is still skipped by range
  selection according to the existing command contract.
- Two or more fixtures with one projected coordinate share one rank and spread value.
- Different coordinates may share a Grid line, Radial radius, or Radar angle and therefore a rank.
- Coincident fixtures remain individually visible in the preview and retain deterministic source
  order within their shared rank.
- Fixtures without valid 3D positions remain last, visible, and individually ranked.
- Unpatched fixtures participate fully; only their DMX output is suppressed.
- Multipatches do not duplicate one logical selection item. Logical heads follow ordinary
  selectable-head semantics.
- Moving a fixture changes evaluated ranks but never rewrites Group source data or its mapping.
- Changing source membership updates all live descendants and live-bound Dynamics.
- Changing a source mapping updates inheriting descendants/Dynamics but not local overrides.
- A mixed multi-reference Group uses source order until it receives a local mapping.
- Removing a local Group mapping reveals the current inherited mapping, not a stale copied one.
- Removing one Dynamic override stage immediately reveals the current inherited stage.
- Random remains deterministic for its stored seed/loop input and independent of positions.
- Group-reference cycles, deeply nested graphs, stale revisions, and concurrent edits fail visibly
  without partial mutation.
- SET + Playback remains settings-only regardless of an existing Group, fixture, empty, or mixed
  selection.
- Clearing/reassigning one of several Playbacks for the same Group leaves its shared Group Master
  active. Clearing the final assignment removes that master contribution. Different assigned Group
  IDs continue to resolve overlaps independently by HTP.

## Verification plan

Start with focused pure-domain tests, then widen according to risk.

### Pure/domain tests

- projection basis and all named presets in the established XYZ convention;
- rotation, anchor translation, normalization, negative zero, and invalid vectors;
- Grid, inward/outward Radial, clockwise/counter-clockwise Radar, and deterministic Random;
- equal-coordinate/equal-key rank sharing and source-order tie-breaking;
- missing-position fallback and empty membership;
- single/nested/multiple Group references, duplicate removal, rules, mixed mappings, cycles, and
  local-override precedence;
- rank-aware two-point and multi-point spreads, including too many control points for rank count;
- Dynamic inheritance, each single-stage override, full override, Random, and group-less targets;
- block/repeat/wing phase behavior over shared ranks; and
- cache invalidation on source, mapping, and Stage-position revisions;
- typed SET-state transitions, cancellation/scope replacement, and the absence of selection-derived
  Playback assignment;
- live/frozen Group gestures and mutation-free settings opening; and
- shared same-Group Master control, different-Group overlapping HTP, final-assignment removal,
  reassignment, and explicit page addressing.

### Persistence/API tests

- representative legacy explicit, derived, frozen, empty, and gridded Groups load without output
  changes;
- every legacy Dynamic ordering and embedded fallback retains its prior evaluated phase order;
- new data round-trips with unknown fields preserved;
- malformed legacy data starts the app with recovery behavior and preserves the original file;
- legacy Group-owned master fields and Playback Group targets migrate without output change, while
  new shows store no Group-owned master authority;
- Group and Dynamic edit replay, stale revision, tolerant unknown field, event, and validation
  behavior;
- live Group spreads in Programmer, Preset, Cue, Preload, and playback re-resolve after membership,
  mapping, and Stage-position edits; and
- dereferenced/frozen values do not later redistribute.

### Operator acceptance

Add focused human-readable scenarios under `docs/testing/` and executable root Playwright coverage
for this named plan only:

1. Create a Group, set a top-down outward Radial mapping, select the Group, enter dimmer
   `0 THRU 100`, and prove centre fixtures receive 0, the outer rank receives 100, and coincident
   fixtures act in parallel through authoritative output.
2. Create a live derived Group, change source membership, and prove membership plus inherited
   mapping/order update. Add a local mapping, change source membership and mapping again, and prove
   only membership flows into the local result.
3. Bind a Dynamic to the Group, verify **Inherit group mapping**, override only projection, then
   only shape, then both, and prove Group settings never change.
4. Verify nested/mixed references, missing 3D positions, an intentionally empty Group, unpatched
   members, and direct selections with exact visible states.
5. Verify no Layout window/pane or hidden selection-grid control remains, migrated desktops still
   open, and the Group/Dynamics workflows are usable in software-only and hardware-connected desk
   layouts.
6. Verify the Group settings modal has only General, Projection, and Phaser tabs, closes with X,
   contains only name/icon/color under General, and contains none of the retired context-menu
   management actions.
7. Verify plain Group click selects live, double quick press selects frozen, command-line
   `SET GROUP 4 ENTER` requires Enter and opens settings, and direct SET-click/right-click open the
   same modal.
8. Verify direct SET + Group 4 + Playback 2 assigns without Enter, while bare SET + Playback 2 opens
   Playback Configuration despite Group 4 or loose fixtures already being selected.
9. Verify Record → Group → Override remains membership replacement, changing the Playback to None
   removes its master without changing the Group, and two overlapping masters produce the HTP
   maximum across level, Flash/release, page, restart, and reassignment boundaries.

Run focused package tests first, then the applicable repository gates:

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e -- tests/<focused-spatial-mapping-spec>.spec.ts
npm run test:e2e-ui
npm run manual
npm run open
curl -fsS http://127.0.0.1:5000/api/v2/readiness
```

Inspect `.artifacts/runtime/light-data/light-headless.log` for runtime failures. Storybook and
frontend builds are design evidence, not proof of authoritative mapping/output behavior. Complete
the plan only after real desktop verification, an implementation `## Result`, the finished move,
and a semantic commit.

## Acceptance criteria

- [ ] No built-in Layout pane/window or hidden global/per-selection layout authority exists.
- [ ] Group settings own an optional canonical projection-plus-shape mapping.
- [ ] Group settings contain only General, Projection, and Phaser tabs, close with X, and contain
      none of the retired Master/selection/membership/Undo context actions.
- [ ] The first stage supports a 3D anchor, view direction, rotation, and Top/Front/Back/Left/Right
      presets.
- [ ] The second stage supports Grid, Radial, and Radar; Random is position-independent and
      Dynamics-only.
- [ ] Exact projected-coordinate peers share rank/value and visibly act in parallel.
- [ ] Group-exact `THRU` and other selection-sensitive operations evaluate by rank on the server.
- [ ] Live Group references preserve membership updates and inherit mapping/order by default.
- [ ] Group-local mappings replace inherited mapping/order without freezing membership.
- [ ] Nested and multiple-reference precedence is deterministic, validated, and visible.
- [ ] Dynamics inherit a live-bound Group mapping by default and can override either/both stages
      without changing Group settings.
- [ ] The Dynamics editor has a dedicated Projection tab alongside Phaser with an explicit
      inheritance state.
- [ ] Editing an existing Dynamic uses its saved target binding, not an incidental current
      selection.
- [ ] Direct/ungrouped selections retain explicit selection order and create no hidden mapping.
- [ ] Plain click selects a live Group, double quick press selects it frozen, and neither gesture
      opens or mutates settings.
- [ ] SET + Group + Enter opens Group settings; SET + explicit Group + Playback assigns; SET +
      Playback alone always opens Playback Configuration regardless of selection.
- [ ] Direct SET-click and right-click open the same Group settings modal, and direct Playback
      targeting completes assignment without Enter.
- [ ] Membership replacement remains Record → Group → Override, not a settings action.
- [ ] Group Masters exist only through Playback assignments; Group settings/data own no master, and
      Playbacks for one Group share one master while different assigned Groups remain independent
      HTP contributors.
- [ ] Legacy Groups, Dynamics, embedded fallbacks, desktops, and malformed-show recovery meet the
      compatibility contract.
- [ ] Software, keyboard, OSC, HTTP/WebSocket, and attached-hardware paths share authoritative
      semantics where those surfaces expose the operation.
- [ ] Focused domain, persistence, API, UI, migration, and real desktop/output acceptance evidence
      is recorded in `## Result` before completion.

## Explicit non-goals

- A built-in Layout screen, Layout pane, paperwork canvas, or general 2D fixture editor.
- Manual dragging of fixtures into arbitrary persisted 2D cells.
- A global, desk-local, or per-selection spatial layout.
- Changing Stage 3D position authoring or fixture mounting/parent transforms.
- Random Group mappings or position-dependent Random behavior.
- Per-lane Dynamics projection/mapping overrides.
- Automatically writing Group settings from the Dynamics editor.
- Freezing live Group membership merely because a local mapping exists.
- A Group-owned Group Master, master slider, or assigned-Playback property.
- Inferring Playback assignment source from selection, the last Group, Highlight, or current
  Playback contents.
- Redesigning non-Group Playback functions or changing HTP arbitration beyond sharing one master
  per Group ID and retaining independent overlapping masters for different Group IDs.
- Timing-offset authoring; the model may support it later, but this plan does not expose it.
- Renderer/paperwork-app layout, output visualization, or a replacement for the Stage view.
- Broad changes to Group record/merge/subtract semantics beyond the source/mapping rules stated
  here.
