# Typed Commands, Events, and Highlight Service

## Goal

Remove the remaining generic WebSocket command/facade event compatibility path and converge
Highlight HTTP, OSC, WebSocket compatibility, persistence, output, and feedback behind one bounded
application service.

Estimated effort: 1.3–2 Codex days.

## Queue dependency

Pending, blocked until plan 02 stabilizes desktop provider/store consumers, generated-client
contracts, architecture checks, and frontend verification. Required work 4–5 edits those exact
surfaces, so this plan must not run concurrently with the active Storybook lane.

## Required work

1. Inventory every production `WsCommand { command: String, payload: Value }` caller/handler and
   every `FacadeNotification { kind, payload }` publisher/consumer.
2. Add generated typed command DTOs, runtime decoders, and capability event variants.
3. Migrate live-control actions to the typed established WebSocket plus their required HTTP action
   forms under `docs/engineering/api-rules.md`.
4. Migrate facade events capability by capability; translate typed events to compatibility output
   only while a deliberate external consumer remains.
5. Replace broad frontend bootstrap/follow-up reads with typed store updates and scoped repair.
6. Create one Highlight application service and use it from HTTP, OSC, any temporary compatibility
   adapter, persistence/output synchronization, and feedback.
7. Remove the second facade event bus, generic command dispatch, obsolete client methods, and
   compatibility tests after callers migrate.

## Acceptance and verification

- Malformed frames fail boundary validation without state mutation.
- Each action produces one semantic outcome/event across HTTP, WebSocket, and OSC.
- Highlight behavior and feedback are identical across surfaces.
- Ordinary events do not trigger unrelated bootstrap or catalog reads.
- Generated-contract, architecture, event gap/replay, API, OSC, UI, and desktop tests pass.
