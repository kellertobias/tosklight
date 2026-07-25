# 10c — Remaining UI-family migration and cleanup

## Outcome

Migrate every remaining ordinary root UI scenario, retain explicit low-level
boundaries only where the mechanism is the acceptance contract, and remove the
last orphaned compatibility helpers.

## Work

- Process the 10a inventory by helper family until every row is migrated or has
  a reviewed protocol/layout/process justification.
- Preserve API/UI pairing, OSC and wire coverage, screenshot names, demo assets,
  serial constraints, and all operator semantics from parent chunk 10.
- Remove wrappers and support helpers after their final consumer.
- Make `docs/testing/README.md` the concise final author guide.

## Done gate

- No inventory row remains pending.
- All ordinary UI scenario bodies use the semantic world and pass enforcement.
- Every retained low-level boundary is documented and narrow.
- The full browser suite, architecture/unit gates, recording catalog, product
  demo, and parallel isolation stress gate pass.

## Result

Split on 2026-07-25 after the generated 10a inventory reached the cleanup phase.

- The remaining scope is 110 pending UI rows across 26 root files.
- Work is divided into five non-overlapping children: show/foundational,
  Cue/Playback, network/OSC/restart, files/lock/operator shell, and final
  operator controls/orphan cleanup.
- The final child retains the global no-pending-row, full unit/browser,
  recording, demo, author-guide, and orphan-removal gates.
