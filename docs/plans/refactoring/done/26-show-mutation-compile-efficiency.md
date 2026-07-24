# 26 — Show-mutation compile efficiency: stop paying whole-show costs per edit

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

## Baseline profile

Measured on 2026-07-24 with a release-build temporary harness using 1,200 current-schema
patched fixtures, 100 Cuelists with 20 Cues each, 100 Groups, and 400 Presets (1,800
objects total):

- `prepare_show_candidate`: 47.309 ms mean, 46.339 ms median, 54.730 ms p95;
- the following one-object SQLite commit: 0.182 ms;
- sampling attributed roughly 83% of preparation to compatibility migration staging
  (dominated by the lean-patch sweep) and 18% to final candidate compilation
  (dominated by compiling the same patch a second time).

The baseline confirms that persistence is not the bottleneck and that a normalized
1,200-fixture patch is decoded and compiled twice for an unrelated one-object edit.

## Implementation

- Active-show object and route mutations now use an incremental compiler whenever the live
  snapshot matches the active document revision. The full compiler remains the show-open,
  activation, recovery, and revision-mismatch oracle.
- Compatibility normalization is staged only for objects named by the transaction. Patch and
  profile changes continue through the full migration/compile path, preserving their cross-record
  compatibility rules.
- Runtime snapshot projections use shared immutable vectors. Cue edits rebuild only the coupled
  Cuelist/playback/page projection; preset-only edits rebuild no engine projection.
- Engine validation, profile indexes, Playback runtime, route slices, snap-attribute indexes,
  group-master indexes, and live-selection refresh are reused or skipped when their source
  projection is unchanged.
- The release benchmark now measures incremental candidate compilation, engine preparation, and
  generation installation for paired 120- and 1,200-fixture snapshots. Forgejo runs the gate
  before semantic release.

## Result

Final release measurements on 2026-07-24, using 100 Cuelists with 20 Cues each,
100 Groups, and 400 Presets:

- 120 fixtures: 0.982 ms median, 1.024 ms p95;
- 1,200 fixtures: 0.979 ms median, 1.016 ms p95;
- untouched projections were structurally shared and matched the full compiler;
- the large fixture projection did not increase cue-mutation latency;
- the 5 ms p95 CI ceiling passed.

The complete application and engine suites pass. The server suite passes, with its CITP loopback
case run separately outside the filesystem/network sandbox. The full E2E gate completed with
286 passing and 9 intentionally skipped cases; its one parallel OSC quiet-period timing failure
passed immediately in an isolated rerun.
