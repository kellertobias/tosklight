# Execution prompt

Start a refactoring execution session with exactly this `/goal`:

```
/goal In /Users/keller/repos/light (branch `refactoring`), execute the refactoring queue
at docs/plans/refactoring/ per its README.md, one chunk at a time. Claim the
lowest-numbered file in pending/ by moving it to doing/ (max one file there). If the
chunk carries the .ATTENTION.md suffix or an unresolved DECISION NEEDED, STOP and ask
the maintainer. Re-verify the chunk's file:line claims against the code before editing.

Work each chunk with parallel subagents where the work decomposes: fan out independent
discovery/verification/migration units concurrently, keep integration and shared-state
mutations in the primary session. When subagents edit files in parallel, give each an
isolated git worktree based on the CURRENT refactoring branch head — every worktree
agent's first action must be `git rev-parse HEAD` and an explicit
`git reset --hard <the sha you were given>` (worktree agents have mis-based off main
before); spot-check `git worktree list` before consolidating. Consolidate worktree
results into the refactoring branch only after the chunk's verification passes; remove
the worktrees afterwards.

Gate per chunk: the chunk's own verification steps first, then the full
`npm run test:e2e` with no net new regressions against the baseline in README.md (run
and record a fresh baseline first if it is unfilled). cargo fmt; follow AGENTS.md and
docs/engineering/api-rules.md throughout. Then append a "## Result" note (what changed,
suite numbers, surprises, follow-ups filed as new pending/ files), move the chunk to
done/, and commit as a topic commit. Do not push. Done when the claimed chunk is in
done/ with its Result note and the suite is at or above baseline.
```

Notes:

- One chunk per session is the intended cadence; take a second chunk only if the first
  was trivial and the suite is green.
- The parallel/worktree machinery is a means, not a goal — a small chunk executed
  directly in the main session is fine. Use worktrees only when agents would otherwise
  conflict on the same files.
- The bench is flaky (worker crashes, "N did not run"); re-run a suspected failure in
  isolation before treating it as real (known: FIXTURE-002 @restart, TIME-002 @ui,
  GROUP-005 @supplemental).
