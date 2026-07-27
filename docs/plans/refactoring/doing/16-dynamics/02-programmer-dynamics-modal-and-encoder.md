# Programmer Dynamics Modal and Encoder

## Status and goal

**DOING.** Add Dynamic On, Dynamic Off, and Fixed At as first-class Programmer values, expose the Dynamics encoder surface, and keep UI, command line, HTTP/WebSocket, OSC, and attached hardware on one server-authoritative operation path.

The full object editor lives in the Dynamics window. The Programmer Dynamics surface is the fast live-programming and running-instance workflow.

## Programmer value model

Programmer content has independent per-address layers:

- the ordinary static value and timing track;
- zero or more Dynamic On values linked to Dynamic instance identities;
- Dynamic Off tombstones targeting one exact instance; and
- `FixAT` values that compete with Dynamics on the addressed attribute.

One Dynamic On value exists per fixture/head-and-attribute address, but every lane created by one start operation carries the same Dynamic instance identity. This lets independent scalar values start, pause, transition, record, and stop together without forming a combined multi-attribute value.

Ordinary static `AT` changes the continuously resolved base under a Dynamic. It never stops or restarts a Dynamic. For example:

- Cue/Programmer static Intensity 50%;
- start a Current-centered Dynamic;
- later set static Intensity 70%;
- the running Dynamic follows 70% immediately without restarting.

To end a specific Dynamic, use Dynamic Off. To force a fixed winning value over every matching Dynamic, use `FixAT`.

## Pool use and targeting

Ordinary tap on a populated pool tile and command `DYNAMIC <number>` use the same toggle action.

Starting resolves targets as follows:

- Live Group or Frozen-target Dynamics always use their stored target scope.
- An explicit command selection for a target-bound Dynamic must resolve exactly to the stored scope or the operation is rejected.
- A targetless Dynamic uses the current ordered selection.
- With no selection, a targetless Dynamic uses every fixture/head that supports at least one lane attribute.
- Per-target unsupported lanes are skipped with an actionable warning; the instance starts when at least one address is valid.

Toggling resolves Dynamic UUID plus target scope against the current Programmer:

- no matching instance starts a Dynamic instance;
- a matching instance creates Dynamic Off for that instance; and
- the operation never affects the same Dynamic running from another Cue, Playback, user, or target scope.

Starting a target-bound Dynamic reuses its singleton runtime. Starting a targetless Dynamic creates an independent instance unless the same Programmer already has the matching UUID-and-scope instance, in which case the toggle turns it Off.

When no Programmer selection is active, a populated Dynamic tile no longer performs the generic “first tap selects stored targets” behavior from the earlier empty-selection plan. It follows the settled Dynamics-specific rule above: bound content toggles on its stored targets and targetless content toggles on all compatible targets. The pool tile's target summary makes that scope visible before activation.

## Dynamics encoder

The Dynamics family in the Programmer encoder view opens a focused instance/lane surface rather than the legacy `dynamic.speed` fader.

It provides:

- running and Programmer-staged Dynamic instances affecting the current ordered selection;
- selected Dynamic name/number, instance source, target scope, active/hidden/paused/pending state, and current winning/losing status;
- lane selection by canonical attribute;
- relative controls for mode-specific values, overall/lane speed, Size, Width, phase offset/span, Blocks, Repeats, Wings, and spatial direction/center;
- a primary **Off** action for the selected Dynamic instance;
- **Take Selection** and **Clear Selection** only when editing the definition and every instance is Off;
- the explicit center **Set Value** path for absolute parameter/spread entry; and
- stable six-slot software/hardware mappings from the accepted editor.

Dynamic Off is instance-wide. It targets one selected instance and the server atomically expands it across every linked active lane and target. It does not stop another instance of the same Dynamic and does not manufacture static zero values.

Every encoder edit to a Dynamic definition is an immediate typed object-intent mutation. Every live instance Size/speed/phase override is a live-control action scoped to that instance. The frontend must not blur these two operations.

Encoder-originated definition and live-instance parameter changes use the shared relative encoder semantics and explicit Set Value entry. They do not acquire Programmer Fade timing merely because an encoder changed them. Programmer Fade applies to starting/releasing Dynamic influence, not to editing the definition.

## Fixed At

The command line displays **`FixAT`**. Operator buttons and help text label the action **FAT**.

The exact shortcut is:

- software/touch: latched `[SHIFT] [AT]`;
- attached hardware: hold `[SHIFT]` while pressing `[AT]`.

The shortcut must not also emit an ordinary AT action.

`FixAT` stores an ordinary scalar value plus Fixed At semantics:

- with no Dynamic on the address, it behaves and records like the equivalent ordinary AT value;
- with Dynamics present, it participates in the Dynamic/FAT winner stack;
- source priority resolves first;
- among equal-priority Dynamic and FAT values, LTP activation time resolves the winner;
- a later FAT hides earlier Dynamics;
- a later Dynamic can retake control from FAT;
- FAT affects only the addressed attribute, not every lane of the Dynamic;
- losing Dynamics continue their clocks, including other lanes that remain visible; and
- releasing a temporary flashed FAT reveals the current phase of the winning underlying Dynamic.

FAT never pauses a Dynamic. Literal Pause freezes output and phase; FAT hides and continues.

## Activation and release fade

Dynamic influence has an activation mix independent from lane Size and Playback Master.

For one address:

- `B(t)` is the live ordinary static base;
- `D(t)` is the currently evaluated Dynamic value at its configured Size; and
- `w(t)` is the activation/release progress from 0 to 1.

The visible pre-master result is `B(t) × (1 - w(t)) + D(t) × w(t)`.

Starting through the Programmer uses Programmer Fade for `w(t)`. The Dynamic clock starts immediately; its visible influence grows without jumping to phase zero output. Releasing performs the inverse blend toward the newly exposed underlying winner. Cue/per-value timing owns the same mix when recorded in a Cue. Size and Master remain separate performance controls.

At Dynamic priority resolution:

- ordinary static Intensity sources first resolve their base through normal HTP;
- the winning Dynamic may produce final pre-master Intensity below that base, so dimmer waves and strobes are not clipped;
- ordinary static values remain alive underneath as `Current`; and
- Group Masters, the owning playback's selected Master behavior, Grand Master, Blackout, fixture projection, and output safety apply in their documented later stages.

## Recording, Update, Clear, and undo

Recording captures:

- ordinary static Programmer values;
- Dynamic On entries per target/attribute with their shared instance identity and Dynamic reference/snapshot;
- Dynamic Off tombstones for exact instances;
- FAT values; and
- the activation transition timing that was deliberately authored.

Dynamic On/Off/FAT values track independently from the ordinary static value. Record/Store, Merge, Overwrite, Update, Cue-only, and Preload use the server's atomic recording boundary; clients never expand linked lanes locally.

Update changes the selected Cue's instance reference, targets, instance overrides, On/Off/FAT state, or transition timing. Editing a referenced pool Dynamic edits the shared object and is not disguised as Cue Update.

Clear removes the addressed Programmer layer according to the existing Clear scope:

- clearing a Dynamic On Programmer value releases that Programmer instance with its transition;
- clearing an unrecorded Dynamic Off removes the tombstone and may reveal/restart the prior tracked instance;
- clearing FAT removes that source and reveals the current winning Dynamic/base; and
- no Clear action deletes or edits the pool Dynamic.

Starting, Off, FAT, instance-parameter changes, and definition edits are undoable at their owning boundary. One discrete gesture is one undo step; a held encoder gesture is one server-owned undo group. Navigation or merely opening/editing views is not undoable.

## Command-line grammar

The parser is case-insensitive; the visible command line uses the exact operator text below.

### Dynamic instance operations

- `DYNAMIC 12` — start/toggle Dynamic 12 for its resolved scope.
- `FIXTURE 1 THRU 10 DYNAMIC 12` — start/toggle targetless Dynamic 12 on that ordered selection; reject a target-bound Dynamic unless the selection exactly matches its stored scope.
- `DYNAMIC 12 OFF` — apply Off to the matching selected/Programmer instance.
- `DYNAMIC 12 SIZE AT 50` — set the matching live instance Size to 50%.
- `DYNAMIC 12 SPEED AT 2` — set a 2× instance speed multiplier; division accepts the shared ratio syntax.
- `DYNAMIC 12 PHASE AT 90` and `DYNAMIC 12 PHASE AT 0 THRU 360` — set scalar or distributed phase.
- `DYNAMIC 12 BLOCKS AT 4`, `REPEATS AT 2`, and `WINGS AT ON|OFF` — change the named phase helper.
- `SET DYNAMIC 12 PLAYBACK 5` — assign Dynamic 12 to explicit Playback 5; current-page shorthand follows existing Playback addressing.

When more than one active targetless instance of the same Dynamic could match an Off or parameter command, the command line enters a typed instance-choice state rather than choosing by array order.

### Fixed At

- with a selection and active parameter context, `[SHIFT] [AT] 100 [ENTER]` displays and executes `FixAT 100`;
- explicit fixture/group and parameter syntax follows the ordinary AT grammar, replacing only the `AT` token with `FixAT`;
- explicit TIME and DELAY apply to FAT as ordinary authored timing;
- untimed encoder Set Value remains immediate and does not acquire Programmer Fade; and
- malformed, unsupported, or discrete attributes fail visibly without partial mutation.

## HTTP, WebSocket, OSC, and hardware

The desk UI sends live instance operations through the established ordered WebSocket. Matching HTTP actions exist without show IDs in the route:

- `POST /api/v2/dynamics/{dynamic_id}/start` with optional target expression and instance overrides;
- `POST /api/v2/dynamic-instances/{instance_id}/off`;
- `POST /api/v2/dynamic-instances/{instance_id}/size`;
- `POST /api/v2/dynamic-instances/{instance_id}/speed`;
- `POST /api/v2/dynamic-instances/{instance_id}/phase`; and
- `POST /api/v2/programmer/values/fix-at`.

The optional `X-Tosk-Show` and `X-Tosk-Desk` headers follow the API rules. Object edits use `POST /api/v2/dynamics/{dynamic_id}/update` with request identity, expected revision, and typed intent fields. Additional unknown fields are tolerated and logged without values.

OSC exposes the same supported live actions and feedback with stable IDs:

- `/dynamic/<pool-number>/toggle`
- `/dynamic/<pool-number>/off`
- `/dynamic/instance/<uuid>/size`
- `/dynamic/instance/<uuid>/speed`
- `/dynamic/instance/<uuid>/phase`
- `/programmer/fix-at`

Exact OSC arguments, error feedback, and subscription snapshots must be added to the protocol help with the implementation. OSC does not edit full definitions.

Software, keyboard-generated command actions, OSC, HTTP, and attached-hardware encoders/buttons call the same application services. No surface computes target fan-out, phase values, priority, or linked-lane Off locally.

## Fixture Sheet, Stage, Cue, and running feedback

Fixture Sheet shows, per fixture/head-and-attribute:

- the winning ordinary static base and its source;
- ordered Dynamic/FAT stack entries by priority and LTP time;
- Dynamic name/number or embedded-snapshot label;
- instance identity/source, lane, Size, activation mix, paused/hidden/pending/winning state;
- the sampled/resolved value at bounded feedback cadence; and
- Dynamic Off when it is relevant tracked content.

Multi-lane values show a shared instance/link marker without combining attribute rows.

Every Cue row shows a small Dynamics icon when its tracked delta contains Dynamic On, Dynamic Off, or FAT content. The icon is informational and does not imply every tracked Dynamic is currently winning.

Stage shows only authoritative Live or Preload resolved output. The Dynamics editor contains no preview. Running-source feedback lists exact Dynamic instances and allows Off for one instance without releasing an unrelated Playback or Cuelist.

## Acceptance coverage

Backend/API coverage written with implementation proves target resolution, toggling, independent instances, singleton reuse, Dynamic Off, FAT, priority, Current, activation/release fades, recording, tracking, Cue-only, Update, Clear, undo grouping, command grammar, HTTP/WS/OSC mapping, and authoritative Fixture Sheet/Stage projections.

After UI acceptance, add exact operator coverage for empty/populated/Shift/Set tile gestures, encoder instance choice, Off, FAT shortcut separation, six-slot mapping, software/hardware parity, Cue icon, Fixture Sheet stack, and Stage Live/Preload behavior.
