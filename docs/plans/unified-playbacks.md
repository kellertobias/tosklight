# Unified Playbacks (TL-714)

## Contract and declared pre-release break

Pages 1–99 each have positions 1–1000. Storage is sparse: a page owns its assignment definitions directly. There is no global regular Playback pool, virtual bank, page-slot indirection or compatibility address alias. An assignment address is `page.position`; all controls displaying that address operate the same target and authoritative runtime.

TL-714 deliberately breaks pre-release persisted and transport compatibility. New portable shows use schema floor 10 and `light.playback_contract=1`; new desk layout databases use schema 17. Incompatible shows, saved runtime, named revisions and Undo/Redo content must be inspected before writable use and refused without changing original data. An incompatible desk starts a separate recovery subtree instead of rewriting its database. Recovery must be visible, preserve originals and permit a separate fresh show. This is recovery, not conversion of old Playback data. Repository-owned shows and generators must be regenerated through their documented writers before acceptance.

## Operator decisions

- Preload range union applies to all pages by default; optional explicit page restrictions narrow it. Unmatched positions remain Live. The permanent-action set and Temp toggle semantics stay as before; Flash and fader movement remain Live. Programmer capture remains independent.
- Typed, keyboard and shared OSC commands use the visibly selected surface's effective page for an unqualified position. Explicit addresses remain fixed.
- Flash bypasses Solo. Activating a permanent peer preserves held Flash contributions. Flash release removes only its temporary contribution, without reviving an old permanent winner or stopping the current winner.
- A page change preserves each held control's captured address and source until release. Simultaneous sources remain independent; cancellation/disconnection removes only the relevant temporary controls.

## Surfaces

A central desk registry gives each stable surface a name, row layout and page source. Rows store first position, count, presentation (fader, buttons or push encoder) and button count. Page sources are Pinned, Own or Follow another named surface; cycles and dangling follows are rejected. Screens and panes reference their registry identity. Take from Screen copies saved arrangement into an explicit draft reviewed before Apply, remains available offline, and never copies assignments or establishes an implicit live follow.

Literal acceptance geometry is Main faders 1–15, Main buttons 101–115, a second screen with 10×5 buttons 201–250 and five push encoders 301–305 above them, and a 20×14 personal pane 401–680. Gaps and row order are intentional.

## Runtime and API

Solo regions are show-owned named sets of explicit stable addresses, independent of pane lifetime. Central arbitration deduplicates overlapping peers and shared targets, preserves handover fades, and never chooses a replacement when the winner turns Off. Preload queues retain captured exact addresses, original control sources and action order for one atomic GO.

Live Playback actions execute once per received action and are never replayed or resent. HTTP live actions carry no request identity. Object update intents retain identity and revision guards; sparse assignment edits change the page object. Generated wire contracts are produced from Rust DTOs through the repository generator.

## Verification status and required acceptance

Implementation is in an isolated worktree based on checkpoint `b02d1ac3fbc465384603b52148cec66dcb319150`. The original candidate, data and RUN46 evidence remain preserved. Focused compile/unit checks are intermediate evidence and do not establish combined release acceptance.

Before integration, verify address boundaries and invalid values, sparse save/reopen, default initialization, incompatible show/desk/runtime/revision recovery without original mutation, each literal row mapping, page following and visible typed-command context, touch/mouse/keyboard/hardware held page-change gestures, independent simultaneous holds and disconnect cleanup, cross-page overlapping Solo including Flash and interrupted fades, Preload eligibility/order/Temp/atomic GO/timing, recording and live references, supported OSC addressing and feedback, and Pixel/media consumers. Rebuild using `npm run build:open` in the isolated environment, restart test apps between runs, capture captioned native screenshots on the internal screen, and verify representative output trajectories on both Art-Net and sACN. Rerun all affected release and downstream scenarios on the combined candidate. Human release acceptance remains separate.

## Implementation checkpoint before network interruption (2026-10-11)

This branch is an incomplete implementation checkpoint, not an accepted release. It preserves the unified sparse schema, transport and surface work for continuation; the previous release candidate remains on `codex/release-acceptance-fixes` at `b02d1ac3fbc465384603b52148cec66dcb319150`.

Headless production compilation passes with `LIGHT_CONTROL_FRONTEND_DIR` pointing to the preserved existing frontend build; that proves source compilation only. The new frontend/native candidate has not been built. Playback domain checks pass 237 unit and 12 integration cases. The addressed/direct/batch Solo and held Flash/Group regression group passes five cases; a final shared-alias transient-inhibition case passes separately. Genuine failures were captured before correcting missing alias-region release bookkeeping and omitted held-release inhibition effects.

Focused frontend checks pass for Cue Recording ownership (53), playback bank (70), Cuelist window (27), unified pane (6), reducer (50), topology provider (6), Preload lifecycle (36), Preload settings (18), and sparse surface intent (10). Additional focused topology HTTP (5) and demo controls (19) checks pass. Final frontend typecheck retains 102 test/story/harness diagnostics, with production modules clean. These are focused checks, not a full frontend pass. Combined headless test compilation still fails: the latest discovery reports 356 errors after migration from 701. Core Preload fixtures/events have no current diagnostics, but runtime tests cannot execute until the entire test crate compiles.

Required next work includes the remaining frontend and backend fixture conversions; authenticated desk qualification of held-control sources and matching safe disconnect cleanup; accepted-frame/automatic Cue/timeline coordination with authoritative Timecode transports; alias event/OSC feedback validation; source cleanup and formatting. Regenerate demo and owned show fixtures through the documented writers, then perform a fresh isolated `npm run build:open`, default startup, save/reopen and non-destructive recovery checks. Continue all affected and downstream native acceptance, supported OSC and actual Art-Net/sACN output checks, with captioned internal-screen captures and test application restarts.

Intermediate logs live under the isolated worktree's `.artifacts/test/results/tl-714`, `.artifacts/tmp/TL714-*` and `.artifacts/tl714-*`. The release handover and current local checkpoints remain under the preserved primary checkout's `.artifacts/test/visual-inspection/release-acceptance/2026-10-08-fixes`. PLAINER TL-714 remains In progress; human acceptance remains separate.

### Desk control-source isolation follow-up

Identical declared surface/control IDs on different desks previously collapsed into the same held-control key. A genuine clone-only fail-first regression reproduced that collision. The corrected `PlaybackControlSource::scoped_to_desk` derives a stable bounded internal surface identity from the authenticated desk UUID and declared surface ID, retaining the control ID. The wire and queued operator-facing source remain unchanged. Live dispatch, direct Group controls, automation fader commands and delayed Preload execution apply qualification at the engine boundary. OSC replacement/unsubscribe/expiry cancellation derives the same identity from the owning session's desk.

Playback checks pass237 unit plus14 integration cases, covering independent desk holds/releases and source/surface cancellation, deterministic identity, maximal external surface IDs and invalid inputs. Headless production check13 passes after integration. Actual native disconnect behavior and software websocket teardown still require verification/implementation; this follow-up does not claim full desk or release acceptance.

### Final bounded pre-outage continuation

Solo event projection now selects regions through every assignment of the incoming target and includes all aliases of affected peer targets, including direct Cuelist activation. A focused regression covers an incoming assignment outside the region, overlapping regions and a peer alias on another page. Headless production check14 passes; regression execution remains pending full test compilation.

OSC address/expanded-control fixtures use canonical page-position validation and the saved surface registry; retired virtual aliases are explicitly rejected, occupied cells remain independent and conflicting expanded claims are rejected. Startup fixtures use sparse assignments and exact restored addresses; obsolete normalization expectations are replaced by central Solo restoration, activation-order and checkpoint-idempotency checks. Default and recovery assertions are preserved. Sparse frontend projection tests pass2/2, including holes and page99/position1000.

Combined headless test compiler check05 remains failing with311 errors, down from355 before this bounded batch. Startup, OSC and Solo projection files have no current compiler diagnostics, but this does not establish runtime test success. Full frontend typecheck was not rerun after the one-file frontend conversion. Existing remaining native build, Timecode frame bridge, software disconnect, demo regeneration and downstream acceptance requirements still apply.

Semantic startup recording fixtures now store the independent Speed assignment in page1.position101 and record Playback cues using explicit page1.position30. Rejected old programming-contract writes preserve page objects as well as show/programmer state; direct Cuelist recording preserves the unrelated page assignment. Test compiler check06 remains failing with307 other diagnostics, with none in this converted file. Runtime execution remains pending the full test-crate conversion.

Final read-only verification: `cargo test -p light-application --lib` exits101 with50 test-fixture compiler errors, chiefly timeline and macro fake Playback ports, event payloads, active-show and incremental compiler fixtures. This is an additional unfinished verification gate, separate from the307 headless test errors and101 desktop test/story/harness diagnostics. Log: `.artifacts/test/results/tl-714/application-unit-pre-outage.log`. Production compilation and focused passes do not establish this suite's runtime behavior.

### Additional offline-safe fixture checkpoint

Application test fixtures now use exact page-position identities and sources across event routing, timeline and macro ports, active-show persistence and incremental compilation. A new event regression isolates equal positions on different pages. Application check03 compiles and executes, but remains failing: twelve cases reported failures and eight Playback service tests ran for over60 seconds without completion; the owned test process was stopped. Log: `.artifacts/test/results/tl-714/application-unit-03.log`. This is not an application suite pass.

Four frontend fixture groups pass60 focused tests; full TypeScript diagnostics drop from101 to81, with none remaining in those groups. Report: `.artifacts/test/results/tl-714/frontend-four-fixture-groups.md`. Remaining backend/frontend migration, hang/failure investigation, authoritative isolated build and native/output acceptance are still required. No integration or release acceptance claimed.

### Resumed online implementation checkpoint

The user explicitly authorized finishing unification, merging the completed result into main and continuing release acceptance on the most recent combined main changes. Fresh upstream inspection showed origin/main at95b0eeae3 already an ancestor of this branch; recheck immediately before integration. TL-714 was unclaimed after its previous lease; the normal Ready-to-claim recovery restored this chat's claim through04:35UTC. No conflicting claim was overridden.

Application test invocation now initializes canonical artifact paths. A double-acquired test mutex caused the eight hangs; the fixture now captures its page once. Deterministic fixture timestamps and retired replay expectations were corrected. Latest full application library check08 passes864 tests, with2 ignored. The earlier finalization failures were invocation/environment failures from missing LIGHT_TMP_DIR.

Canonical selective import now traverses page assignment CueList, Group, Macro, Timecode and Dynamic dependencies; Dynamic embedded fallback/generated-lane identities follow duplicated live definitions. SpeedGroup letters remain fixed runtime identifiers. Schedules rebind only the page part of exact Playback addresses. Duplicating a page searches valid free pages1..99, with explicit full-capacity preview rejection and unchanged original/destination evidence. Commit8d10b0da4 contains these repairs.

Canonical wire, control mapping, Timecode convergence, event transport and output transaction fixtures retain exact addresses/sources and meaningful provenance/retirement/frame-rejection assertions. Headless test compiler check09 remains failing with242 other errors, reduced from307 before this continuation. Runtime execution of that crate remains pending. Full frontend typecheck remains67 fixture/story diagnostics; source/native build and complete frontend verification remain unfinished.

Remaining major implementation gates include per-software-instance held control ownership and literal held-page-change behavior; exact software action-socket disconnect cancellation; the accepted-frame/automatic Cue/Timecode authority bridge; remaining fixture conversions and obsolete-source cleanup. Complete isolated build/demo initialization and all requested downstream native, OSC and dual-protocol output acceptance before claiming the combined candidate ready. Human release acceptance remains separate.
