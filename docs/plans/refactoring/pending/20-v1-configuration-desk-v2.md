# 20 — Configuration, desk-lock, speed groups, desk metadata onto v2; retire their v1 routes

## Context (verified 2026-07-23)

Remaining v1 client modules:

- `api/client/configuration.ts` (via `features/deskConnection`, setup sections,
  DeskLock/Configuration action providers): `GET/PUT /api/v1/configuration`
  (`http_router.rs:103`), `GET /api/v1/matter/status` (`:106`),
  `GET/PUT /api/v1/speed-groups/{group}` (`:108`) + observation (`:112`) + action (`:116`),
  `POST /api/v1/shutdown` (`:101`, also bench + desktop-smoke), desk-lock GET/PUT +
  lock/unlock (`:126-128`).
- `api/client/desk.ts` (via `features/commandHistory`, `features/showLifecycle` users):
  `GET /api/v1/command-history` (`:250`), `POST /api/v1/users` (`:130`),
  `GET /api/v1/audit` (`:251`, also bench).
- `api/client/programming.ts` v1 remnants: `GET /api/v1/programmers` (`:230`, also bench),
  `POST /api/v1/programmers/{id}/clear` (`:231`).
- Update routes `GET/PUT /api/v1/update/settings`, `POST /update/preview|apply`
  (`:235-239`) — **tests only** (`tests/support/updateHighlight`); the UI already uses the
  v2 programming-update surface (`programming_update_http.rs`). Migrate the test helpers
  to v2 and delete.
- `PUT /api/v1/master` (`:247`) — tests only (09-cue-go-to-load, 09-desk-lock).

Classification: configuration/desk-lock/speed-group settings + users are object-intent
edits (request identity); speed-group action/observation and grand master are live control
(v2 speed-group surface exists: `runtime/speed_group_v2.rs` — extend); shutdown is an
operational action (plain POST, fine).

Caution: desk-lock is a security-ish surface with hardware parity (lock from software +
OSC hardware) — validate both paths; desk-level data stays separate from portable shows.

## Work

1. v2 intent routes for configuration, desk-lock, users; extend speed-group v2 for the
   observation/action calls still on v1; v2 snapshot for command-history + audit; move
   shutdown under v2.
2. Migrate the client modules' consumers (setup-window controller sections, DeskSettings,
   CommandLineHistoryPanel, switchUser flow), bench helpers, desktop-smoke (shutdown), and
   the updateHighlight/master test helpers.
3. Delete each v1 route with a caller grep.

## Definition of done

- configuration.ts, desk.ts, and the v1 parts of programming.ts are deleted; all listed v1
  routes gone; desk lock (software + OSC), speed groups, user switching, command history,
  audit, shutdown all verified working.

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e -- tests/09-desk-lock*.spec.ts
npm run test:e2e            # full suite gate
npm run test:desktop-smoke  # shutdown path
```

## Decisions

None.
