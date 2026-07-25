# Refactoring plan — chunk queue

This folder is the execution queue for the remaining ToskLight refactoring work. Every
chunk was verified against the actual code on 2026-07-23 (file:line references in each
chunk file were confirmed to exist at planning time — re-verify at execution time, the
tree moves). Architectural intent and contracts live in
[`../major-refactoring.md`](../major-refactoring.md) (bound to by
`docs/engineering/architecture-boundaries.md` and `refactoring-test-boundaries.md`);
the completed execution history is
[`../Done/major-refactoring-execution.DONE.md`](../Done/major-refactoring-execution.DONE.md).
Start execution sessions with the `/goal` in [`prompt.md`](./prompt.md).

## Workflow

```
pending/   ordered chunks, NN-slug.md — lower number first
doing/     the chunk currently being executed — AT MOST ONE file here
done/      finished chunks, each with a "## Result" note appended
```

1. **Claim**: `git mv` the lowest-numbered file from `pending/` to `doing/`. If `doing/`
   already has a file, finish or abandon that one first (abandon = move back to
   `pending/` with a note on what was learned).
2. **Check decisions**: chunks with an unresolved maintainer decision carry the
   `.ATTENTION.md` suffix (e.g. `06-….ATTENTION.md`) and contain a **DECISION NEEDED**
   section. STOP — ask the maintainer, record the answer in the chunk file, rename it
   back to plain `.md`, and only then execute. Conversely: when adding a new chunk that
   needs a decision, give it the `.ATTENTION.md` suffix.
3. **Execute** within the chunk's scope. Re-verify its file:line claims before editing.
4. **Gate**: run the chunk's verification steps; land only with no net new regressions
   against the baseline below.
5. **Finish**: append a `## Result` section (what changed, suite numbers, surprises,
   follow-ups filed as new `pending/` files), `git mv` to `done/`, commit.

Chunks may be split at execution time (e.g. `16a-…`): put the split files in `pending/`,
keep the ordering, note the split in the parent's Result.

## Suite baseline

- Last recorded full-suite result (2026-07-23, after the skipped-test/DMX-006/telemetry
  fixes landed): **274 passed / 13 skipped**; the only failing test is the user-dirty
  `product-demo` run.
- Current tree has 5 `test.skip` sites (chunk 02 unskipped both CUE-011 entries):
  3× MANUAL-019 @ui and PRELOAD-004 @supplemental-ui (deferred §5 features, out of scope
  here), the conditional desktop-smoke gate, and `product-demo` — a **known pre-existing
  failure** (ServerProvider/useServerConnection tears down its session and blacks out on
  the demo's Show Patch phase; diagnosis and the fix live in chunk 22, whose acceptance
  includes un-skipping it). It is deliberately skipped, not fixed, for now.
- **Before claiming the first chunk, run `npm run test:e2e` once and record the fresh
  numbers here** — that run is the binding baseline for "no net new regressions".

Fresh baseline: `274 passed / 13 skipped / 1 failed on 2026-07-23` — the single failure was
the known `product-demo` run. Updated 2026-07-23 (after chunks 02/02b/02c/03/03b): the two
CUE-011 skips became passes and `product-demo` is now deliberately skipped (see the skip-site
note above), so the binding expectation is **0 failed** with TIME-002/FIXTURE-002/GROUP-005
re-run in isolation before treating a failure as real. After chunk 03b-c the clean
full-suite result is **281 passed / 12 skipped / 0 failed** (chunk 03b-b added two
hardware encoder-modal PROG-002 cases, 03b-c three software-modal twins; PRELOAD-006
and the tests/31 `hardware_connected` OSC-subscribe poll have each flaked once
in-suite, both green in isolation).

## Standing rules (apply to every chunk)

- Read `AGENTS.md` first; `docs/help/**` is operator truth; `docs/testing/**` + root
  `tests/` are the acceptance contract; read `docs/acceptance-criteria.md` before
  persisted-data changes.
- `docs/engineering/api-rules.md` is binding for every touched route: intent-shaped
  writes, WS for desk live control, optional show-guard header, tolerant typing
  (accept+log unknown fields — use chunk 08's helper once it exists), request identity on
  edits. Bring violating routes you touch into compliance in the same chunk.
- The **OSC API is frozen**; keypad/command contract unchanged. REST/WS is internal —
  no protocol documentation updates required (decided 2026-07-23).
- Every v1 route deletion: grep for callers first (`apps/control-ui/src`, `e2e/bench`,
  root `tests/`, `crates/`, `desktop-smoke.mjs`); migrate bench helpers alongside.
- `deny_unknown_fields` is removed from wire types **as they are touched**, never as a
  big-bang sweep. Same for `crates/server/src/runtime/tests/` (82 modules): migrate
  feature-local when touching that feature.
- Gates per chunk: smallest relevant check first, then the full `npm run test:e2e`.
  `node tools/check-architecture.mjs`, `check-source-size.mjs`,
  `test-command-boundaries.mjs` where UI structure changed. `cargo fmt` (never standalone
  rustfmt). Land only with no net new regressions. Do not push unless asked.
- Known flaky-in-suite: FIXTURE-002 @restart, TIME-002 @ui, GROUP-005 @supplemental —
  re-run a suspected failure in isolation before treating it as real.
- Silent UI no-ops: suspect (1) scoped-store scope/generation guards, (2) stale
  show-revision `If-Match`, (3) a v1 mutation path that never publishes its v2 event.
- Preserve unrelated dirty-worktree changes; topic commits per chunk.

## Chunk order rationale

01–02 quick wins and the deferred CUE-011 bug, with 02b re-architecting persistence to
write-behind autosave (api-rules §8) on top of 02's event/revision guarantees;
03–06 server-owned show logic (§7),
with 03b adopting `Done/50` (deterministic multi-point spread rule — cheapest while the
spread paths are being consolidated anyway) and 09b adopting the one refactoring-shaped
item of `Next/64` (remove the direct programmer encoder type before migrating encoder
paths to WS);
07–08 small api-rules compliance enablers; 09–11 desk live control onto the WebSocket,
with 10 adopting the atomic server-owned multi-attribute mutation seam from `Next/71`
while leaving activation configuration and behavior in that feature plan;
12 route de-scoping; 13–21 the v1 retirement tail (events first, then data surfaces,
biggest one — 16 objects — in the middle); 22 capstone facade deletion; 23 housekeeping.
Dependencies are noted inside each chunk (e.g. 05 after 03; 10–11 after 09; 17 after 16;
22 after 13–21).

## Execution prompt

The canonical `/goal` prompt for execution sessions lives in [`prompt.md`](./prompt.md)
(one chunk at a time; parallel subagents where the work decomposes; isolated git
worktrees — explicitly based on the current `refactoring` head — for parallel file
edits, consolidated into the branch once the chunk's verification passes).
