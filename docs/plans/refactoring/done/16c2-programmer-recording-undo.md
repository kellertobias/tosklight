# 16c2 — Make recording undo programmer-owned

## Context

The remaining public generic object undo route bypasses the desk-scoped programmer model.
Preset and Preload compatibility store routes also remain beside their typed recording services.

## Work

1. Track successful cue, preset, and group recordings as programmer-owned undo entries.
2. Make desk-scoped Programmer Undo roll back the recording through the active-show service.
3. Preserve ordinary programmer value/selection undo and define safe redo behavior.
4. Migrate remaining preset/Preload compatibility callers to typed v2 recording services.
5. Remove public generic object undo and obsolete compatibility store routes.

## Definition of done

- Recording undo is scoped to the originating desk/programmer.
- Generic public object undo is absent.
- Preset and Preload compatibility store routes have no callers and are absent.
- Recording, undo, persistence, and recovery tests are green.

## Verification

```sh
cargo test -p light-programmer -p light-application -p light-server
npm run test:unit
npm run test:e2e-api
npm run test:e2e
```

## Decisions

Inherited from chunk 16 and api-rules section 7. No open decisions.

## Result

- Added desk- and user-scoped show-recording history for cue, preset, group, and Update
  operations, while preserving the existing programmer snapshot ordering.
- Programmer `UND` now reverses each recorded show mutation atomically through the
  active-show service; cue creation undo includes its new cue list and playback/page
  topology, and show undo deliberately clears unsafe programmer redo state.
- Migrated remaining Preset, Preload, Highlight, and Update callers to typed v2 recording
  services or Programmer `UND`, then removed the public generic object undo and obsolete
  compatibility store routes.
- `cargo test -p light-programmer -p light-application -p light-server` passed (92
  programmer, 398 application, 463 server tests; one server test remains intentionally
  ignored), `npm run test:e2e-api` passed (86 passed, 1 skipped), and the full
  `npm run test:e2e` passed (285 passed, 11 skipped). Architecture and source-size
  ratchets passed. The unit aggregate exposed one unrelated cross-test selection-action
  flake, which passed immediately in isolation.
