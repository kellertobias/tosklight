# Intention programming — Opus 5.5 handoff

This is a handoff of unfinished implementation, requested by the user on 2026-10-01. Start with PLAINER TL-556 under TL-544. Read the current PLAINER item, its Working Log, acceptance criteria and readiness before claiming it. Remaining implementation issues are labelled **[Opus 5.5]**. No Opus agent has been launched by this handoff.

## Released ownership at handoff

- TL-636 is **Review/test revision 6**, unclaimed. Its domain acceptance evidence is complete; human acceptance remains separate.
- TL-556 is **Next revision 86**, unclaimed and unfinished, ready for an Opus agent to claim after a fresh readiness check.
- The outgoing Codex goal is **paused at the user's request to stop and hand over**, not complete. Resume/continue in the receiving agent after claiming work.
- The active work timer is stopped, all current workers are frozen/completed, and no Cargo or application build remains running.

## Source and authorization

- Worktree: `/Users/keller/.codex/worktrees/intention-based-programming/tosklight`.
- Branch: `codex/intention-based-programming`.
- HEAD: `45f4f42c8fd08f3a10061abb7e28c758f8d89548`, **plus the full dirty and untracked initiative**. HEAD alone is not the implemented state. Preserve it; do not reset, clean, stash away, or transfer only committed files.
- Primary checkout `/Users/keller/repos/tosklight` is protected. Do not implement or build there.
- User approved all work on this intention branch, including previously discussed concurrency, API and rebuild/restart work. The former usage 5% cutoff was removed.
- The production programming contract remains **0** in `crates/light/adapters/headless/src/runtime/startup_state.rs`. Do not raise it until TL-560 and the integration/cutover gates are satisfied.
- Current worker sources are frozen. Root Cargo sessions must be terminal before another build. No new application rebuild/restart, commit, push, schema, route or lock change was made in this handoff increment.
- Generated evidence and scratch belong under canonical `.artifacts/` paths. Use the artifact-path helpers. Scoped Cargo formatting snapshots are saved with the reports; never run broad formatting and restore other workers' files afterward.
- Read [the architecture plan](fixture-independent-programming.md), repository AGENTS, acceptance criteria, API rules and applicable PLAINER skill. Review/test is agent evidence, not human Done.

## Checkpoint being handed over

### TL-635 — already in Review/test

Same-driver, opt-in reached Required/Size graph operation discovery is implemented. Exact original registry and lexical routes are retained; pending-request APIs remain strict. The prior verified report is `.artifacts/tmp/tl635-reached-graph/verification-report.md`: Dynamics 646 and headless 1674 passed, 4 existing headless ignores, stable 12 owned paths and 2210 Rust/manifest inputs.

### TL-636 — current completed domain increment

Original multi-source Resume operand replay is implemented through:

- `PositionResumeOperandLocator`, issued only from an actual outstanding original Resume request.
- `begin_resume_operand` and `PositionResumeOperandContinuation` with NeedsMaterialization, OperandReady and Inactive results.
- Exact inherited Full, Graph, Stage and nested Resume goals; original registry/use/node/scope identity and bounded ancestry.
- Atomic SourceCohort member replay with all relevant original siblings and lower prefixes preserved. Stop before installing a child endpoint or applying the parent suffix.
- Removed or discarded enclosing goals stay inactive; no fallback to Full or revival of the other Resume side.
- Nested request forwarding and reached reports; the old single-source `at_resume_operand` guard remains.
- Stage matching enabled only for an actual enclosing Stage goal. Graph/cohort callbacks work independently, avoiding needless envelope suspension.
- Seven regressions cover genuine runtime hot-edit cohorts, lexical and registry isolation, choices/removal/coverage, dependency retries, terminal unwind/recovery, and removal of enclosing Graph/Stage goals.

The Graph-removal test's extra Required wrapper is explicitly an unwitnessed compositor unit assembly around a genuine runtime Resume. It does not claim sampler-produced cross-owner correspondence or physical correctness.

### TL-556 — captured raw Current memo completed; parent still unfinished

`CapturedProgrammingSources` now caches the original raw family, including absence, per captured logical fixture and owner. Addressed Current reads, exact family baselines and occurrence presence share it. Address-specific adoption, errors, validation, native models and original evidence remain independent. It is not a cache of fitted or adopted values.

Four new tests cover cross-address baseline reuse, absence and fresh captures, actual Live/Preload isolation, and legacy Zoom family requirements versus scalar adoption.

## Verification for this increment

- Final `cargo test -p light-dynamics --lib`: **653 passed**, 0 failed/ignored.
- Final `cargo test -p light-headless-runtime --lib`: **1678 passed**, 0 failed, **4 existing ignored** (two Position benchmarks, Color benchmark, Matter ports).
- Seven focused Resume regressions pass; four new captured-Current tests pass and are included in the full headless count.
- Stable **20 owned paths / 2214 Rust and manifest inputs** through verification; no unowned crate source writes or added whitespace.
- Scoped Cargo formatting and independent bounded source review completed. Current helper source is verified; the entire initiative is not cut over.

Canonical evidence:
- `.artifacts/tmp/tl636-multi-source-resume/verification-report.md`
- `.artifacts/tmp/tl636-multi-source-resume/independent-review.md`
- `.artifacts/tmp/tl636-multi-source-resume/joint-verification.json`
- `.artifacts/tmp/tl556-current-capture-memo/verification-report.md`

Failure logs are retained as correction history. Only the named final logs prove the final source. These are software regressions, not measured lamp matching, native Stage acceptance or frame-rate certification.

## First work for Opus: complete TL-556

### 1. Preserve the original driver goal while materializing synchronous peers

The current headless operation environment can consume actual pending graph operations. TL-635 now also identifies synchronous reached operations, but that alone cannot install a physical operation result into a completed peer while preserving its parent suffix.

A changing synchronous peer is not a constant. Its completed parent value is not the operation operand. Do not replace the whole parent value or fabricate a pending UUID.

The likely missing domain seam is opt-in materialization at an exact original reached Required/Size site on the same captured Full/Graph/Stage/Resume goal:

1. Validate the issued original locator/role/registry/use before using workspace.
2. At the real dependency-ready operation, expose an owned request with its original parameters, endpoints and route, even when angle algebra would otherwise finish inline.
3. Reuse pinned capture/Current and real generated request routing. Calling a callback that returns Requires is not by itself permission to invent request authority.
4. Resume the original goal after the fitted operation result and retain later non-idempotent suffix, masking and trace.
5. Do not resample clocks, Random or the Dynamic producer.

This discovered seam is recorded in TL-556 rather than implemented in the handoff. Make it a bounded dedicated issue if splitting ownership, with dependencies on TL-635/TL-636 and an explicit acceptance contract.

### 2. Consume TL-636 in the actual physical coordinator

Key paths:
- `crates/light/adapters/headless/src/runtime/output_scheduler/dynamic_projection/physical_adapter/position/cut_coordinator/operation.rs`
- Its `operation/local_stage.rs` and related environment machinery.
- Domain `crates/light/domain/dynamics/src/programming/composition/retained_family/position_program/`.
- Original authority `position_conditioning/origins/`.

The coordinator currently has bounded Full/Graph/Envelope/Mask/Constant environments. Integrate the new Resume driver and forwarding without weakening the old public guard. Correlate owners only through authentic producer occurrence/site/controller/target authority; a locator is owner-local.

Every operation must fit the complete mechanical peer set and every calibrated copy. Owner-local masks/envelopes are real stages; other changing peers retain their own inherited goals. No sampling through a synthetic node, matching by equal value/rank/path, skipping a suffix or speculative continuity publication.

### 3. Include static-only mechanical peers

The top compositor currently expects at least two peers with sampled programs. A mechanically coupled owner with only a captured static baseline still belongs to the complete physical fit. Add it without inventing a Dynamic or freezing an actually changing peer.

### 4. Preserve existing accepted-continuity rules

Read `.artifacts/tmp/tl556-heterogeneous-stages/integration-report.md` before editing:

- Complete accepted mechanical holds are valid.
- Incomplete prior coverage uses a complete captured baseline.
- Held diagnostic rows cannot replace accepted continuity, token or provenance, or create an owner entry.
- Failed or speculative finalization must preserve the previously accepted dependency and continuity state.

Do not reinstate the historical blanket skip of protected accepted holds described in older review notes. The final integration report owns the corrected policy.

### 5. Finish readouts, gestures and Pending ownership

- Production requested/achieved readouts must use accepted captured output identities, not unrelated fresh snapshots.
- TL-625 FinishGesture backend/client transport is implemented and verified. Actual controls still need correct gesture-end integration.
- Pending episodes must recreate their owned lanes/state; existing memo/rollback tests do not constitute the production lifecycle.
- Normal adoption evidence is not Pending correctness or physical accuracy.
- Preserve Live/Before/After frame identities, held/default rules, output controls and all-family injection.

## Remaining issue order

| Issue | Remaining scope |
| --- | --- |
| TL-556 | Position adoption/Dynamics/recording integration above |
| TL-548 | Assemble physical family adapters in Live and retained Preload |
| TL-594 | Accepted captured frames in native Stage and visible value consumers |
| TL-549 | Production Position encoders, modal, ranges/targets and gestures |
| TL-550 | Production semantic Color/Media controls |
| TL-554 | Direct/native Color pages and best-effort portable capture |
| TL-551 | Production Zoom degrees/Focus controls and modal |
| TL-596 | Exact-source semantic output deadlines and dirty-work measurements |
| TL-553 | Assembled native Stage and performance verification |
| TL-560 | Full persistence/import/replacement matrix and cutover prerequisites |
| TL-552 | Rebuild/run the actual application and operator acceptance |

Use actual dependency readiness, not table order alone. Existing Review helpers have substantial integrated source; do not rebuild them from scratch. Current PLAINER and newer checkpoint sections supersede historical notes.

## Operator and persistence requirements to retain

- Scope is Position, Color, Focus/Zoom. Gobos and other beam attributes remain as they are.
- Angle OR Target owns Position as one activation family. Switching disables the other representation; both may spread. A pan Dynamic must also carry tilt, including underlying static tilt.
- Targets may be Origin/XYZ or an existing referenced 3D point plus XYZ offsets. Mounting points and aim targets are independent. Moving a mount updates resolved motors without rewriting the target intent.
- Color presets/Cues/Dynamics store semantic intent, not the old lamp's channel set. RGB to RGBW/CMY replacement must not add an uncontrolled W channel.
- Easy color presents familiar emitters while storing the shared coordinate intent. Direct native color remains available on later pages, with honest best-effort portable appearance.
- UV has independent semantic demand; visible color fitting accounts for UV leakage. Missing UV/capability approximation is a quiet nonblocking notice/triangle, never a flow-blocking error.
- Compact Color: 2D picker, colored horizontal touch fader for White Blend, then White Balance and Expand buttons. White Balance has Temperature and DUV touch faders with white midpoint. Expanded modal uses existing title/modal components, a large hue ring plus saturation slider, both pages and approximation. SHIFT ranges work for picker/faders.
- No changes to regular upper area. Position and Focus special dialogs are modal-only. Position has stacked Pan/Tilt indicators, -90/reset/+90 actions and a square rate-control aim pad to the right, gentle near center; no moving-light picture, aim reference or extra lower status bar. Preserve existing preset UI.
- Focus is 0–100%; Zoom is opening degrees. Media reuses White Blend for grayscale and shared color tint while intensity stays independent.
- There is no browser Stage. Native Stage simulation and cadence are required.
- Portable show intent, group order, Programmer LTP, unpatched fixtures and software/hardware/OSC parity remain repository contracts.

## Pickup protocol

1. Read this handoff and fresh PLAINER TL-556/TL-544. Claim the first dependency-ready scope with your own agent/session identity.
2. Use the same full worktree. Preserve other owners and unrelated dirty work; coordinate serial Cargo and exact paths.
3. Read final reports, source hashes and existing tests before implementing. Domain locators prove original authority, not cross-owner fitting by themselves.
4. Complete a bounded verified chunk, append evidence to the Working Log and request Review/test only when that issue's criteria hold.
5. Continue through production wiring, persistence, native Stage and measurements. Do not raise the contract, mark the whole initiative complete or claim a demo is live from unit tests alone.
