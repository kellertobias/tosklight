# Unified Playbacks (TL-714)

## Contract and declared pre-release break

Pages 1–99 each have positions 1–1000. Storage is sparse: a page owns its assignment definitions directly. There is no global regular Playback pool, virtual bank, page-slot indirection or compatibility address alias. An assignment address is `page.position`; all controls displaying that address operate the same target and authoritative runtime.

TL-714 deliberately breaks pre-release persisted and transport compatibility. New portable shows use schema floor 10 and `light.playback_contract=1`; new desk layout databases use schema 17. Incompatible shows, saved runtime, named revisions and Undo/Redo content must be inspected before writable use and refused without changing original data. An incompatible desk starts a separate recovery subtree instead of rewriting its database. Recovery must be visible, preserve originals and permit a separate fresh show. This is recovery, not conversion of old Playback data. Repository-owned shows and generators must be regenerated through their documented writers before acceptance.

## Operator decisions

- Preload range union applies to all pages by default; optional explicit page restrictions narrow it. Unmatched positions remain Live. The permanent-action set and Temp toggle semantics stay as before; Flash and fader movement remain Live. Programmer capture remains independent.
- Typed, keyboard and shared OSC commands use the visibly selected surface's effective page for an unqualified position. Explicit addresses remain fixed.
- Solo membership belongs to the exact Playback address permanently started. A Timecode starts Cues directly and those Cue starts do not arbitrate Solo. A Timecode assigned to a member Playback stops when another member actually starts permanently. Another alias of the same target does not inherit membership.
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


### Current backend verification checkpoint (2026-10-11 03:35 UTC)

Frontend Solo grid selection, inert configuration gestures, joined amber fences and general cross-page membership management pass 4,371 tests in 583 files plus TypeScript checks; commit `a0e5546d3` is verified on origin. Native geometry/touch acceptance remains pending. The regenerated packaged demo uses schema 10 and 44 unified assignments, with canonical generation/import/reopen validation; ordinary embedded-default startup is included in the next runtime run.

Runtime all03 reports 1,986 passed, 25 failed and five ignored. Corrective command-event, Solo partial-failure feedback, canonical fixtures and capture changes are being rerun in all04; no remaining failures have been waived. Complete restored Solo checkpoint engine tests pass three cases. A real Timecode-owned Cue reproduced stale same-frame DMX (255 despite stopped transport); the isolated test-only settle/reconcile/same-sample render proof passes with zero output. The capture callback now applies that verified ordering before render and retains accepted event claims. Private Dynamic persistence fail-first proves lost activation ordinal; its private serializer was repaired without exposing internal order in public runtime JSON. Combined cold startup restore still needs wiring and regression verification.

Implementation remains isolated and unmerged. Required next gates retain source/contract cleanup, complete backend checks, authoritative native rebuild, actual gesture/OSC/Art-Net/sACN evidence and downstream release scenarios. The original candidate and live show data remain preserved.


### Verified checkpoint 2026-10-11 04:00 UTC

The complete engine gate passes 416 unit and 13 integration tests; application passes 867 unit plus three integration tests, with two ignored; programmer passes 182. Headless all06 passes 2,017 tests with five ignored. All07 after command-key retirement passes 2,016 with one stale VPBK expectation now corrected, pending rerun. New demo held-page-change/source-isolation tests pass 23 cases; software keypad retirement passes 37; API-boundary tests pass 114 and TypeScript checks pass. Source-size and architecture gates pass after bounded cleanup and inventory regeneration.

Same-frame Timecode-stop DMX, complete Cue/Dynamic cold Solo restoration, private Dynamic activation-order persistence and software socket cleanup now have passing backend regressions. Demo and compact-rig assets were regenerated and reopened canonically. Reachable obsolete Dynamic configuration and Speed fader normalization are being removed before native acceptance. Commits 8f2ab1db5 and 904f2d5a1 preserve verified demo holds and isolated OSC fallback on origin.

Implementation remains isolated and unmerged. Combined frontend verification, authoritative native rebuild, geometry/gesture/OSC/Art-Net/sACN evidence and downstream latest-main release acceptance remain pending. Restart owned test applications between runs and capture captioned internal-screen evidence. Original candidate and live data remain preserved; human release acceptance remains separate.


### Verified backend and native startup checkpoint 2026-10-11 04:18 UTC

Backend finalization is committed and pushed as `827f5fd807445f2e23f8636145caf5660316c5bf`. Engine: 416 unit plus 13 integration; application: 870 unit plus 3 integration (two ignored); headless: 2,018 passed (five ignored); Programmer: 182; Playback: 238 unit plus 25 integration; Show: 135 (one explicit generator ignored); Wire: 123 plus one integration; Control: 22. New candidate deletion protection refuses deleting assigned Cue/Group/Macro/Timecode targets before persistence, while atomic clear/retarget plus deletion and Dynamic embedded fallback remain valid.

Desktop 4,381 tests and typecheck, additional workspaces 1,372 tests, bench 200 tests, source-size ratchet, and manual PDF/offline HTML verification pass. Remaining executable scenario helpers are being converted and are not yet accepted as current-contract coverage.

Authoritative isolated `build:open` completed with exact worktree bundle and server 5034/OSC 19014 using isolated native-acceptance data. Readiness is healthy; bootstrap takes approximately 4 ms. The app was moved to Built-in Retina Display through the native Window menu. Its frontend is blank: packaged startup reproduces React undefined-component failure because default `playbacks` pane registration still used the retired kind. Fix and rebuilt native proof are required. Captioned failure evidence is saved under the primary `.artifacts/test/visual-inspection/release-acceptance/2026-10-08-fixes/TL714/native-startup-blank-captioned.png`. No UI, OSC, Art-Net/sACN acceptance or main merge is claimed.


### Current acceptance checkpoint 2026-10-11 05:30 UTC

The implementation remains isolated and unmerged. Reviewed commits through `b8d7f7f9b` are pushed; the exact upstream hash is recorded in the Working Log. Desktop passes 4,451 tests in 586 files, hardware controls 42, bench 250, and the other TypeScript workspaces passed their type/unit gates. Application now passes 872 unit plus three integration tests (two ignored); engine passes 416 unit plus 13 integration tests. The complete architecture gate passes after canonical narration and full discovery fixes; Playwright lists 440 cases without discovery errors. These are source and browser gates, not native release acceptance.

Actual old-frontend MEDIACUE-002 fail-first reproduces missing Playback preview after the canonical fixture reaches the UI. The committed artwork provider batches only visible eligible single-Cue targets once per bank. Actual VPB-007 passes through policy revision rejection, cold restart, cross-page numerical output, two desks and OSC Solo release audit. UI-driven DMX-001/002/003 pass against real isolated Art-Net and sACN datagrams, exact level conversion and wire headers. Final native dual-protocol trajectory acceptance remains required.

Explicit Timecode Cue Start and State Start now reactivate retained Off Cues, proven by service-level exact-Cue, resolved-output and timing regressions. Ordinary GO activation settings, manual Off between starts, discrete reconstruction suppression and direct Cue Solo bypass remain preserved. Canonical compact current-page OSC receipt capture and lifecycle cleanup are still being completed alongside explicit-page routes; no retired aliases are restored.

Visual review rejected the first Main scrolling implementation as final acceptance: eventual access did not prove initial fader identity/action visibility. A bounded software-Main compact layout and combined toolbar are being verified before native rebuild. Expanded Solo partial joins and sparse pane geometry have browser proofs, but native screenshots, simultaneous holds, page sources, offline layout copying, final media pixels and output trajectories remain open. Rebuild and restart owned applications, use only the internal screen, preserve captioned original pixels, then integrate and rerun affected release scenarios from the latest main. Human release acceptance remains separate.


### Current native acceptance checkpoint 2026-10-11 07:25 UTC

Implementation remains isolated and unmerged. Reviewed commits through `1b67076c8a311bd4fc8a22e77b64aa9b145ea31d` are pushed. The prior candidate `e975525a12d3cfd3da1695c1cdf84f37ddc703f8` passed architecture (147 tests plus static gates), seven workspace TypeScript checks and bench types, and nine freshly restarted actual Art-Net/sACN, OSC, held-control and Screen-layout cases with no failures, skips or retries. Full desktop earlier passed 4,477 tests; the later label-only correction passed 47 focused tests and TypeScript. Canonical native builds 14 and 15 restarted the owned isolated application successfully; readiness is healthy. These results do not establish full release acceptance.

Captioned Built-in Retina evidence proves ordinary recording into page 2 positions 101 and 102 succeeds after fixing independent Cuelist allocation and validating the actual sparse assignment catalog before commit. Distinct Cuelists with independent pool numbers 1 and 2 are confirmed through production projections. The original failed show is preserved; a separate clean retest show is used. Personal 20 by 14 grid arrow navigation reaches position 680, and maximize/restore retains its original footprint and neighbours. Exact native Solo Region 2.101/2.102 switches permanent typed GO TO starts, leaving only the second Playback running. Current pane settings visibly use Playbacks after the label correction.

A further native gate found a blocking frontend decoder omission for valid Timecode assignments: Main and pane reject target.type timecode. Macro is also absent from that decoder. The valid assignment succeeded; an initial suspicion of postcommit HTTP failure was disproved. A later, separate malformed test setup update correctly returned HTTP 400 because hidden buttons were not None. Corrected failure evidence supersedes that initial inference. The supported-target decoder must be repaired and rebuilt before native assigned-Timecode/Flash Solo and Preload tests continue. The encoder-center operator question remains unanswered; no dependent behavior is assumed.

Current native evidence and hash ledger are under the preserved primary checkout release-acceptance `2026-10-08-fixes/TL714` directory; executable reports are in the isolated worktree `TL714-verification` directory. Remaining native, output-trajectory, media/Pixel and broader release scenarios are not claimed complete. Main integration is authorized after implementation and acceptance gates finish; fresh fetch and latest-main rebuild/retest remain required. Human release acceptance remains separate.


### Current automation and native checkpoint 2026-10-11 08:22 UTC

Reviewed automation fixes are committed and pushed as `65dd1525731bf4e88c22c5d3366f69b653302bce`; separate CSV test synchronization and Preload keyed-draft commits are `1db795515` and `85bcac343`. Macro/Timecode assignments round-trip through strict frontend codecs, authoritative object-event choosers and actual transport card state. Unsupported fader controls are unavailable in normal, expanded and encoder presentations; explicit authoring resets that capability, while invalid stored data is rejected without migration. Four saved-data cases cover explicit and defaulted invalid Macro/Timecode faders, prior active-show and file preservation, visible startup recovery and subsequent valid load.

The combined desktop suite passes 4,514 tests in 597 files; shared UI passes 271; seven workspace TypeScript checks and bench types pass, with final desktop typecheck after keyed Preload draft hardening. Headless passes 2,036 tests with five existing ignored; Playback passes 243 unit plus 25 integration. Application passes 874 unit plus three integration with two ignored; engine passes 416 unit plus 13 integration. The initial combined headless run's 17 failures came from lifecycle fixtures intentionally using contract1 and a stale feature2 marker expectation after current assigned recording gained independent Cuelist pool ownership. Corrected test-only fixture support preserves explicit historical contract2 acceptance and contract1/2 rejection boundaries; production guards are unchanged. Final architecture/static checks and Cargo formatting pass. The regenerated manual verifies 390 PDF pages/273 embedded images and 63 HTML pages/265 images. Logs preserve the earlier CSV readiness and Preload snapshot timing failures; the latter is source-supported hardening without a deterministic isolated fail-first claim.

Canonical Native16 build:open exited0, restarted the owned isolated5034/19014 application and retained the active retest show. Built-in Retina captioned screenshots prove closed Screen1 Page2 command context remains visible, and valid saved Timecode2.103 no longer blanks the banks. Exact Solo Region2.101/2.102/2.103 permanent native GO observes Timecode Playing then Stopped when2.101 starts. Actual2.5-second native held Flash observes196 transport samples remaining Playing before, during and after release; this narrow proof does not claim actual dual-protocol Cue output. Original2.101 configuration is restored; Timecode is stopped through its existing authenticated transport endpoint. Two exploratory Off command strings were rejected as operator input; no command contract or parser repair is inferred from them.

A new native-discovered clear gap remains: Running & Output's modal counts/releases Cue/Dynamic owners but omits running Timecodes/Macros, so All Off can be disabled during an assigned Timecode's actual Playing state. A bounded repair using existing supplemental authority is underway. Its separate Running pane already includes automation; the command bar No Timecode refers to routed SMPTE-source frames and is recorded as ambiguous, not a transport failure. Encoder-center operator question remains unanswered. Main remains unmerged; native Preload/remaining layout controls, fresh supported OSC and actual Art-Net/sACN output, latest-main rebuild/retest and broader release scenarios remain required. Human acceptance is separate.
