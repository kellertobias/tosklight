# 25 — Show-mutation compile efficiency: stop paying whole-show costs per edit

## Context (maintainer directive, 2026-07-23)

Every show-object mutation currently runs `prepare_show_candidate`: a compatibility-
migration sweep over **every** object plus a **full engine-snapshot compile of the whole
show** before commit (`crates/application/src/show_compiler/prepare.rs`,
`compile_show_candidate`). On large shows this CPU cost dominates the mutation path by
orders of magnitude (the disk write is one WAL append — see the resolved
`done/02b-c-deferred-show-commits.RESOLVED.md`).

The maintainer's bar: show operation must use as little CPU as possible. Expected load
includes cue-object changes multiple times per second and, once the Dynamics effect
engine lands, self-referencing effects running pan/tilt/color across ~1,200 fixtures —
per-mutation whole-show compiles do not survive that world.

## Work

1. Profile the mutation path on a large generated show (~1,200 fixtures, realistic
   cue/preset/group counts): measure migration sweep vs candidate compile vs commit.
2. Make migration staging incremental: after the initial post-open pass, a mutation
   should only re-check objects it touched (the show is already normalized on open;
   track normalization per object revision instead of sweeping).
3. Make candidate compilation incremental where the dependency graph allows: a cue_list
   body edit must not recompile patch/geometry/groups; recompile only the affected
   projections, or introduce a dirty-subgraph compile.
4. Re-verify the chunk 02 invariants (revision/event ordering, migration write-back
   publication) — incremental compilation must be observably identical.
5. Add a benchmark gate (extend `light-benchmark`) pinning per-mutation latency on the
   large-show fixture so regressions surface in CI.

## Definition of done

Per-mutation cost on the large-show benchmark is bounded by the touched subgraph, not
show size; all existing suites green; benchmark gate in place.

## Verification

```sh
cargo test -p light-application -p light-engine -p light-server
npm run test:e2e   # full suite gate
# plus the new large-show benchmark run
```

## Decisions

None open. Sequence: near the end of the migration (after 16/22 reshape the mutation
paths), together with or after 24 (UI snappiness).
