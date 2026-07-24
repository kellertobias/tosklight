# 27 — Close the refactor and complete the CodeSafari handoff

Status: in progress.

## Purpose

This is the capstone for the architecture refactor. Run it only after every lower-numbered
refactoring chunk has either moved to `done/` with a `## Result` or has an explicitly recorded
deferral. Do not start the `28-test-bench/` batch until this chunk is complete.

This chunk has four ordered outcomes:

1. prove whether the refactor is complete;
2. publish a durable refactoring summary;
3. integrate the refactor onto the current `main` safely; and
4. turn the existing `.tour/` material into a complete, validated CodeSafari handoff.

The repository already contains an architecture overview, boundary rules, state-ownership matrix,
code tour, extension recipes, test map, and an initial CodeSafari. Audit and improve those sources;
do not create a second architecture narrative that can drift from them.

## Scope boundaries

- This is primarily an acceptance and documentation chunk. Do not hide a newly discovered
  production defect inside tour work.
- A small, clearly refactoring-owned omission may be fixed here only if it remains one reviewable
  slice with focused coverage. Otherwise add a lower-numbered follow-up chunk, record the blocker,
  and stop this chunk.
- Do not begin any work from `28-test-bench/`.
- Preserve unrelated dirty files. Stage exact paths; never use a blanket add or cleanup.
- Do not push, delete the `refactoring` branch, or rewrite a remote branch unless the maintainer
  asks separately.

## Phase 1 — Refactor completion audit

### 1. Establish the exact baseline

1. Read all refactoring `Result` sections, `docs/plans/major-refactoring.md`,
   `docs/plans/refactoring/README.md`, and the current files under `docs/engineering/`.
2. Verify the current branch, worktree, local `main`, `refactoring`, their merge base, and the
   complete `main...refactoring` commit and path diff. Record any unrelated dirty paths before
   touching files.
3. Confirm `doing/` is empty and no lower-numbered pending chunk remains. A high completion
   percentage or a passing focused suite is not evidence that this gate is satisfied.
4. Re-run source inventory against the current tree; do not rely on old file counts, old
   `useServer()` counts, or paths quoted in earlier handoffs.

### 2. Audit against the intended architecture

Build a short evidence table with `complete`, `deferred`, or `blocked` for each applicable area:

- executable and composition roots;
- Rust dependency direction and public crate boundaries;
- one-action/one-authority behavior across software, keyboard, OSC/attached hardware, HTTP, and
  automatic runtime sources;
- typed commands, outcomes, semantic events, replay/no-change behavior, and compatibility adapters;
- active-show mutation ordering, lossless unknown-field preservation, migrations, revisions,
  atomicity, Save As/export, and recovery;
- deterministic render, contribution arbitration, fixture projection, DMX output, scheduler health,
  and the tick-budget contract;
- frontend feature ownership, narrow projections, dormancy, loading/error states, optimistic
  reconciliation, event gaps, authority replacement, and removal of broad refresh paths;
- Tauri lifecycle, sibling Hardware Controls ownership, shared desk state, packaging, and the
  real desktop path;
- accessibility, keyboard/focus behavior, exact operator wording/geometry, and parity between
  software-only and hardware-connected layouts;
- configuration, logs, readiness/bootstrap, operational debugging, and deployment/build commands;
- test layering, deterministic fixtures, architecture/source-size ratchets, and remaining
  intentionally skipped or flaky coverage.

Use the refactor skill's quality rubric as a completion checklist, but apply only items justified by
ToskLight's actual deployment and threat model. Separate:

- behavior-preserving refactoring;
- correctness or performance fixes made during the refactor;
- product changes;
- compatibility surfaces deliberately retained; and
- genuinely deferred work with an owner or trigger.

### 3. Acceptance gates

Start focused, then run the current repository-wide gates documented in
`docs/engineering/build-and-test-commands.md`. At minimum:

```sh
npm run test:architecture
npm run test:unit
npm run test:e2e-api
npm run test:e2e-ui
npm run test:e2e
npm run test:desktop-smoke
npm run storybook:build
npm run test:ui-package
npm run manual
npm run open
curl -fsS http://127.0.0.1:5000/api/v2/readiness
```

Also run the release output benchmark and any focused migration/recovery checks required by the
audit. Record exact results, skips, flakes re-run in isolation, reference hardware, and limitations.
Inspect `.artifacts/runtime/light-data/light-server.log` during the desktop acceptance.

Completion requires more than green tests:

- exercise one representative programming action through software and OSC/attached hardware;
- load or migrate a legacy show and verify lossless save/reopen behavior;
- verify a cue jump reconstructs the correct tracked stage state;
- verify a show switch, reconnect, secondary screen, and sibling Hardware Controls app;
- inspect the packaged/bundled path rather than only a browser build; and
- compare the final implementation and documentation against every literal refactor acceptance
  criterion.

If an environment prevents a required check, record that as unverified evidence rather than calling
the refactor complete.

## Phase 2 — Durable refactoring summary

Add `REFACTORING-SUMMARY.md` at the repository root. It is the human handoff, not a commit dump.
Base claims on the audited `main...refactoring` diff and current verification evidence.

Include:

1. executive summary and refactor scope;
2. architecture before and after;
3. independently runnable components and their contracts;
4. major changes by backend/application, persistence, engine/output, frontend, desktop/hardware,
   tooling/tests, and documentation;
5. correctness, performance, and operator-facing defects fixed;
6. compatibility decisions for show files, desk data, OSC, keyboard/control surfaces, and UI;
7. what common changes are now easier and which extension seams support them;
8. exact verification evidence and runtime observations;
9. known risks, retained adapters, deferred debt, and deliberately excluded product work; and
10. links to `docs/engineering/`, `.tour/`, the refactoring execution history, and the next
    test-bench batch.

Keep `docs/engineering/` authoritative for living architecture rules. Update stale paths, commands,
or claims there in the same documentation commit. Do not copy volatile file counts or line numbers
into the long-lived summary unless they are clearly labeled as the final audit snapshot.

## Phase 3 — Rebase and local `main` integration

Do this only after all refactor work, summary material, and acceptance evidence are committed and
the worktree contains no unaccounted changes.

1. Refresh and verify the intended `main` ref. Record the pre-rebase `refactoring` commit so the
   operation is recoverable.
2. Rebase `refactoring` onto the current `main`. Resolve conflicts by preserving both current-main
   behavior and the documented refactor contracts; do not discard unrelated work to make the
   rebase pass.
3. Re-run architecture, unit, full E2E, desktop-smoke, build, and any conflict-affected focused
   checks on the rebased commit.
4. Switch to local `main` and integrate `refactoring` with a fast-forward-only merge. If this is not
   a fast-forward, or if `main` moved during verification, stop and report the exact refs instead of
   creating an unplanned merge.
5. Continue subsequent local work on `main`, but retain the `refactoring` branch as a recoverable
   reference. Do not push either ref without explicit authorization.

Append the exact before/after refs and post-rebase verification to this file's `## Result` before
moving it to `done/`.

## Phase 4 — CodeSafari completion

### Existing material to preserve and audit

The current `.tour/` already includes:

- `Orientation`;
- `One Action, End to End`;
- `Add a Capability`;
- `A Frontend Slice in Detail`;
- `Rust by Example`;
- component guides for Control UI, UI Library, Tauri apps, Programmer,
  Backend/Application, Engine/Output, Help Generator, and Testbench; and
- domain and architecture glossaries.

Do not recreate these under slightly different names. Extend, split, or retitle them when the
requested learning path is not yet explicit. `docs/engineering/` remains normative; safaris are
guided paths through real code.

### Required safaris

Use these reader-facing titles unless the implemented content reveals a more precise domain term:

| Safari | Existing page or new page | Required path and questions |
| --- | --- | --- |
| **One Value: From Desk Input to DMX and Back** | Extend/retitle `One Action, End to End` | Follow one semantic value from software and OSC key input through command parsing, ordered selection/group resolution, Programmer LTP state, application outcome/event, engine contribution/arbitration, fixture projection, DMX frame/delivery, and frontend feedback. Include no-change, conflict/replay, unpatched fixture, logical head, and fine-byte behavior. |
| **Cue Tracking and Goto: Reconstructing the Stage** | New | Start with recorded cue data, follow tracking compilation and runtime state, then show how GO, GOTO/jump, release/off, assert, and backward navigation reconstruct the correct values without relying on having played intervening cues. Explain which cue values can turn off or release, HTP/LTP/ownership, MIB, fades, automatic transitions, and tests that prove direct jumps. |
| **State Ownership to Pixels: Snapshots, Events, and Repair** | Extend `A Frontend Slice in Detail` or split if it becomes too long | Trace all six state lifetimes, composition/provider ownership, hydration, narrow subscription, optimistic overlays, response/event races, revision conflicts, sequence gaps, snapshot repair, reconnect/show replacement, dormancy, and why stale bootstrap fallbacks are forbidden. |
| **Ordered Selection: Fixtures, Heads, Groups, and DEGRP** | New | Follow software, command-line, OSC, and Group selection through stable fixture/head identity and ordered membership. Cover ranges, toggle/add/subtract, missing IDs, stored-empty groups, overlapping groups, unpatched fixtures, selection-before-Show event ordering, and why order affects spreads. |
| **Value Spreading: Selection Order and Multi-Point Curves** | New | Follow a two-point and multi-point spread from encoder/modal input to server validation, canonical control points, deterministic interpolation, live Group references, membership edits, DEGRP/freeze, cue/preset storage, migration, engine recall, and software/hardware/OSC parity. Use the accepted spread resolver tests as executable landmarks. |
| **The Portable Show: Load, Migrate, Revise, Compile, Save** | New | Explain `.show` versus `desk.sqlite`, lossless raw bodies plus typed deltas, profiles and immutable snapshots, candidate/transaction/CAS revision, backup and atomic commit, prepare/install lifecycle, runtime generation, Save As/export/selective import, legacy migration, malformed-show recovery, and why unknown data survives. |
| **Recording and Live References: Groups, Presets, and Cues** | New | Start with Programmer values and record/update them into Groups, Presets, and Cues. Explain embedded values versus live Group/Preset references, dereference/DEGRP/freeze behavior, selection order, update menus, undo, revision conflicts, recall after membership/value changes, and Highlight never being recorded. |
| **Fixture Semantics: Attributes, Modes, and DMX Channels** | New | Follow a fixture package/profile revision through Patch, selected mode, logical heads, semantic attributes, attribute groups, activation groups, defaults/Highlight values, splits, virtual intensity, multipatch, unpatched state, fixture compilation, channel/fine-byte encoding, and output binding. Make the distinction between an attribute and a DMX channel explicit. |
| **Playback Runtime: Cues, Masters, Speed, and Arbitration** | New, cross-link the cue-tracking safari | Follow a Cuelist assigned to a page/playback through GO, pause/back/goto, fader and executor input, contribution production, intensity HTP, non-intensity LTP/ownership, independent overlapping Group Masters, Speed Groups, Grand Master, Highlight, Blackout, automatic transitions, and event publication. Distinguish current-page from explicit-page addressing. |
| **Rust and Tauri for TypeScript Developers** | Extend/retitle `Rust by Example` and cross-link `Tauri Desktop Apps` | Teach only the Rust used here: ownership/borrowing, enums and exhaustive matching, `Result`, traits/generics/`dyn`, `Arc`/locks/`ArcSwap`, async tasks/cancellation, ports, serde/wire generation, Axum composition, Tauri commands/events/windows, and the typed desktop bridge. Contrast each concept with the nearest TypeScript/React mental model and include lifecycle/error examples. |

The first nine behavior safaris are independent learning paths, but they must cross-link rather than
repeat shared explanations. Keep `Add a Capability` as the extension recipe and `Orientation` as
the shortest onboarding route.

### Glossary concepts to add or deepen

Audit existing entries before adding anything. At minimum, make these concepts directly linkable:

- attribute versus DMX channel;
- attribute group versus activation group;
- fixture, logical head, mode, split, multipatch, virtual intensity, profile revision, and
  unpatched fixture;
- ordered selection, stored-empty group, absent group, missing range ID, live Group reference, and
  DEGRP/dereference/freeze;
- Preset scope/filter, embedded value, live Preset reference, Record, Update, and undo;
- cue tracking, block/assert/release/off semantics, GOTO reconstruction, MIB, and automatic
  transition;
- Cuelist, Playback, current-page versus explicit-page addressing, Group Master, Playback Master,
  Speed Group, Grand Master, Highlight, Blackout, HTP, LTP, and ownership;
- Programmer, Preload, capture mode, per-user versus per-desk state, and the shared command line;
- semantic attribute value, contribution, arbitration, transition, fixture projection, DMX frame,
  output route, and tick budget;
- portable show, desk database, profile snapshot, lossless body, revision, transaction, candidate,
  migration, prepared install, EngineSnapshot, and runtime generation; and
- action context, no-change, replay, idempotency, typed event, projection, optimistic overlay,
  sequence gap, snapshot repair, authority replacement, and compatibility adapter.

### Safari authoring contract

Each safari must:

1. begin with the operator-visible behavior and name the authoritative `docs/help/` and
   `docs/testing/` contracts;
2. follow actual repository-relative paths and stable symbols from executable entry point through
   every owning layer;
3. distinguish authoritative state, projections, overlays, persisted data, and transient runtime;
4. include one important failure/edge path and the code that owns recovery;
5. link executable tests that protect the behavior;
6. end with a small exercise that can be completed without changing production state;
7. identify deliberate compatibility adapters without presenting them as extension points; and
8. avoid brittle line-number references, duplicated architecture prose, speculative future
   behavior, or claims not verified against current code.

Add source `@tour` anchors only where they genuinely improve navigation and do not turn production
code into narrated documentation. Prefer stable symbol/path links when CodeSafari supports them.

### CodeSafari verification

```sh
npx --yes "@tobisk/codesafari@1.0.0" validate .
npx --yes "@tobisk/codesafari@1.0.0" export . --out .artifacts/generated/codesafari
npm run pages:generate
```

Then inspect the exported safari visually:

- every page is reachable and ordered intentionally;
- all file, symbol, glossary, component, and cross-tour links resolve;
- code excerpts match the current rebased tree;
- narrow and wide layouts remain readable;
- no generated site is committed outside the repository's documented artifact path; and
- `README.md` accurately lists the final tours and launch/validation commands.

## Definition of done

- Every lower-numbered refactor chunk is resolved and the completion audit has no unreported
  blocker.
- The full acceptance evidence is green or each unavailable check is explicitly labeled
  unverified with risk.
- `REFACTORING-SUMMARY.md` gives a new engineer an evidence-backed before/after handoff.
- `docs/engineering/` contains no stale migration-era claims presented as current architecture.
- The refactor is rebased and local `main` is fast-forwarded only under the safe integration rules
  above.
- All required safaris and glossary concepts are covered without duplicating the authoritative
  engineering docs.
- CodeSafari validates, exports, and renders correctly.
- A `## Result` records exact refs, checks, limitations, safari pages changed/added, and any deferred
  work before this file moves to `done/`.

## Progress — 2026-07-24

### Baseline and completion audit

- Branch: `refactoring`; pre-capstone head `f7a86f59`; local `main` and merge base
  `5c92eb07`. `main` has not advanced independently, so the eventual rebase is expected to be a
  no-op before fast-forward integration.
- The audited `main...refactoring` history touches 2,840 paths (461,097 additions / 99,922
  deletions). Those figures include generated/manual/fixture-library history and are recorded only
  as the final snapshot, not as a quality metric.
- `doing/` was empty and no lower-numbered pending implementation chunk remained. The two completed
  plans lacking the queue's literal `## Result` heading were normalized; the resolved 02b-c
  write-behind decision remains explicitly recorded as a maintainer resolution.
- Current inventory: zero production `useServer()` calls and zero served application `/api/v1`
  references. The two source-tree `/api/v1` strings are the Forgejo publishing API and the
  architecture guard rejecting retired ToskLight endpoints.

| Area | Audit result | Evidence or limitation |
| --- | --- | --- |
| Executables and composition | complete | Thin Rust entry point; server runtime composition; two separately built Tauri apps. |
| Dependency direction and crate boundaries | complete | `npm run test:architecture` passed with an empty hard-limit ratchet. |
| One action / one authority | complete | Paired API/UI, OSC, desk WebSocket, automatic Playback, and output acceptance are green. |
| Typed commands, outcomes, events, repair | complete | v2 wire generation, request replay/no-change, typed event cursors, gap repair, and correlated frontend writers are covered. |
| Portable show lifecycle | complete | Lossless bodies, migration riders, CAS revisions, atomic candidate/install, Save As, revision copies, selective import, and malformed recovery are covered. |
| Engine and output | complete for the required floor | 32 packed universes at 100 Hz passed with zero deadline misses; 64/120 remains a target and reached 68.8 Hz on this reference run. |
| Frontend ownership and responsiveness | complete | Narrow projections, dormancy, explicit loading, optimistic reconciliation, authority replacement, and no broad v1 provider remain. |
| Desktop and Hardware Controls | complete | Desktop smoke passed both process cases; `npm run open` built both apps and readiness returned `status: ready`. |
| Accessibility and operator parity | complete with explicit product skips | Complete UI suite passed; three MANUAL-019 and two PRELOAD UI contracts remain deliberately skipped product work. |
| Operations and developer commands | complete | Readiness/bootstrap/log guidance and root npm workflows are current; runtime log showed a clean active-show compile and ready bind. |
| Test layering and ratchets | complete with explicit product skips | Unit/API/UI/full E2E, source-size, dependency, generated-contract, benchmark, and desktop layers are present. |
| Engineering and human handoff | in progress | Living stale v1/write-behind claims corrected; `REFACTORING-SUMMARY.md` and all required safaris/glossary entries added. |

Behavior-preserving work is separated from defects fixed during the refactor in
`REFACTORING-SUMMARY.md`. Deferred product features and deliberately retained OSC/integrator/
desktop/test adapters are named there rather than presented as incomplete migration seams.

### Fresh acceptance evidence

- `npm run test:architecture`: passed.
- `npm run test:unit`: all frontend/build/Rust checks passed; the sandbox-blocked CITP loopback
  cases passed separately with socket access (5 passed).
- `npm run test:e2e-api`: 86 passed / 1 skipped.
- `npm run test:e2e-ui`: 104 passed / 5 skipped.
- `npm run test:e2e`: 287 passed / 9 skipped.
- `npm run test:desktop-smoke`: 2 passed.
- `npm run manual`: 40 Markdown pages; verified 140-page PDF and offline HTML/ZIP.
- `npm run open`: both Tauri applications built; app-owned server readiness was healthy and the
  runtime log contained no startup error.
- Release output benchmark: required 32-universe/100 Hz floor passed with zero dropped/deferred/
  missed ticks; 4- and 8-universe/40 Hz goals passed. The 64-universe/120 Hz target did not pass
  (68.8 Hz). CPU, allocation rate, socket delivery, and sound analysis remain explicitly
  unmeasured/accounted in the report.
- Release show-mutation gate: 120 fixtures p95 0.945 ms; 1,200 fixtures p95 1.053 ms;
  structurally shared, full-compiler equivalent, size-independent, and below the 5 ms ceiling.

The representative programming, OSC/attached-hardware, legacy/recovery show, direct Cue jump,
show-switch/reconnect, secondary screen, and sibling Hardware Controls paths are covered by the
fresh acceptance and desktop runs above.

### Open acceptance blockers

1. The pinned CodeSafari validator/export could not be executed because running a third-party npm
   package with network access in the private workspace requires explicit maintainer approval.
   Validation, export, visual inspection, and `npm run pages:generate` remain unverified.
2. `npm run storybook:build` fails before Storybook starts because `package.json` targets the
   nonexistent workspace `@tosklight/ui`. The ignored `packages/ui/` directory contains only old
   `dist/` and `storybook-static/` artifacts; no tracked package source or manifest exists.
   `npm run test:ui-package` is unavailable for the same reason. This predates the capstone and
   needs a product/repository decision rather than reconstructing a library from generated output.
3. Rebase and local `main` integration wait until the handoff blockers are resolved and committed.
   The unrelated dirty worktree remains recorded and excluded from every staged path; integration
   can use a temporary clean worktree so those files are never stashed or rewritten.
