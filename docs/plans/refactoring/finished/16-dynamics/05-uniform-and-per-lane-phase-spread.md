# Uniform and Per-lane Phase Spread

## Goal

Allow an operator to choose whether one Phase Spread applies uniformly to every Dynamic lane or
whether each lane has its own Phase Spread, without changing target membership, losing settings,
or breaking existing shows and runtime snapshots.

## Acceptance

- The production Phase Spread view uses desk-native controls to select **Uniform** or **Per lane**.
- Per-lane mode provides a lane selector and routes the Phase Spread form plus the shared
  software/hardware encoder surface to the selected lane.
- Switching from Uniform to Per lane seeds every lane without an override from the current shared
  Phase Spread, preventing an output jump.
- Switching back to Uniform retains every lane override for a later return to Per lane.
- New and replacement lanes receive the shared Phase Spread while Per-lane mode is active.
- The runtime captures and samples phase maps per lane, including spatial ordering and each lane's
  effective Random-each-loop boundary.
- Uniform Random-each-loop remains one shared permutation even when lane speeds differ.
- Stored definitions without a phase mode or lane phase load as Uniform.
- Legacy runtime snapshots with one phase map restore that map across every lane.
- HTTP object intents, generated TypeScript, and generated JSON schemas represent the mode and
  optional lane phase without moving server-owned seeding or evaluation into the browser.

## Result

Implemented the two Phase Spread modes across the domain model, runtime snapshots and sampling,
show persistence, typed update intents, generated wire contracts, optimistic client projection,
the production Phase Spread workspace, and the reusable software/hardware encoder surface.

Compatibility tests load a physically legacy persisted Dynamic through `StartupState::load` and
restore the old shared runtime phase-map shape. Focused verification passed for all 32 Dynamics
domain tests, the headless mode-transition route, the legacy startup path, generated contracts,
21 desktop Dynamics tests, desktop typechecking, Rust formatting, and diff whitespace.
