# 10c2f — Move in Black semantic migration

## Outcome

Migrate the ordinary `MIB-001` UI workflow.

## Scope

- `MIB-001` in `tests/07-move-in-black.spec.ts`

## Done gate

- A dark fixture prepositions for its next lit Cue without leaking visible
  intensity or disturbing Cue timing and ownership.
- Existing API and supplemental MIB boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.

## Result

- Added a dedicated Move in Black bench area with typed setup, visible
  SET-gated fixture configuration, persisted configuration assertions, and
  deterministic runtime-state and safety-delay oracles.
- Migrated the ordinary UI workflow to a semantic scenario that proves the
  enabled fixture stays dark while prepositioning, the disabled comparison
  fixture waits for normal Cue timing, and both retain the expected Cuelist
  ownership.
- Kept the paired API case and both wire-level blocking, restart, retarget, and
  cancellation boundaries in the original focused suite.
- Verified the semantic UI case, the three retained API/wire cases, a 20-run
  parallel stress pass, the control UI build, semantic documentation and
  compiler checks, architecture, source-size, inventory, formatting, and diff
  hygiene.
