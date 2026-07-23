# Refactoring — remaining steps (moved)

This plan has been decomposed into an execution queue of small, independently landable
chunks, each verified against the code on 2026-07-23:

**→ [`refactoring/README.md`](./refactoring/README.md)** — workflow, suite baseline,
standing rules, and the `/goal` prompt for execution sessions.
**→ [`refactoring/pending/`](./refactoring/pending/)** — the ordered chunk files.

Historical context:

- Completed execution plan: [`Done/major-refactoring-execution.DONE.md`](./Done/major-refactoring-execution.DONE.md)
  (suite 267 passed / 19 documented skips at close).
- Architectural intent: [`major-refactoring.md`](./major-refactoring.md).
- Binding API rules: [`../engineering/api-rules.md`](../engineering/api-rules.md).
- Sections 2 (skipped-test residues), 3 (DMX-006), 4 (playback telemetry), and 6
  (pre-existing failures) of the previous version of this file were resolved on
  2026-07-23; their write-ups live in this file's git history. The CUE-011 server-side
  residue carries forward as chunk `02-cue011-silent-revision-fix`. Section 5 (deferred
  UI features) is feature work, deliberately not in the refactoring queue.
