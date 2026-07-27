# Dynamics Window

## Status and goal

**DOING.** Build the numbered Dynamics pool and the full production editor, using the standalone experiment as the visual and encoder baseline while applying the settled behavior below.

The production editor deliberately has no embedded fixture grid, fixture preview, preview transport, or browser-side Dynamic evaluator. Operators open a separate Stage pane when they want visualization. A Stage pane may follow Live or Preload through the real authoritative runtime.

## Pool behavior

The Dynamics pane/window opens in Pool view and uses the accepted shared pool/window primitives:

- integer slots 1–9999 in the same scrollable 88-pixel shared `PoolGrid`/`PoolCard` geometry as Presets and Groups, with stable UUID identity, name, presentation color, target summary, validation state, and running-instance count;
- no page selector, page legend, or Dynamics-only pool grid;
- default Dynamic pool color cyan/light blue, with the shared object-type and per-item color rules;
- empty slots remain visible and distinct from invalid or deleted objects;
- moving/renumbering preserves UUID identity; copying creates a new UUID; deleting follows the snapshot rule in the runtime plan;
- no implicit merge of selected Dynamics; reuse is through Duplicate, Copy Lane, and Add Lane; and
- no local-only selection or running state.

Exact tile actions are:

- ordinary tap/click on an empty tile opens the standard desk modal first; the first lane is chosen inside that modal and the Dynamic is created only after the first valid lane is selected;
- ordinary tap/click on a populated tile toggles the matching Programmer Dynamic instance for the resolved target scope;
- right-click, touch hold, or Shift-tap/Shift-click on a populated tile opens its editor and suppresses the browser context menu;
- an armed `DELETE` followed by a populated Dynamic tile deletes that Dynamic and clears the command only after the revisioned delete succeeds;
- software uses latched Shift and attached hardware uses held Shift;
- `[SET]` followed by a Dynamic tile and a Playback assigns that Dynamic to the Playback, so Set is not an edit gesture; and
- armed Record/Store/Update modes keep their documented precedence instead of accidentally starting an instance.

For a populated tile, the toggle key is Dynamic UUID plus resolved target scope:

- when no matching Programmer instance exists, the tap starts one;
- when a matching Programmer instance exists, the tap applies Dynamic Off to that instance;
- a target-bound Dynamic resolves its stored scope;
- a targetless Dynamic uses the current ordered selection, or all compatible fixture/head targets when the selection is empty; and
- multiple independent instances of one targetless Dynamic may still exist when they come from different target scopes, Cues, Playbacks, or users.

## Pool-to-editor navigation

The editor replaces Pool view inside the same Dynamics pane/window. The title bar contains **Back to Pool** and the standard window controls. Returning to the pool does not stop output or discard edits.

There is no draft Save/Discard model after creation. Every accepted editor operation is an immediate server-authoritative object mutation:

- discrete actions create discrete revisions and undo entries where the current show-object workflow supports undo;
- a held/continuous encoder gesture uses one server-owned mutation/undo group and a bounded update cadence;
- every accepted mutation commits its small SQLite transaction before acknowledgement;
- a stale revision causes the client to re-read and deliberately reapply the operator action;
- an invalid edit is rejected without changing the last valid stored or running revision; and
- a valid revision hot-swaps running instances at the next sample boundary while preserving phase, source ownership, playback-local controls, and lifecycle.

The editor shows actionable validation beside the affected control and on the pool tile. It never leaves a browser-only value that disagrees with the server.

## Target binding

A Dynamic has exactly one binding class:

1. **Live Group** — a stable Group UUID whose current ordered membership is resolved when the singleton instance starts.
2. **Frozen targets** — an ordered list of fixture/head identities captured from the authoritative selection.
3. **Targetless** — a reusable template resolved per instance from the current selection or, with no selection, every compatible fixture/head.

Both Live Group and Frozen targets are target-bound and permit only one runtime instance of that Dynamic. Targetless Dynamics permit independent instances.

The editor provides:

- **Take Selection** — stores one selected Group as a live Group reference; otherwise stores the exact ordered fixture/head selection;
- **Clear Selection** — changes the definition to Targetless; and
- a target summary showing Group identity or ordered target count, missing targets, unpatched targets, and compatible-lane coverage.

Take Selection and Clear Selection are blocked while any instance of that Dynamic is running. The operator must turn every instance Off first. A target-bound Dynamic cannot be retargeted by a pool tap, command, Cue, or Playback. An explicit command selection must resolve exactly to the stored scope or fail without mutation.

Live Group membership and order are resolved at instance start. A running singleton retains its resolved target projection until restart/retrigger. Unpatched targets remain present and evaluate normally; only physical DMX output is suppressed.

For a targetless Dynamic:

- a target is compatible when it exposes at least one lane attribute;
- supported lanes run for that target;
- unsupported lanes are skipped for that target without blocking supported lanes; and
- the result reports a concise actionable warning and exact skipped target/lane counts.

## Editor structure

The standalone `experiments/dynamics-editor` is the full-view baseline, except that production removes its mock fixture grid and preview playhead. While the editor is open, the existing Programmer control section replaces its ordinary Intensity/Color/Position family row with three Dynamic editor tasks:

1. **Curves** — scalar lane mode, sources, functions, keyframes, interpolation, Size, Width, and per-lane speed.
2. **Phase Spread** — one shared target projection, phase expression, offset/span, Blocks, Repeats, Wings, spatial center, and ordering.
3. **Speed** — fixed duration or Speed Group, beats per cycle, overall multiplier, activation policy, quantization, and transport status.

The editor uses the normal six-slot ToskLight encoder surface in the existing Programmer control section. The Dynamics pane never renders a second encoder row. Slots never shift when a control is unavailable; an unsupported slot remains visible, numbered, and disabled. Software encoder gestures remain relative, the center **Set Value** path remains explicit absolute entry, and hardware/software fine/coarse semantics use the shared encoder contract.

Vertically stacked lanes support one primary lane and additive multi-selection:

- ordinary lane tap selects it as the sole primary lane;
- Shift-tap adds/removes a lane while retaining at least one selected lane and one primary lane;
- shape, speed multiplier, width/scale, and other explicitly shared edits affect all selected lanes;
- keyframe and scalar-source edits affect only the primary lane;
- Add Lane, Duplicate Lane, Delete Lane, and reorder actions are touch reachable and do not depend on hover; and
- a lane always addresses one canonical continuous-scalar attribute.

## Lane schema and modes

The attribute registry supplies stable identity, family, display label, units, normalized/domain bounds, fixture-facing mappings, and continuous/discrete capability. Initial Dynamics lanes support continuous scalar attributes across Intensity, Position, Color components, Beam, Focus, Zoom, Iris, and compatible custom continuous attributes. Indexed/discrete functions, fixture control actions, compound values, and raw DMX values are rejected.

Every lane preserves all three configurations while the operator switches modes:

### Keyframes

- Sources are **Current**, **Value**, or a live matching scalar value from an ordinary Preset.
- The first keyframe at 0% is also the loop-closing value shown at 100%; the terminal point is an alias and cannot diverge.
- Selecting sources A and B initially creates A at 0%, B at 50%, and closing A at 100%.
- Inserted keyframes have explicit normalized cycle positions and remain lane-local.
- Segment interpolation choices are Linear, Ease in, Ease out, Ease in + out, Hold, and Drop.
- The default interpolation is Ease in + out.
- Keyframe Scale changes points 1 through N-1 while the closing point remains at 100%; fine/coarse/reset behavior follows the experiment.
- Playback Size scales every keyframe deviation around the first/closing keyframe.

### Max/min function

- Top and Bottom independently use Current, Value, or a matching scalar Preset value.
- Functions are Sinus, Cosinus, Linear +, Linear -, and PWM.
- Playback Size scales the Top/Bottom interval around its midpoint.

### Middle/amplitude function

- Middle uses Current, Value, or a matching scalar Preset value.
- Amplitude uses the attribute's scalar display/domain unit and clamps only at the final supported attribute bounds.
- Functions are Sinus, Cosinus, Linear +, Linear -, and PWM.
- Playback Size multiplies Amplitude around Middle.

Switching modes never converts or discards inactive settings. Only the selected mode evaluates.

## PWM

PWM uses Minimum/Maximum or Middle/Amplitude values from its selected mode and four normalized cycle portions:

- Attack rises to the high value;
- On holds the high value;
- Decay returns to the low value; and
- Off holds the low value.

On and Off define the cycle partition; Attack is contained within On and Decay within Off. Editing either slope never moves the On/Off boundary or cycle end. Zero Attack and Decay produce a hard pulse. Each slope uses its chosen scalar interpolation.

## Initial Random function

Initial Random is a deterministic Gaussian pulse function, not the full experimental Random catalog.

Each Dynamic owns local Random groups. A lane links to one group; lanes linked to the same group receive the same normalized per-target event/envelope stream and map it through their own Minimum/Maximum or Preset-derived scalar values. This synchronizes multi-component Color and other multi-lane Random content without creating combined attribute values.

Each target makes an independent seeded event decision. Each independent Dynamic instance derives an independent stream; all linked lanes inside that instance remain correlated.

Random controls are:

- low and high scalar sources: Current, Value, or matching Preset;
- decision interval in milliseconds;
- start probability evaluated for an off target at each decision boundary;
- Gaussian mean pulse duration in milliseconds;
- Gaussian pulse-duration spread in milliseconds;
- Attack and Decay as percentages of each sampled pulse duration, constrained so their sum is at most 100%;
- the remaining duration as high-value hold;
- a local Random group selector; and
- Generate Seed for the selected local group.

Pulse durations are deterministically drawn, bounded to at least one evaluator/output interval, and may cross a Dynamic cycle boundary. Decision interval and pulse durations scale inversely with overall Dynamic speed, lane multiplier, and playback-local Double/Half/learned speed. Zero Attack and Decay are hard binary pulses. Random timing, Markov stay-on/stay-off controls, density/grouping/burst modes, and separate timing/gate modes remain later extensions. Macro functions are not a Dynamics extension: [Macros](../../../Later/46-macros-and-scheduled-macros.md) may edit and start Dynamics, but Dynamics never call Macros.

## Phase Spread

One phase projection is shared by every lane in an instance. It assigns phase only; it never changes target membership or stored selection order.

Available orderings are:

- authoritative selection/Group order;
- Grid linear using Stage X/Z positions and a direction angle;
- Radial out;
- Radial in;
- Axial/Radar around a center; and
- Random each loop, usable with every lane shape.

Grid-linear angles are 0° left-to-right, 90° top-to-bottom, 180° right-to-left, and 270° bottom-to-top; intermediate values are diagonal. Fine/coarse encoder steps are 5° and 45°, and Push resets to 90°.

Radial and Axial/Radar default to the current target-position centroid and store an editable Stage X/Z center. Positioned targets are projected first. Targets without usable Stage positions are appended in stored order and produce a warning. A running instance captures its phase map at start; Stage movement affects only a later restart/retrigger.

Phase assignment uses this pipeline:

1. Resolve ordering and collapse exact spatial ties into common phase ranks.
2. Apply Block Size to consecutive ranks; a short final block is valid.
3. Split ranks into contiguous Repeats as evenly as possible; earlier repeats receive one extra rank when uneven.
4. Apply Wings inside each repeat by calculating its first half and mirroring it; an odd repeat has one center peak.
5. Spread each repeat endpoint-exclusively across Phase Span and then add Phase Offset.

Automatic Phase Span is cyclic and endpoint-exclusive for every span. Thus 360° across four ranks is 0°, 90°, 180°, 270°; 720° produces two cycles. The experiment's 180°, 360°, and 720° presets remain. Tightening curve keyframes creates a narrower single band; increasing span creates additional wavefronts.

Explicit phase entry accepts scalar degrees and `THRU` expressions. The existing deterministic multi-point spread resolver places explicit anchors. `0 THRU 360` over four ranks remains endpoint-exclusive at 0°, 90°, 180°, 270°. `0 THRU 360 THRU 0` over eight ranks yields 0°, 90°, 180°, 270°, 270°, 180°, 90°, 0°. Blocks, Repeats, and Wings then operate through the pipeline above.

Random each loop derives a deterministic per-instance permutation from the Dynamic seed, instance identity, loop index, and stable target identity before phase assignment.

## Speed view

A Dynamic uses exactly one speed source:

- fixed cycle duration; or
- one authoritative Speed Group A-E.

Speed-Group Dynamics store a positive rational beats-per-cycle value with a four-beat default. The Dynamic has an overall rational multiplier and every lane may have its own rational multiplier; required direct choices include multiply/divide by 2, 3, and 4. All lanes share one epoch even when rational multipliers make their cycle counts differ.

Run Mode is independent from activation timing:

- **Loop** repeats until its exact controller or instance is turned Off; and
- **One-shot** evaluates one complete effective Dynamic cycle, then stops and disappears from operator-visible running state. Its internal source ownership remains terminal until that authored activation is removed or deliberately retriggered.

One-shot completion is remembered for the authored activation identity so a still-active Programmer, Cue, or Playback value cannot restart it on the next output tick. A later deliberate activation may trigger it again. Older stored Dynamics omit Run Mode and therefore migrate as Loop.

The three activation policies are:

- **Start now** — local epoch, immediate phase zero;
- **Join sync now** — immediate activation at the current authoritative Speed Group position; and
- **Next boundary** — pending until the selected next beat/bar boundary, then phase zero.

Join/Next require a Speed Group. Fixed duration uses Start now. Fixture phase remains independent from transport position and activation quantization.

The Speed view shows authoritative source, effective duration/BPM, paused/running/pending state, Run Mode, activation policy, boundary, beats per cycle, overall multiplier, and bounded transport position. Run Mode is editable from both the form and the sixth encoder. It has no draggable or editable preview playhead.

## UI review and acceptance

Implementation first produces the complete working backend and frontend. The user then reviews and iterates the real pool/editor UI. Before that approval:

- use Storybook and packaged desktop/manual inspection for development;
- do not add new Dynamics pool/editor component tests, Playwright interaction tests, pixel snapshots, or help screenshots; and
- do not treat the experiment's mock geometry as a production acceptance oracle.

After approval, add focused tests for empty/populated/Shift/Set tile gestures, navigation, lane selection, immediate edits, target controls, encoder mappings, validation, touch and hardware modes, and dirty-free window behavior. Update help/manual screenshots only from the accepted production state.

### Review iteration 1 — rejected and incorporated

The first Storybook review was rejected on 2026-07-27. The required corrections are now part of this plan and the production implementation:

- Dynamics and Cuelists use the exact shared Preset/Group pool geometry;
- Dynamics pagination and implementation-language legend are removed;
- right-click opens edit, and armed Delete followed by a Dynamic tile deletes it;
- creation starts with the standard modal, with first-lane selection inside it;
- the pane-local encoder deck is removed and the existing Programmer section switches to Dynamic editor tasks and encoders;
- Dynamic name, icon, and color use desk-native form controls in the right inspector;
- the workspace is darker and shows all ordered lanes at once.

This iteration is not visual acceptance. The revised full-application Storybook story must be reviewed again before final UI automation or screenshots are added.

### Review iteration 2 — Curves hierarchy rebuilt, awaiting review

The second Storybook review on 2026-07-27 narrowed the visual checkpoint to the
Curves task. The production editor and the full-application story now share this
composition:

- Curves, Phase Spread, Speed, Add Lane, Back to Pool, and Settings are window-title actions.
- The full-application story shows only that authoritative Dynamic title bar; its
  action order is Add Lane, task tabs, Back to Pool, then Settings.
- Window Settings owns Dynamic name, icon, and color.
- Curves shows every lane as a compact horizontal attribute/mode/curve row.
- Each lane has the desk checkbox for multi-edit selection and a desk action menu
  for attribute, order, duplication, and deletion.
- The active lane has one shared Keyframes / Max-min / Middle-amplitude / Random
  editor below the lane rows.
- The Programmer owns the same task switch, a desk lane menu, a visible encoder
  page indicator, and the six production Dynamic encoders without a duplicate
  encoder-section heading.

This remains a visual-review checkpoint, not acceptance. Do not add the final
Dynamics UI automation or refresh the acceptance screenshot until the user
accepts this Curves composition.

### Review iteration 3 — Curves composer and Programmer row, awaiting review

The next Curves-only review moved target selection into Window Settings and
removed the object-management footer. The lane editor is now the **Curve
Composer**: Key-frames, Max/min, and Middle/amplitude are two-line method
buttons at the left; keyframe cards or supported function buttons occupy the
middle; and Add keyframe appears at the right only for Key-frames. Random is
presented as a supported function for both value methods rather than as a
fourth method.

The Programmer task row now owns the lane selector at its right edge. Encoder
page position belongs in the active task label, using `Curves (1/2)` only when
the task has more than one page; no `1/1` label is shown. This remains pending
visual acceptance.

### Review iteration 4 — lane selection and attribute flow, awaiting review

Lane rows are now the selection surface: click selects one lane and Shift-click
(including the desk Shift latch) adds or removes lanes from the multi-edit set.
The redundant checkbox is removed. Each lane has one regular dropdown containing
only **Change attribute** and **Delete lane**. Change attribute opens the standard
desk modal and applies the selected continuous scalar attribute only after
confirmation.

### Review iteration 5 — flat composer and full-height encoders, awaiting review

The Curve Composer is now a full-width strip at the bottom of the Curves
workspace instead of a separate pane. It contains only the curve method,
function, and keyframe structure; Top, Bottom, Middle, Amplitude, and the other
numeric curve values live exclusively on the Programmer encoders.

The Curves encoder mapping follows the experiment's mode-specific ordering. The
touch encoder section has no extra divider or inset deck chrome, and its six
encoders occupy the complete standard Programmer parameter-surface height.

### Review iteration 6 — keyframe timeline, Speed, and Phase Spread, awaiting review

The Curve Composer now uses the same visual hierarchy as a window title bar at
the bottom of the Curves workspace. Keyframes, Max/min, and Middle/amplitude
form one single-line toggle followed by one desk dropdown for the selected
keyframe or supported function. Keyframe lanes show the real compact timeline:
the first point is **A** at 0%, the implicit closing point is **A′** at 100%,
and interior points can be selected and moved in the lane or edited with the
shared Programmer encoders. Function curves remain narrow and repeat across the
full lane width rather than being stretched into one oversized wave.

The lane action is the shared desk dropdown with only **Change attribute** and
**Delete lane**. The Speed task now presents its source and live beat grid on
the left, with fixed BPM or Speed Group configuration, and desk-native
multiplier, run-mode, activation, and boundary controls on the right. The Phase
Spread task uses desk-native controls, adds a two-dimensional fixture-position
preview, and exposes **Take Selection** and **Clear Selection** without removing
them from Window Settings. The full-application story shares the editor's
selected keyframe with the production Programmer encoder surface.

This checkpoint was superseded by the accepted seventh review iteration.

### Review iteration 7 — reusable encoder pages and transport preview, accepted

Encoder group navigation is now a shared UI component rather than
Dynamics-only tab behavior. A group declares its page count and encoder models;
selecting another group opens page one, while selecting the active group cycles
and wraps its pages. The production Dynamics Programmer uses this contract in
both touch and hardware modes. Curves demonstrates two real pages: keyframe
editing on page one and method, function, curve width, lane speed, keyframe
count, and loop closure on page two. The lane selector is the portal-based desk
dropdown and opens upward above the complete command surface.

Curve previews normalize their visible interval to the slowest lane. Faster
lanes repeat proportionally; the first interval remains cyan, its repeat
boundary is orange, and later intervals are grey without duplicate keyframe
handles. Keyframes are fixed-size round orange handles even when their SVG
curve is stretched. The Curve Composer remains pinned to the bottom of the
workspace. Window Preview/Stop controls a synchronized green playhead across
every lane.

Phase Spread keeps its left visualization at the available height while its
right control column scrolls independently. Fixed BPM now includes a large Tap
Tempo target. Run mode, Activation, and Boundary include short operator-facing
descriptions. The full-application story follows Storybook's software/hardware
toolbar mode and renders the corresponding production encoder surface.

The user completed the iterative review and directed implementation to continue.

## Result

### Changes

- The production Dynamics pool uses the shared pool component and supports normal, Shift, context
  edit, Set, and Delete-mode gestures.
- The editor uses one title bar with Add Lane, Curves/Phase Spread/Speed, Preview/Stop, Settings,
  and Back to Pool in the reviewed order. Settings owns name, icon, color, and target selection.
- Curves renders compact horizontal lanes, shared dropdown actions, attribute modal, round orange
  keyframes, repeat boundaries, playhead, pinned composer, and shared paged Programmer encoders.
- Phase Spread and Speed use desk-native controls, visualization, scrolling, beat grid, Tap Tempo,
  explanatory copy, Loop/One-shot, and software/hardware encoder surfaces.

### Verification

- User visual review completed against the production full-application Storybook story.
- Ten focused desktop tests cover pool/editor/Delete, lane and Shift selection, Change Attribute,
  encoder paging, and software/hardware encoder rendering.
- Desktop typecheck, targeted help screenshot verification, marketing screenshot verification, and
  the PDF/HTML manual build passed.

### Limitations

- The story remains a discussion fixture, but it renders exported production components and the
  same application composition rather than a Storybook-only lookalike.

### Commit

`test(dynamics): automate reviewed editor interactions` plus the preceding reviewed Dynamics UI
commits.
