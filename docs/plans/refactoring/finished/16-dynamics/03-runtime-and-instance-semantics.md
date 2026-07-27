# Runtime and Instance Semantics

## Status and goal

**DOING.** Implement portable Dynamic definitions, first-class Dynamic/FAT Programmer and Cue values, authoritative runtime instances, deterministic sampling, persistence, tracking, Preload, projection, and output through the existing application and contribution boundaries.

## Portable definitions

`DynamicDefinition` is a portable show object:

```text
DynamicDefinition
  id: UUID
  pool_number: integer 1...9999
  revision: positive integer
  name: string
  presentation: shared pool-object presentation fields
  target_binding: LiveGroup | FrozenTargets | Targetless
  lanes: ordered DynamicLane[]
  random_groups: local RandomGroup[]
  phase: PhaseDistribution
  speed: DynamicSpeed
  default_activation: ActivationPolicy
```

The exact Rust/wire names may follow repository conventions, but those fields and ownership boundaries are required.

`DynamicLane` contains one canonical continuous-scalar attribute, all preserved Keyframes/Max-min/Middle-amplitude configurations, the selected mode, per-lane multiplier, width/scale, optional local Random-group link, and validation state. Each scalar source is:

```text
Current
Value(typed scalar)
Preset {
  preset_id,
  attribute_id,
  last_valid_scalar_fallback_by_target
}
```

Regular Presets remain static-only. A live Preset edit updates matching Dynamic scalar sources and hot-swaps valid running definitions in phase. A missing Preset or missing per-target attribute uses the stored last-valid scalar fallback when one exists and reports the stale reference; without a fallback that target/lane is invalid and skipped. A reference never switches to another Preset by number or array position.

Live Group bindings store Group UUID. Frozen targets store the exact ordered fixture/head identities. Targetless stores no targets.

## Object identity and references

Dynamic references in Cues and Playbacks use stable UUID identity, not pool number:

```text
DynamicReference
  dynamic_id: UUID | null
  last_known_pool_number
  embedded_fallback: DynamicDefinitionSnapshot
```

- Move/renumber preserves UUID, so references follow.
- Copy creates a new UUID and copies definition content plus live Group/Preset dependency identities.
- Editing/replacing the same object preserves UUID and updates every live reference.
- Delete atomically writes the last valid compiled definition into every reference, clears `dynamic_id`, keeps the last-known number for display, and lets existing/running references continue from the embedded snapshot.
- A new object in the deleted object's old slot never captures those embedded references.
- Deletion is rejected if the snapshot transaction cannot update every reference atomically.

Selective import copies a Dynamic and its live Group/Preset dependencies through the normal preview/conflict/remap workflow. Embedded snapshots require no external Dynamic dependency but still report any Group/Preset references they contain. Import never rewrites an unresolved dependency to an unrelated same-number object.

## First-class values and instance linking

The semantic value boundary gains:

```text
Static(TimedValue)
DynamicOn {
  instance_link,
  dynamic_reference,
  lane_id,
  instance_overrides,
  transition_timing
}
DynamicOff {
  instance_link
}
FixAt {
  value,
  timing
}
Release
```

This is represented through typed Programmer/Cue/Preload content, not hidden inside raw `AttributeValue` JSON. Each target/attribute stores its own Dynamic On lane value. `instance_link` coordinates lanes that start together, but does not contain a multi-attribute curve or value node.

Dynamic Off targets one exact instance link. The application service atomically expands an instance-wide Off operation into the linked per-address tracked state. FAT remains per addressed attribute.

## Runtime service and engine boundary

A supervised Dynamic Runtime Service lives outside the render engine, transport adapters, frontend, and output scheduler. It owns:

- compiled definitions and embedded snapshots;
- instance/control-stack identity;
- resolved targets and captured phase maps;
- monotonic/local and Speed Group clocks;
- activation, pending boundary, pause, resume, and release state;
- Random group state and deterministic seeds;
- lane evaluation and scalar validation;
- activation/release mixes;
- source priority/LTP metadata; and
- bounded runtime telemetry.

At each authoritative output tick it emits a finite immutable `ContributionBatch`. The production output scheduler supplies these batches to the engine's prepared external-contribution seam before fixture projection/encoding. The engine retains ordinary contribution arbitration, Group/Grand Masters, Blackout, fixture projection, output diagnostics, and DMX delivery.

No definition edit, Preset lookup, Group lookup, SQLite access, JSON serialization, event publication, OSC work, or frontend telemetry occurs in the timing-critical sampling loop. Definitions, dependencies, and target projections compile before installation.

## Instance classes

### Target-bound singleton

Every Live Group or Frozen-target Dynamic has exactly one runtime instance for the loaded show.

Several Programmer, Cue, or Playback sources may hold it concurrently. The singleton retains one clock and target projection. A control stack resolves the controlling source:

1. highest source priority;
2. at equal priority, latest LTP activation/control timestamp.

The winning controller supplies Size, Master-related instance input, local speed multiplier, pause/resume override, and activation policy. Releasing it reveals the previous active controller without restarting the singleton.

### Targetless independent instance

Every start of a targetless Dynamic for a distinct source/target scope creates an independent instance with its own:

- UUID instance identity;
- resolved ordered targets and phase map;
- clock/epoch and activation policy;
- Random streams;
- Size, speed, pause, and transition state; and
- source ownership.

Instances from different Cues, Playbacks, users, or target scopes never share release or pause state merely because they use the same definition. Speed Group transport may align them without coupling lifecycle.

One Programmer pool/command toggle reuses its matching Dynamic UUID-and-target-scope instance so the second toggle turns it Off.

## Static base, Dynamics, and FAT arbitration

Evaluation is separated into two acyclic layers.

### Ordinary static base

Ordinary static Programmer/Playback/Cue sources resolve through their documented HTP/LTP and priority behavior. Their winning scalar value is `Current`.

`Current`:

- follows the live winning ordinary static value continuously;
- excludes every Dynamic and FAT result;
- updates a running Dynamic without restarting it;
- is evaluated independently for each target/attribute; and
- cannot depend recursively on the Dynamic that consumes it.

### Dynamic/FAT winner stack

Dynamic instances never feed into one another in the first release. Every Dynamic evaluates against the ordinary static base. Modulation graphs, add/multiply composition, and feed-forward `Current` remain later explicit features.

For each target/attribute:

1. collect active Dynamic and FAT candidates;
2. choose highest source priority;
3. at equal priority choose latest LTP activation timestamp;
4. evaluate only the winning Dynamic/FAT contribution for visible output; and
5. retain losing Dynamics as hidden running instances.

A later FAT hides earlier Dynamics. A later equal-priority Dynamic retakes control. Higher priority always resolves before recency.

Hidden Dynamics keep advancing even when every lane is hidden. They reveal their current phase when the winner releases. Only Dynamic Off, source/playback Off or release, show unload, or another explicit terminal action ends the instance.

FAT does not pause. It supplies a fixed scalar winner on the addressed attribute. With no Dynamic candidate, FAT behaves like the equivalent ordinary static value for output and recording.

### Intensity

Ordinary static Intensity resolves its base through normal HTP. A winning Dynamic then owns the final pre-master animated Intensity and may move below `Current`; otherwise a 100% static base would make dimmer waves/strobes impossible and a 50%-centered wave would be clipped to its upper half.

The ordinary base remains alive underneath for Current and release. Playback Size/Master, Group Masters, Grand Master, Blackout, and output safety apply at their specified later boundaries.

## Size, Master, and activation mix

These are separate:

- **Size** scales a lane around its mode-native pivot: first/closing keyframe, Max/min midpoint, Middle, or Random low value.
- **Master** is a Dynamic Playback fader behavior defined in the Playback plan.
- **Activation mix** cross-fades between the exposed underlying value and the Dynamic result.

For one scalar address:

`visible(t) = base(t) * (1 - mix(t)) + dynamic(t) * mix(t)`

Programmer activation uses Programmer Fade. Cue values use their per-value/Cue timing fallback. Playback activation/release uses the assignment's transition configuration. The Dynamic clock starts immediately while mix grows. Release reverses the mix toward the newly exposed winner, so absolute and Current-based Dynamics do not jump.

Definition/parameter hot-swaps preserve phase and do not acquire Programmer Fade. If a valid changed curve would make the current sample discontinuous, the evaluator changes at the next sample as an explicit live edit; only start/release ownership transitions use activation mix.

## Speed, synchronization, and sampling

Speed and editor controls follow the Dynamics Window plan.

- Fixed duration is positive seconds per complete cycle.
- Speed Group source uses A-E authoritative BPM, phase origin, paused/advancing state, and a positive rational beats-per-cycle value.
- Overall and per-lane rational multipliers support multiply/divide by at least 2, 3, and 4.
- All lanes in an instance use one monotonic epoch.
- Fixture phase is independent from transport phase and activation quantization.

Each definition has a backward-compatible Run Mode. Loop is the default. One-shot samples exactly one complete effective cycle, marks its owned runtime instance terminal at the cycle boundary, removes it from operator-visible running projections and output, and emits the normal authoritative instance-off transition. Runtime persistence retains that terminal ownership while the authored source remains active, so reconciliation of an unchanged Programmer, Cue, or Playback `DynamicOn` cannot restart a completed one-shot after the next tick or process restart. Removing that source releases the terminal ownership; a later deliberate activation may run the Dynamic once again.

Activation policies are Start now, Join sync now, and Next boundary. Join/Next require a Speed Group. A missing/deleted Speed Group blocks a new start and leaves an already compiled instance in a visible failed/held state; it never silently changes to a different group.

Literal Pause freezes sampled output and phase:

- local Start-now instances resume from the frozen phase;
- synchronized Join instances rejoin the authoritative Speed Group position;
- Next-boundary instances wait for their configured next boundary; and
- a playback may override resume policy as defined in the Playback plan.

When synchronized resume would jump, transition from the held sample to the rejoined sample through the configured activation transition unless Snap is explicitly configured.

The global Pause Dynamics control affects Programmer, Cue, and Dynamic Playback instances plus any retained compatibility-free Dynamic source. Speed Group Pause affects only instances linked to that group. Definition previews do not exist.

Sampling occurs exactly on the authoritative output/manual-clock tick. The evaluator accepts positive durations but reports an aliasing warning when a cycle segment receives fewer than four output samples at the configured output rate. It never invents sub-frame DMX output or blocks the scheduler.

## Random evaluation

Random groups are local to one definition and portable. Each contains a stable group ID and seed. Linked lanes receive one normalized envelope stream per stable target identity.

An independent instance derives its stream from:

- local group seed;
- instance UUID;
- stable target identity; and
- monotonically increasing decision/event index.

This makes lanes within one instance correlate while different instances remain independent. Save/reload, virtual-time replay, and synchronized resume reproduce deterministic state from persisted/runtime epoch and event index.

At each scaled millisecond decision boundary, an off target starts a pulse according to Start Probability. Pulse duration is drawn deterministically from the configured Gaussian mean/spread, bounded to at least one evaluator/output interval. Attack and Decay consume their configured percentages and the remainder holds high. A target cannot start an overlapping second pulse.

Dynamic/lane/playback speed scales decision interval and pulse duration inversely. Random output then maps the normalized envelope independently through each lane's low/high scalar sources.

## Phase projection

Phase order, Blocks, balanced Repeats, Wings, endpoint-exclusive Span/Offset, explicit `THRU`, spatial center, missing-position append behavior, and Random-each-loop use the exact pipeline in the Dynamics Window plan.

The Dynamics Runtime owns a replaceable target-ordering provider that currently consumes Stage X/Z positions directly. It does not persist a private selection-grid schema. A later shared grid implementation may replace the provider without changing Dynamic definitions or instance semantics.

Target projection is captured at instance start:

- Live Group membership/order resolves then;
- Frozen targets retain stored order;
- targetless scope resolves from the start request;
- spatial positions map then; and
- Stage/Group changes affect the next restart, not a live instance.

Missing/deleted targets are skipped by stable identity and reported. Missing Stage positions are appended in stored order. Unpatched fixtures remain evaluated and visible but produce no physical DMX.

## Cue storage and tracking

Static, Dynamic On, Dynamic Off, and FAT are separate tracked per-attribute layers.

Example in one Cuelist:

1. Cue 1 records static Intensity 50%.
2. Cue 2 records Dynamic On using Current; the static track remains.
3. Cue 3 records static Intensity 70%; the Dynamic remains tracked and follows 70% without restart.
4. Cue 4 records Dynamic Off for that instance; only that Dynamic ends.

The same independence applies across separate Cuelists; sources then meet in priority arbitration.

Cue tracking rules:

- omitting a Dynamic layer retains its tracked On/Off/FAT state;
- Dynamic Off targets one instance and does not stop other Dynamics on the same attributes;
- FAT affects all competing Dynamics on its addressed attribute but does not delete their tracked state;
- ordinary static values never imply Dynamic Off;
- Dynamic On/Off/FAT values use per-value timing and Cue timing fallback;
- Cue-only generates automatic restoration of the prior Dynamic/FAT/Off state in the following Cue;
- instance-wide Dynamic Off is expanded atomically to linked per-attribute deltas;
- renumbering/moving Cues preserves instance links;
- deleting an active Cue follows the existing held-output/navigation policy while retaining enough instance identity to release deliberately; and
- a Cue row shows the Dynamics icon when its stored/tracked delta contains Dynamic On, Dynamic Off, or FAT.

Record Merge/Overwrite and Update must preserve unrelated static and Dynamic layers. A whole-Cue overwrite replaces the explicitly captured layer set according to the existing recording mode; it must not clear Dynamics merely because the old prototype `phasers` vector was cleared.

## Preload

Preload supports definition edits, Dynamic On, Dynamic Off, FAT, static values, and Playback actions.

While Preload is active:

- accepted Dynamic definition edits still create authoritative persisted revisions;
- live instances pin their pre-Preload effective revision;
- the Preload projection compiles/evaluates the latest revision;
- Stage/Fixture Sheet panes following Preload update immediately;
- Live Stage, live Fixture Sheet context, running instances, and physical output remain unchanged;
- new Dynamic On/Off/FAT and playback actions remain staged; and
- the editor itself has no preview.

On **Preload Go**:

1. publish/install and compile all staged/latest Dynamic definition revisions first;
2. after successful compilation, atomically commit staged static values, Dynamic On/Off/FAT state, and Playback actions at one runtime timestamp; and
3. render no output frame between definition installation and the atomic runtime commit.

Any definition failure aborts the runtime commit and leaves live output unchanged.

On clearing/leaving Preload without Go:

- discard staged instance/static/playback actions;
- retain accepted Dynamic definition edits;
- unpin live instances and hot-swap them to the latest valid revision; and
- apply the configured transition behavior for any ownership-visible change.

Preload capture masks and release rules include the Dynamic layer explicitly. No client performs per-lane fan-out or commit ordering.

## Immediate definition editing and concurrency

Every editor operation uses a typed object intent on `POST /api/v2/dynamics/{id}/update` with:

- client request ID and replay-safe outcome;
- expected definition revision;
- only the changed lane/field/order/target intent;
- tolerant unknown-field decoding and logged paths without values; and
- one in-memory mutation, SQLite WAL commit, revision, event, and runtime-install ordering.

Create, move, copy, and delete are typed intents:

- `POST /api/v2/dynamics/create` lets the server allocate/confirm the requested free pool slot atomically;
- `POST /api/v2/dynamics/{id}/move`;
- `POST /api/v2/dynamics/{id}/copy`;
- `POST /api/v2/dynamics/{id}/delete`; and
- `POST /api/v2/dynamics/{id}/update`.

There are no show-scoped route paths. `X-Tosk-Show` guards the active show. Concurrent users are last-write-wins after deliberate stale-state reread/reapply; concurrent creates cannot collide.

Continuous encoder edits use one mutation-group identity and bounded update cadence. Each accepted sample is authoritative; the server may coalesce persistence/runtime projection within that group only if the final ordered revisions, undo behavior, events, and crash recovery remain correct.

## Snapshots, events, and feedback

The authoritative bootstrap/snapshot exposes:

- Dynamic object catalog and revisions;
- validation/dependency state;
- Programmer/Preload Dynamic values;
- running/pending/paused/hidden/failed instance projections;
- singleton control stacks and winning controller;
- target/lane coverage;
- Fixture Sheet stack and resolved value;
- playback assignment/runtime data; and
- global/Speed Group pause state.

Durable semantic events cover object mutation, instance start/pending/active/off/release, control-stack winner, pause/resume, failed dependency, Preload commit, and transition completion. Replaceable bounded telemetry covers transport phase, sampled values, and Random activity. Reconnect always re-reads authoritative snapshots and never replays queued actions.

Running-source Stop acts on exact instance identity. It never infers Dynamics by counting Cue phasers or releases an entire source Playback merely to stop one unrelated instance.

## Persistence, restart, and recovery

Portable show data includes:

- Dynamic definitions and revisions;
- Group/Preset dependencies and fallbacks;
- Cue Dynamic On/Off/FAT values and instance links;
- Playback Dynamic references/snapshots and configuration;
- local Random seeds;
- targets, phase, speed, activation, and lane configurations; and
- embedded snapshots created by Dynamic deletion.

Desk/output runtime includes active instance/controller state, phase/epoch, pause timestamps, playback-local controls, Random event indices, and global Pause Dynamics according to the existing runtime-persistence boundary.

Save/reload and application restart restore deterministic output:

- Start-now instances resume their persisted local epoch/phase;
- synchronized instances rejoin their Speed Group transport;
- pending Next-boundary instances retain pending intent;
- hidden instances remain hidden and advancing;
- paused instances restore the frozen sample/phase; and
- malformed instance runtime is ignored safely without discarding valid show definitions.

Malformed definitions remain visible and non-startable with actionable validation. A bad active show still enters the repository's recovery path and preserves the original.

## Removal of legacy Cue phasers

The existing `Cue.phasers`, Phaser sampler/contribution code, `/api/v2/cue-lists/{id}/dynamics/record` convenience writer, frontend `storeDynamic` helper, `dynamic.speed` special-dialog writer, running-source phaser counting, and Phaser-specific tests are removed. They are not migrated into Dynamics.

Legacy `phasers` fields are treated as unsupported prototype data:

- tolerant show decoding ignores them silently;
- they produce no runtime value or warning;
- the next save omits them; and
- no compatibility evaluator remains.

The repository has exactly three canonical SQLite shows: `assets/demo.show`, `tests/fixtures/compact-rig.show`, and `tests/fixtures/default-stage.show`. Only `assets/demo.show` currently contains `phasers` fields, and all are empty. Re-save/regenerate all three through the real server path after the schema change and verify that no Phaser field or behavior remains.

## Performance

Dynamics must preserve the output goals in the major-refactoring contract:

- hard floor: 32 fully packed universes at 100 Hz with representative simultaneous Dynamics;
- target evidence: 64 universes at 120 Hz; and
- low-power evidence: 4–8 universes at 40 Hz.

Benchmarks include multiple simultaneous multi-lane instances, singleton controller changes, Random pulses, spatial phase, Preset/Current sources, FAT, hidden stacks, wet/dry transitions, Fixture Sheet telemetry, and optional Sound-to-Light/Speed Group load.

Runtime complexity is bounded by active target/lane contributions. Hidden instances advance clocks/Random state without emitting losing per-address contributions into fixture projection. Precompiled scalar evaluators and immutable batches avoid allocation and dependency lookup in the sample loop.

## Backend and acceptance tests

Write deterministic backend tests with implementation, before UI acceptance:

- schema validation, all lane modes, mode preservation, Preset fallback, and patent-boundary structure;
- fixed and Speed Group clocks, rational multipliers, all start/resume policies, pause, hidden continuation, restart, and alias warnings;
- Gaussian Random seeds, instance independence, lane correlation, speed scaling, Attack/Decay, save/reload, and virtual-time replay;
- selection, linear, radial, axial, missing-position append, Blocks, balanced uneven Repeats, Wings, endpoint-exclusive spans, explicit `THRU`, and Random-each-loop;
- Current following static changes, Dynamics not feeding Dynamics, priority then LTP, FAT, Dynamic Off, Intensity below HTP base, and wet/dry fade boundaries;
- singleton and targetless instances across Programmer, Cues, multiple Playbacks, multiple users, and controller fallback;
- Record/Update/tracking/Cue-only/Preload definition-first commit/clear, deletion snapshots, selective import, save/reload, startup recovery, and malformed data;
- ignored legacy Phaser data and regenerated canonical shows;
- typed object intents, live WS actions, matching HTTP URLs, OSC actions/feedback, request replay, stale revisions, tolerant fields, snapshots, and events; and
- resolved output and decoded DMX at exact manual-clock timestamps, plus the performance matrix.

After the user's UI acceptance checkpoint, add the pool/editor/encoder and cross-surface UI scenarios described in the other plans, then update help/manual screenshots.

## Result

### Changes

- Implemented validated scalar definitions, target-bound singletons, targetless independent
  instances, controller stacks, priority/LTP/FAT arbitration, activation/release mix, One-shot,
  fixed and synchronized clocks, deterministic Random streams, and spatial phase distribution.
- Corrected lane-width evaluation, inward missing-position ordering, Random-each-loop hashing,
  Intensity FAT HTP behavior, synchronized resume crossfades, and show-scoped runtime restoration.
- Persisted Preset sampler fallbacks losslessly, preserved unknown fields, recovered malformed
  runtime safely, and removed legacy Phaser behavior and stored fields.
- Added exact deletion fallback continuity for Cue and Playback references and prevented pool-slot
  recapture.

### Verification

- `cargo test -p light-dynamics` — 26 passed.
- Focused application migration/import/update — 8 passed.
- Focused restart, malformed runtime, deletion, command, OSC, output arbitration, and exact
  virtual-time tests passed.
- Release performance matrix: required 32 universes at 100 Hz passed with no missed windows; 4 and
  8 universes at 40 Hz passed; 64 universes at 120 Hz was measured at 58.34 Hz.
- Canonical SQLite show audit found zero remaining Phaser object fields.

### Limitations

- The 64-universe rate is an unmet target on this machine, not the required 32-universe floor.
- Allocation rate is not instrumented. Runtime definitions and dependencies are installed before
  sampling, while further buffer reuse would require a broader sampling API redesign.

### Commit

`fix(dynamics): align runtime phase and lane sampling`, `fix(dynamics): persist preset sampler
fallbacks`, `fix(dynamics): transition synchronized resume`, and `fix(dynamics): isolate persisted
runtime by show`.
