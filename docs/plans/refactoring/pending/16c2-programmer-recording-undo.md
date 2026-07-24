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
