# Playback Assignment

## Status and goal

**DOING.** Allow a Dynamic to be assigned to a Playback and operated as a single animated-value source with configurable fader and button actions, independent source ownership, exact feedback, Preload, page addressing, and auto-off.

## Assignment

`[SET]` → Dynamic pool tile → Playback and command `SET DYNAMIC <number> PLAYBACK <number>` create or replace a Dynamic Playback assignment through the existing Playback configuration service.

The assignment stores:

- Dynamic stable UUID reference plus embedded fallback snapshot;
- assignment revision and presentation/color fields;
- fader mode;
- three configured button actions;
- source priority and normal playback protection/settings;
- activation and resume-policy overrides;
- local speed multiplier and learned fixed-duration speed;
- non-intensity Master snap/cross-fade choice;
- fader-zero and Flash-release auto-off settings;
- **Turn off when other playbacks take full control**, enabled by default with an opt-out; and
- current/explicit page and Playback identity through the existing addressing model.

Moving/renumbering the Dynamic preserves the reference. Editing it hot-swaps the running instance in phase. Deleting it embeds the last valid definition into the assignment; the Playback remains operable and is labelled as an embedded snapshot.

An invalid/missing Group, Preset, attribute, target, or definition blocks a new start with an actionable error. It never retargets by pool number.

## Runtime identity

A target-bound Dynamic uses its one singleton runtime even when several Playbacks/Cues/Programmers reference it. Every active source becomes a controller in the singleton control stack. Priority then LTP selects the controlling source's Size, Master input, local speed, pause/resume override, and activation policy. Releasing that controller reveals the prior controller without restarting the clock.

A targetless Dynamic Playback creates one independent instance owned by that Playback assignment and its resolved target scope. The stored Playback assignment must include an explicit target expression for a targetless Dynamic; assignment is rejected until the operator chooses a live Group or frozen ordered fixture/head scope. Playback execution never falls back to “all compatible fixtures” because a show-control assignment must be reviewable before running.

The same targetless Dynamic assigned to two Playbacks creates two independent instances. Their Off, pause, speed, Random, Size, Master, and auto-off states do not leak across assignments.

## Fader modes

Dynamic Playbacks add these selectable fader assignments:

1. **None**
2. **Master**
3. **Size**
4. **Size + Master**

### Master

Master is normal playback output control after Dynamic lane evaluation and activation mix:

- Intensity output is multiplied by Master.
- Position, Color, and other non-intensity attributes either snap to the Dynamic above zero or cross-fade from the exposed underlying value to the Dynamic according to the assignment's **Cross-fade non-intensity** setting.
- At Master zero, the Dynamic remains clocked when the Playback stays On, but its Intensity result is zero and non-intensity behavior follows the configured zero/snap rule.

### Size

Size scales every lane's authored Size around its mode-native pivot:

- Keyframes: first/closing value.
- Max/min: interval midpoint.
- Middle/amplitude: Middle.
- Random: low value.

At 100%, authored Size is unchanged. At 50%, the excursion is halved. At exact zero, the Dynamic contribution is absent and the underlying stack is visible, while the instance may remain running/synchronized when fader-zero auto-off is disabled. Crossing upward from zero uses the assignment's activation transition so the pivot does not appear as an un-timed jump.

Size never scales a static output master and never changes the stored Dynamic definition.

### Size + Master

Apply Size to every lane first, evaluate the Dynamic, then apply Master to the resulting output. For a 50%-middle sine with authored Size 100%:

- Size at 50% produces 25–75%.
- Master at 50% with Size 100% produces 0–50% Intensity.
- Size + Master at 50% first produces 25–75%, then produces 12.5–37.5% Intensity.

The fader operation is playback-local and does not mutate the pool Dynamic or another controller.

## Starting from the fader

Raising a Dynamic Playback fader from zero starts an Off assignment. It resolves targets, installs/reuses the instance, and follows the assignment's activation policy and transition.

If **Auto-off when fader reaches zero** is enabled, accepting zero turns the Playback Off. Raising it again starts/restarts the assignment. If disabled:

- the Playback remains On at zero;
- the runtime remains available/synchronized;
- Master and Size+Master retain their defined zero output;
- Size exposes the underlying stack at exact zero; and
- feedback must distinguish **On at zero** from Off.

Hardware pickup/takeover behavior remains the existing playback-fader contract and never applies a stale physical value immediately after assignment, page change, or external mutation.

## Button actions

Dynamic Playbacks support:

- On
- Off
- Toggle
- Pause/Resume
- Flash
- Restart
- Double Speed
- Half Speed
- Tap/Learn Speed
- Empty Button

New assignments default to:

1. **Off**
2. **Pause**
3. **Flash**
4. fader **Size + Master**

### Off, On, Toggle, and Restart

- **Off** releases the Playback controller and its independent targetless instance. For a singleton it removes only that controller; another controller may remain.
- **On** starts using the configured activation policy.
- **Toggle** alternates those On/Off operations.
- **Restart** re-enters the configured start policy: Start now resets local phase zero, Join sync uses current synchronized position, and Next boundary becomes pending for the next boundary.

Off is terminal for that Playback source. It is distinct from hidden loss, Pause, fader zero with auto-off disabled, and tracked Dynamic Off inside a Cuelist.

### Pause/Resume

Pause freezes the current sampled output and the affected instance phase.

Each assignment stores a resume override:

- **Follow Dynamic** — Start-now resumes frozen local phase; Join-sync rejoins current Speed Group position; Next-boundary waits for the next configured boundary.
- **Resume frozen phase** — continue from the held phase even for a Speed-Group definition.
- **Rejoin synchronized position** — require a Speed Group and rejoin immediately.
- **Resume on next boundary** — require a Speed Group and wait for the configured boundary.

Follow Dynamic is the default. A synchronized resume that changes sampled value uses the assignment transition unless Snap is configured.

For a target-bound singleton, only the winning controller's Pause/resume override controls the singleton. A losing controller may update its stored pause intent, which becomes effective if it later wins.

### Flash

Pressing Flash creates a temporary controller at the assignment's Flash semantics:

- from Off, it starts/joins/pends according to the assignment's Flash start policy, using full configured Size/Master values;
- while On, it adds only the normal temporary priority/controller entry;
- release removes that temporary controller;
- release never counts as another playback taking full control; and
- enabled **Auto-off when Flash is released** removes the temporary controller first and then turns the underlying Dynamic Playback Off.

The existing Flash release-mode configuration remains visible where non-intensity retention applies.

### Double Speed and Half Speed

These actions multiply/divide this Playback controller's local speed by two. They do not edit the pool Dynamic or a Speed Group. Repeated actions compose using the bounded rational multiplier model and preserve phase/transport epoch.

They work for fixed-duration and Speed-Group Dynamics.

### Tap/Learn Speed

Tap/Learn is available only for fixed-duration Dynamics. It is disabled with a clear explanation for Speed-Group Dynamics.

For fixed duration:

- the first tap arms Learn;
- later taps use the existing robust tap-tempo window/outlier behavior;
- the learned interval becomes this Playback's local complete-cycle duration;
- it does not edit the pool Dynamic;
- Double/Half acts on the learned local duration; and
- clearing the learned value returns to the definition's fixed duration plus local rational multiplier.

## Auto-off and full-control overwrite

Dynamic Playbacks expose three independent settings.

### When fader reaches zero

Disabled by default. When enabled, accepted zero turns the assignment Off. It applies to Master, Size, and Size + Master because exact zero is the end of the configured Dynamic control. It does not apply to None.

### When Flash is released

Disabled by default. When enabled, Flash release removes the temporary controller and then turns the underlying assignment Off. It does not apply to another temporary action.

### Turn off when other playbacks take full control

Enabled by default for new Dynamic Playbacks and explicitly opt-out.

Another persistent normal source has full control only when it wins every active target/attribute address contributed by this Playback. Then the Playback switches Off through one authoritative mutation.

- Partial lane overwrite leaves the Playback On.
- Hidden loss on every lane triggers Off only when this option is enabled.
- Temporary Flash, Swap, or temporary FAT coverage never counts.
- A persistent FAT or Dynamic from another normal source may count when it wins every address.
- The check uses actual stable addresses and the current compiled target set, not object-level labels.
- For a singleton, turning this Playback Off removes only its controller; another controller may keep the singleton running.

This setting is different from tracked Dynamics remaining hidden inside an active Cuelist. Cuelist tracking ends only through Dynamic Off/source release or the Cuelist's own full-control auto-off behavior.

## Preload

Preload may queue:

- Dynamic Playback On/Off/Toggle/Restart;
- Pause/resume;
- Flash press/release only where the existing Preload temporary-action contract permits it;
- fader/Size/Master values;
- local speed actions; and
- assignment changes only through the definition-first object-edit phase.

Preload Go installs Dynamic definition revisions first, then commits Playback and Programmer state at one runtime timestamp with no intermediate output frame. Preload release/clear discards staged Playback actions. The Preload projection shows pending state and projected output; Live output remains unchanged.

## UI and feedback

Playback Configuration identifies target type **Dynamic** and shows:

- Dynamic object/snapshot identity and target scope;
- fader mode and non-intensity Master behavior;
- three button assignments including Double/Half/Tap;
- activation and resume override;
- local learned/multiplier speed;
- auto-off settings and default-on full-control setting;
- validation/dependency state; and
- active singleton/independent instance identity.

Software, hardware-connected, virtual, and external playback surfaces show:

- Dynamic name/number or embedded label;
- On/Off/zero/pending/active/paused/hidden/failed state;
- winning/losing singleton controller status;
- effective Size, Master, speed source/multiplier, and learned duration;
- target/lane coverage and warning state; and
- button LED/pressed feedback.

Running-source feedback stops one exact Dynamic instance/controller. A source started from a Cuelist is not stopped by releasing an unrelated whole Playback merely because both contain Dynamics.

## Addressing and transport surfaces

Current-page and explicit-page Playback addressing remain distinct. Page changes do not retarget a running assignment. The same action is available through:

- software/touch Playback;
- virtual Playback;
- attached hardware;
- keyboard/command line;
- ordered WebSocket action;
- matching plain HTTP action URL; and
- OSC input/feedback where Playbacks expose that action.

Dynamic-specific action names extend the typed Playback action enum rather than adding a second transport:

- `dynamic-double-speed`
- `dynamic-half-speed`
- `dynamic-learn-speed`
- `dynamic-restart`

Fader values continue through the ordinary typed Playback fader action with the configured fader mode in authoritative topology. HTTP paths use the existing `/api/v2/playbacks/{number}/...` current/explicit addressing conventions and optional desk header; OSC mirrors current-page and explicit-page forms. No client calculates Size, Master, auto-off, controller-stack, or target effects locally.

## Acceptance coverage

Backend/API/output tests written with implementation cover:

- stable reference and deletion snapshot assignment;
- target-bound singleton controller stacking and targetless independent instances;
- every fader mode at 0/50/100%, Size pivot math, Size-then-Master order, Intensity scaling, and non-intensity snap/cross-fade;
- fader raise from Off, On-at-zero, fader auto-off, and physical pickup;
- every button action, fixed-only Tap/Learn, Double/Half phase preservation, all resume overrides, Flash ordering, and Restart;
- default-on/opt-out full-control behavior, partial overwrite, persistent complete overwrite, and temporary negative controls;
- current/explicit page addressing, page changes, Preload, restart persistence, UI/API/OSC/hardware agreement, LEDs, and running-source identity; and
- exact virtual-time resolved/DMX output during activation, fader moves, pause/resume, controller fallback, auto-off, and release.

After UI acceptance, add exact production interaction tests for SET assignment, Playback Configuration choices, default layout, software/hardware/virtual faders and buttons, feedback labels/colors, touch targets, and modal geometry.

## Result

### Changes

- Added stable Dynamic Playback assignments with embedded fallbacks, explicit target scopes,
  fader modes, priority, activation/resume overrides, local speed, learned duration, and auto-off
  settings.
- Implemented On/Off/Toggle/Restart, Pause/Resume, Flash, Double/Half, Tap/Learn, fader start and
  zero auto-off, persistent full-control auto-off, and runtime persistence.
- Added current-page, explicit-page, and pool-number action routing across typed WebSocket and
  plain fire-and-forget HTTP URLs, plus OSC and authoritative runtime feedback.
- Aligned the production Playback configuration defaults with server defaults.

### Verification

- Dynamic Playback/Cue runtime focused tests: 6 passed.
- Full-control coverage, temporary negative control, runtime event, projection, current/explicit
  addressing, HTTP, OSC, and restart tests passed.
- Desktop playback/Dynamics defaults and reviewed software/hardware encoder automation passed.

### Limitations

- Physical fader pickup was not exercised on attached hardware in this local run; the shared
  hardware routing and pickup contracts remain unchanged and are covered by their existing tests.

### Commit

`fix(dynamics): align playback control defaults`, `feat(dynamics): add plain playback action URLs`,
and the authoritative runtime foundation commit.
