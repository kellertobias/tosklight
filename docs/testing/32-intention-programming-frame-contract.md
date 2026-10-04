# Intention Programming Frame Contract

## Purpose and status

Acceptance for the Position, Color and Focus/Zoom work in TL-544, especially the coherent
frame boundary in TL-548. The architecture is specified in
[Fixture-independent programming](../plans/fixture-independent-programming.md).

These are acceptance scenarios for the programming contract production reports since TL-552
(contract 1). `tests/121-intention-programming-frame-contract.spec.ts` drives the operator- and
API-observable parts end to end through the real server (HTTP, command line, published frame
readouts and DMX). Steps not listed below remain Rust-only or manual; Rust tests do not establish
operator, native Stage or capacity acceptance.

- **INTENT-FRAME-001**: Playwright covers steps 1–6 and storage/recall: the saved Pan Dynamic gains
  a Current Tilt partner; Tilt follows a static edit, a Speed Group pause and save/reopen; a later
  Angle Dynamic wins the whole pair with no mixed frame; a Pan FixAT masks only Pan and Undo reveals
  the running phase; Target and Angles replace each other atomically; Angles stored in a Preset, a
  Cue and a Group Cue recall in degrees on a replaced mover, and Preload GO commits them. FixAT fade
  timing is Rust-only (`full_coverage_and_release_make_masks_inactive_or_reveal_the_continuing_phase`,
  `angle_pair_normalization_is_persisted_stable_and_prunes_only_untouched_partners`).
- **INTENT-FRAME-002**: Playwright covers steps 1–3 and 6: deleting the aimed Point withholds the
  Angle Dynamic as a complete pair while Intensity continues and the Target reference is kept; a
  static Target holds its last aim; edits, FixAT and a Preset recall with nothing selected leave the
  Programmer unchanged (Undo history is not yet asserted). Steps 4–5 (Resume clocks across
  save/reopen, an unavailable original Direct model) are Rust-only. One test is an expected failure
  pending a product decision: a targetless Dynamic started with nothing selected falls back to
  every fixture in the show, so the Programmer changes.
- **INTENT-FRAME-003**: Playwright covers steps 1–4 and the record comparison of step 6: magenta with
  UV and a warm white survive RGB → RGBW → CMY replacement through Presets and a Group Cue, White is
  never retained, the wheel head reports its quality and unsupported UV passively, and a Direct
  recipe keeps its source identity with best-effort replay on a different fixture. Step 5 (Color
  FixAT masks) and the pause/release/transition parts of step 6 are Rust-only.
- **INTENT-FRAME-004**: Playwright covers steps 1–3 without PSN: DMX, Position readouts and the
  Color report publish one frame identity; reads are inert; a scalar Dynamic on a Point axis moves
  the Point once, the aimed mover follows in the same frame, and the show is not written. PSN steps
  (1, 7, 8) belong to tests/123 for [doc 19](19-tracking-with-posistagenet.md). Freeze is
  family-wide, so freezing one Point axis is not an operator action. Steps 4–6 and 9–20 are
  Rust-only, or manual where native Stage is involved.
- **INTENT-FRAME-005**: Playwright covers steps 1, 7 and 8 for static values: Preload publishes its
  own frame with the pending Tilt while the Live effect and Live Tilt are unchanged, reads dispatch
  nothing, Shift Preload clears, and Preload GO commits the pending Tilt under the running effect.
  Steps 2–6 and 9–20 (coalesced replay, journal cursors, rollback) are Rust-only
  (`cold_and_controls_follow_cursor_order_even_at_equal_time` and the `retained_preload_hybrid`
  tests). Observing both lanes in native Stage is manual.
- **INTENT-FRAME-006**: Playwright covers steps 3, 5 and 6: with crossfade enabled, a standalone
  Dynamic Playback master blends from static Current and zero is a Current-valued vote; without
  crossfade, a positive master keeps the endpoint and zero casts no vote; Cue brightness dims only
  Intensity; a Pan FixAT keeps its value while the master blends Tilt. These tests move the virtual
  fader. One test is an expected failure documenting a product bug: the physical fader is rejected.
  Steps 1–2, 4 and 7–8 are Rust-only
  (`cue_submillis_precedes_stable_order_in_actual_fixed_composition`,
  `captured_master_controls_endpoint_before_activation_over_a_different_dynamic_underlay`).

The older [Color Intent](26-color-intent.md) and [Dynamic reuse](28-dynamic-reuse-across-groups.md)
scenarios describe the existing contract. Their normalized Position and show-wide Direct/Intent
mode rules must not be used as acceptance rules for this new contract. Unrelated behavior remains
covered by those scenarios.

## INTENT-FRAME-001 — An Angle Dynamic owns a complete pair

1. On a calibrated mover, set static Pan to 30° and Tilt to 15°. Start a Pan Dynamic sweeping
   from −90° to 90° with its Tilt source set to static Current.
2. Confirm Pan moves and Tilt stays at 15°. Change static Tilt to −20° through an ordinary
   value edit. The Dynamic keeps running at its current phase and Tilt follows −20°.
3. Pause the Dynamic, change static Tilt to 10°, and confirm the paused Pan remains held while
   Tilt follows 10°. Save/reopen and verify the partner source is still Current, not a recorded
   copy of 15° or −20°.
4. Start a higher-ranked Angle Dynamic with a different animated Tilt. Its complete pair wins.
   No output frame combines the first Dynamic's Pan with the second Dynamic's Tilt.
5. Apply a Pan FixAT. Its explicit Pan mask takes effect with its timing while the unmasked
   Tilt continues according to the eligible family. Release the mask and verify the underlying
   Dynamic resumes according to its existing phase and transition rules.
6. Change the winning Position intent between Angles and a Point target. Only one representation
   owns Position in each composition; offsets and angles cannot become separate active owners.

Repeat storage and recall through a preset, a Cue, a Group-addressed Cue and Preload GO. Replacing
a mover preserves degrees or the target reference; it does not store normalized Pan/Tilt bytes.

## INTENT-FRAME-002 — Expected limitations remain passive

1. Use a Target as static Current and an Angle Dynamic that needs the solved joints for a
   keyframe, bound, pivot or Size calculation. Make the required mount/aim geometry unavailable.
2. The affected Dynamic Position candidate is withheld as a complete pair. Another eligible
   source or static family continues. The missing conversion never becomes zero, an unscaled
   endpoint, a release, or a Tilt borrowed from a different Dynamic.
3. Independent Intensity and Color effects continue. A visible passive notice may identify the
   affected fixture/family; no toast, modal or error blocks programming.
4. Let a Resume transition reach its nominal completion while the conversion is unavailable.
   Save/reopen, restore geometry and verify the original occurrence and clock continue. The
   transition must not silently restart and retained source history must not be replaced by
   the temporary fallback output.
5. Repeat with an unavailable original Direct color model. Preserve the original recipe and UV
   intent. Do not reinterpret its channels using the replacement fixture's native layout.
6. Perform the same edits and recalls with no fixtures selected. They succeed as no-ops without
   activation, Undo entries, or attention-grabbing feedback.

Malformed stored data remains a real failure and follows the existing recovery rules. It is not
reported as an expected capability limitation.

## INTENT-FRAME-003 — Color intent survives fixture replacement

Use an RGB mover, an RGBWAU fixture and a wheel fixture with Open, Warm White, Red, Yellow,
Light Blue, Dark Blue and Green. Profile/calibration confidence remains visible where relevant.

1. Program magenta, then warm white with Temperature/Duv and White Blend. Record each as a
   preset and Group-addressed Cue. Also record a color with independent UV above zero.
2. Replace the RGB fixture with RGBW, then CMY. Recall the records without editing them. The
   stored semantic color, white target, recipe and UV remain unchanged. The new fixture maps
   the same intent to its own controls; its new White channel cannot retain an unrelated value.
3. The wheel fixture reports its approximation passively. Unsupported UV stays stored, with a
   passive indication; it does not turn into visible violet or disappear from the record.
4. Switch to Direct control and set a native channel/wheel recipe. Record and recall it on the
   original fixture, then on a different fixture. The original source identity, recipe and exact
   native values stay stored. Replay on the different fixture is explicitly best effort.
5. Apply whole Color and narrow component FixAT values. A whole mask covers the complete Color
   owner, including UV and white controls; a narrow mask preserves its declared footprint and
   original complete source family. An unavailable attribution record does not remove the mask.
6. Exercise pause, release, Cue transitions, Preload GO, save/reopen and preset updates. Compare
   the semantic records before and after fixture replacement; fitted DMX must never replace them.

## INTENT-FRAME-004 — One captured frame, all consumers

1. Attach movers to a Point representing a moving truss and aim them at a separate Point.
   Move the truss with PSN while keeping the aim Point fixed. Also run a scalar Dynamic on a
   Point axis and Freeze a different Point axis.
2. DMX, native Stage mounting transforms and physical pose, value views and Channels must report
   the same show/generation/frame/sample identity. Final geometry includes the scalar Dynamic
   and Freeze once; typed adoption may not use the initial Point pose.
3. Open duplicate consumers, hide them, reconnect a slow consumer and change visible fixture
   selection. Readers retain or skip complete frames; they do not solve aim or color again and
   do not create a frame backlog. Motion must not write the show or trigger broad UI reloads.
4. Repatch, change mode/calibration, rebind a Point, delete its reference and reload the same
   show. Verify generation invalidation, documented fallback/hold behavior and absence of stale
   slots. An unchanged Point reuses cached mounts; a moved Point updates its dependents only.
5. Force a sampling or final-projection failure. Retain the previous accepted output, its source
   evidence and its identity. Do not commit speculative Dynamic history or mount continuity.
   An automatic Cue advance already captured still emits/persists its real event exactly once.
6. Enable Hold and edit sources. The retained values and origin catalogue remain one immutable
   pair, rather than joining held output to the current Programmer or Cue state.
7. Let two PSN senders report the same tracker. After the newer accepted position owns the
   Point, make its calibrated coordinates overflow. Hold the last finite position and identity;
   do not fall back to an older competing sample or refresh its age. A genuinely newer valid
   sample may take ownership under the normal deterministic source-order policy.
8. Load an older tracking configuration with duplicate binding UUIDs, including an enabled and
   disabled row sharing an ID. Preserve its stored body and revision at load; withhold every
   conflicting row and report their count only in passive receiver diagnostics. Independent
   bindings still drive their Points. Receive-off, unrelated edits and incremental repairs
   remain available; neither the PSN route nor generic object writes may introduce or increase
   a collision. Repairing one collision must not require repairing every other one first.
9. Reject a Dynamic definition or native dependency during cold preparation. The Engine, running
   instances, captured model pins and persisted revision remain unchanged. Prepare a valid edit,
   then advance an effect and change pause/speed before publication. Installation must preserve
   that latest runtime history and apply the current Blind definition-pinning policy. A prepared
   registry is not an earlier runtime snapshot.
10. Overlap Engine/registry publication with an output capture. Test both a new Engine snapshot
    awaiting its registry and an older capture after a newer registry is installed, including
    identical snapshot revision numbers and unchanged Dynamic-definition Arcs. Keep the last
    accepted output without sampling, changing controller membership/history or acknowledging
    the rejected inputs. Retry successfully once the exact snapshot/registry pair is installed.
    A Group-master-only generation change must continue using its unchanged snapshot identity.
11. At startup, prepare the Engine and fresh Dynamic registry against the exact candidate's
    original-model catalogue before committing a portable migration. An available model that
    proves a native value invalid rejects the candidate without publishing or creating a
    migration backup. Missing models stay passive. Retain the prepared runtime through startup,
    including original models for unpatched fixtures. Recovery must not retain a rejected
    candidate's catalogue; checkpoint recovery keeps its existing preservation and fallback rules.
12. Reject final runtime dependency validation after initial compilation but before persistence.
    Neither a backup nor commit, installation, reconciliation or mutation event may occur. If
    persistence itself fails, do not install the candidate. Cover ordinary object edits, output
    routes, route ranges and the shared transaction path; a no-change action skips finalization.
13. Prepare preset-source tables for multiple Dynamic instances. One invalid table or one stale
    dependency must prevent the whole batch from installing. Preserve the latest clocks, pause
    state and Random streams when a valid batch installs. Change a spatial ranking without
    changing targets, then change target order and restore it; reject earlier prepared results
    in both cases. A failed output transaction restores the previous tables and dependency
    generations so only results prepared against that restored state remain eligible.
14. Finalize destination Playback before committing a Cue, assignment or Group edit. Verify the
    candidate Dynamic rows match the subsequently installed state, including removed controllers
    and preserved global pause. Advancing the clock between preparation and installation must
    not rebase the candidate a second time or dispatch automatic Cue actions. Abandoning the
    candidate leaves Live unchanged. Release clears active state even when compilation reused
    the current Playback; preparation made before another installation cannot restore stale rows.
15. Exercise simultaneous show edits, Programmer Update and OSC Record/Update. Preserve activation
    ownership and acquire Programmer before show mutation and desk gates, followed by publication,
    ordered Playback and Dynamics. Native validation rejection or failed persistence leaves the
    snapshot, source provider, running history and selection untouched. Advance/pause a Dynamic
    after initial preparation, then finalize; its newest state must survive. Complete selection
    and Highlight callbacks only after releasing the runtime lease. Measure output stalls while
    the persistence callback runs; successful locking tests alone do not establish frame rate.

16. Supply an invalid current Programmer Dynamic controller during cold finalization. Reject it
    before persistence and retain the exact Engine/runtime pair. Correct the row after preparing a
    retry: finalization must use the latest accepted row. With an active Cue Dynamic, change its
    destination target scope and fail persistence; Live Cue rows, targets, controller identity and
    history stay unchanged. A successful retry installs the new scope with the original clock.
17. Edit a retained semantic Preset source while its Dynamic is active. Cold finalization compiles
    the destination tables after reconciling controllers. Failed persistence retains old tables and
    dependency identities; successful retry publishes the new values with the same active clock.
    Missing originals and verified fallback quality remain passive. Repeat with Group ranking and
    a Group referenced only by a retained fallback template.

18. Start a Dynamic whose Pan comes from a semantic Preset and whose Tilt passes through static
    Current. Its first sample must already contain the materialized Pan value; paused sampling
    still follows the static Tilt. Retarget or rebind the definition and compile only changed
    instances. Repeated missing-Group/native capability results must not trigger compilation on
    every frame. Force encoding failure after preparation and verify the previous tables and
    freshness state are restored; retry then prepares and commits once.

19. A present Preset Group with an invalid nested reference fails typed preparation before
    sampling. The frame transaction restores newly reconciled controllers, tables, freshness and
    sample boundary; retry against a corrected capture succeeds. The legacy scalar observer does
    not compile semantic Presets or panic on this typed-only dependency failure.

20. Restore a checkpoint after destination Group ranking or native capabilities change. The
    explicit restore and shared bootstrap seam prepare fresh Preset tables before publishing
    runtime and origins. Invalid dependency preparation leaves Live, its native pins and recording
    cursor unchanged and fails before Playback occurrence reservation. Success resets the cursor
    epoch and the first sample uses destination tables without resetting the restored clock.
    Also replace a suspended native instance with an empty checkpoint while its original model
    becomes available: discarded values must not veto the replacement. Incoming show activation
    validates against the destination before transition, Highlight, media or Engine changes and
    publishes the exact prepared runtime/origins; dependency failure keeps the prior show intact.

Measure full output and presentation paths using the capacity and timing gates in the architecture
plan. A passing idle readiness check or isolated solver benchmark is not performance acceptance.

## INTENT-FRAME-005 — Preload has independent retained history

1. Run an effect Live. Enter Preload and prepare different Color/Position intent plus a queued
   playback action. Observe Live and Follow Preload in native Stage simultaneously.
2. Pause/resume, restart an effect, and edit its size/speed/phase between two preview publication
   times. The preview reproduces those ordered controls and their original timing even when
   intermediate frame inputs are coalesced. Live history remains unchanged by preview sampling.
3. Edit a definition in Blind. Live keeps its pinned definition while the pending lane follows
   its explicit preview policy. Refresh a native model without resetting clocks or Random state.
4. Apply a Color Release whose before/after branches have different consumed controls. Only
   controls proven to belong to the released source are removed; unknown evidence remains
   unknown. Both branches use their own captured history and retained final geometry.
5. Open two Preload consumers, hide both, then show them again. They share one retained producer;
   returning visibility does not restart a queued GO. Wire throttling does not lose control edges.
6. Cause a preview failure and then advance Live. Keep the older coherent preview with its
   original Live identity and passive status. Reload the same show while work is in flight;
   the old activation's result must not replace the new show's result.
7. Commit with Preload GO or clear with Shift Preload. Retire the corresponding pending episode
   and reconcile both source identity and cached history. Repeated reads dispatch no commands.
8. Give Live and pending Position different static Tilt values, with a pending Pan Dynamic whose
   Tilt passes through Current. Preview uses pending Tilt, including while paused, even if Live
   changes after capture. Add scalar Point motion and verify target adoption uses that branch's
   final Point/mount geometry. Before/after Color Release must each use their exact source rows;
   substituting Live or the opposite branch's inputs fails before reconciliation changes state.
9. Pause and resume between coalesced preview samples after a visible source change. Reproduce
   the branch's required held-history sample boundary before applying the control. Do not copy
   Live held values or substitute a new transition identity. If the necessary boundary is
   unavailable, retain the previous preview with explicit passive status instead of claiming
   an up-to-date result or silently resetting phase.
10. Select pending source inputs at 40 ms while Live output and tracking run at different rates.
    Delay the worker across multiple selected inputs and throttle publication to 10 Hz. Compare
    with sequential pending execution for NextBoundary, speed-group pause, Random and a successful
    Current followed by unavailable Current. Rejected attempts retain their preceding successful
    branch history. Same-time accepted samples must have distinct identities; failed output and
    unwind must not advance the accepted history marker. Partial-instance samples and restored
    checkpoints must not claim a whole-runtime retained capture.
11. Replay a new start with the authoritative instance identity and original timing. With equal
    inputs and sampling cadence, Random progress matches. With different pending Current, a
    paused winning controller retains the pending graph. When several targetless clocks have
    eligible scope, use the recorded identity; a conflicting identity must not replace any
    clock. Fail after new-clock creation or completed-instance rearm and retry: history and
    bound indices roll back, and the start is accepted once through its journal cursor.
12. Place several controls at the same timestamp and split replay at selected-input boundaries.
    Sequence order remains exact, a duplicate batch is rejected, and a failed later command
    rolls back all earlier commands in that batch without advancing the cursor. Force bounded
    history eviction: operator controls still succeed; an old preview cursor reports a passive
    history gap. A retained batch remains immutable after pruning. Live one-shot completion
    must not turn a pending delayed Off into an immediate deletion.
13. Apply authored controls inside an output frame. Reads during the calculation see only the
    prior committed cursor; commit publishes the records once, while error or unwind discards
    them with provisional state. Recording after sampling starts is rejected. Preview forks
    never append to Live's journal; a committed cold candidate has independent storage. Failed
    restore preserves the cursor, successful restore invalidates it with a new epoch, and a
    second journal owner is rejected. Verify actual Start/Off/Update/global Pause entry points
    preserve their application timestamps and return values.
14. Reconcile a Programmer, Cue or Playback source after its authored activation time. Record
    acceptance at the current capture while preserving the original phase/transition origin;
    replay must not restart the effect. Repeat unchanged reconciliation without growing history.
    Change targets or spatial ranks, then mutate/discard caller inputs and prune the producer log:
    retained batches still replay the accepted mapping. A rejected output frame rolls back both
    reconciled state and records. With recording disabled, the same controls retain direct-path
    behavior. Automatic sampling cleanup does not create a duplicate authored release.
15. Start a Programmer, Cue and Playback Dynamic whose show definition has been deleted but whose
    source retains an embedded fallback. Record fallback insertion before Start at the same capture
    time. Replay onto a branch without the definition, and onto a branch with a same-ID edited
    definition: the first installs the fallback, the second keeps its current definition. Repeated
    reconciliation does not append insertion again. Reject the output/replay transaction after
    insertion and verify registry, instances and cursor are unchanged; retry succeeds.
16. Enable cold retention under the Dynamics guard and repeat seeding without resetting either
    history. Commit two cold edits with no intervening controls; require distinct ordered events
    and exact previous/destination snapshot identities. Reject preparation or persistence and
    require no new event or control cursor. Apply an event to Pending only after preceding ordinary
    controls; install definitions before its new Start and compile destination Preset tables from
    Pending manifests. Pending's independently sampled held Current survives a cold Pause. Failed,
    duplicate or out-of-order replay changes neither branch state nor either cursor. Eviction is
    passive while Live continues; retained event Arcs remain usable. Successful ordinary install
    or checkpoint restore invalidates old epochs; rejected restore preserves them.
17. Enable input retention and render through both authoritative entry points. Select at 0 and
    40 ms, but not 39 ms; derive selection time inside their shared ordered Playback boundary.
    Reject the complete Live render: neither cursor nor accepted-capture cadence advances, and
    retry at the same time is eligible. After success retain the exact frame, baseline, speed
    transports/rate, control/cold barriers and optional new sample marker. Later caller mutations
    cannot change them. Duplicate, mismatched and old-epoch tokens never block Live or append an
    incorrect entry. Cold events preserve input ordering; ordinary install invalidates it even
    for the same snapshot Arc. Eviction is passive and retained Arcs survive. Consumer result
    coalescing must preserve all selected attempts. Before enabling queued Playback, additionally
    retain exact virtual exclusions/desk origin and verify its branch overlay preserves timing.
18. Keep a running definition A pinned while the registry contains edited definition B. Reject
    a prepared Preload Playback installation after its generation becomes stale: effective A,
    instance history and retained cursors must remain unchanged. A successful fresh installation
    rebinds B while preserving the instance and breaks the old pending lineage. Verify queue
    rejection also precedes unpinning. Complete GO staging must additionally reject Dynamic or
    final-projection preparation before any Playback/runtime publication and keep persistence
    failures as committed success with warnings.
19. Prepare final GO feedback from a private Playback and Dynamic candidate. Cover physical and
    page-qualified virtual Cue owners, direct Cue-list feedback, Dynamic state, and Group fader
    pickup fields. Ancillary Group/Speed/Output controls retain their captured values. Mismatched
    authority or a missing required assignment rejects preparation without changing Live. Build
    event drafts and runtime projections as pairs; ordered publication returns one sequence per
    pair, and typed success assembly cannot return an error after publication.
20. Configure a zone containing Virtual Playback1001 and1302, activate1302 and queue1001 On/Go.
    GO releases1302 on its own page and publishes its release before1001's activation, preserving
    the Go navigation cause and one exclusion notice. An unrelated zone1301/1302 must not reject
    a page-one operation or release its peers. Addressed page/number mismatches still reject.

## INTENT-FRAME-006 — FixAT timing and Playback masters

1. Create competing masks whose Cue changes fall in the same millisecond but at different
   submillisecond times. The later real timestamp wins before stable-order ties. Repeat after
   reversing input iteration; fixed rows do not need fabricated Dynamic instance/lane IDs.
2. With a lower Dynamic providing an underlay different from static Current, fade a standalone
   Playback's non-intensity master. Its evaluated endpoint blends from static Current before
   ordinary activation/arbitration. Multiplying activation by the master is not equivalent.
3. With crossfade enabled, zero master is a Current-valued vote at the source's rank. With
   crossfade disabled, zero contributes no vote and a positive master leaves the endpoint intact.
4. Pause the Dynamic and move its master. Output follows the master while retained history stays
   unmastered. Complete Angle pairs remain complete and Color components keep their footprint.
5. Color, including RelativeOutput, stays a non-intensity family. Cue brightness is applied through
   the captured Intensity/source-master path exactly once, not by fading stored Color intent black.
6. Apply a component FixAT and then a whole-family FixAT while moving or suppressing the Dynamic's
   master. The mask retains its own authored timing and remains independent of that master.
   A partial mask fade is evaluated once; it must not also enter the legacy scalar stack.
7. Use a static Direct color whose pinned original fixture identity differs from the Dynamic's
   endpoint. Master zero selects that original Current recipe without borrowing the endpoint's
   native model. Intermediate values use the models from this captured frame only when needed
   for native interpolation or a surviving Direct result. A portable appearance conversion must
   not fail merely because an unused original native model is unavailable.
8. Read Direct Current from a recipe missing an original channel or containing a raw value
   outside its original function. Available original models reject it before component extraction,
   whole-family master-zero selection or adoption. Missing original models instead retain a
   passive requirement. Multiple Current reads and paused frames reuse an unchanged verification;
   a changed recipe/model or newly unavailable capability must invalidate that proof.

Discrete native wheel/function masks follow the established step transition, preserving the old
function until the destination endpoint. They must not sweep unrelated raw values, switch functions
early through adoption, or broaden a component mask to the whole recipe.
