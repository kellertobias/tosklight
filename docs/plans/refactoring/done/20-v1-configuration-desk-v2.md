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
3. Correct Speed Group interaction and settings behavior while the speed-group surface is
   migrated:
   - tapping/clicking a Speed Group performs the speed tap action directly;
   - opening the Speed Group settings modal requires Shift-click/Shift-tap or a long
     press/hold on the Speed Group;
   - the settings modal has **Apply** in the title bar beside the close button;
   - the Cancel button is removed;
   - closing an unchanged modal closes immediately;
   - closing a dirty modal opens a confirmation with **Close and discard**, **Close and
     save**, and **Stay**;
   - **Learn** is removed from the settings modal because ordinary tap now performs the
     tap-tempo action;
   - **Half**, **Double**, and **Pause** move into the modal title as immediate actions
     using icons (`÷2`, `×2`, and Pause), and clicking them does not dirty the modal;
   - microphone permission, audio source, and audio input selection move out of the
     Speed Group modal into the Desk Settings **Input** section;
   - input level and selected-band level remain visible in the Speed Group modal;
   - band selection is colocated with the selected-band level display;
   - the modal exposes a Speed Group source selector with exactly **Manual**, **Speed
     Group**, and **Sound to Light**;
   - **Manual** hides the source-specific settings;
   - **Speed Group** shows a selector for a different Speed Group and rejects recursive
     references, including indirect cycles;
   - **Sound to Light** shows the remaining sound-to-light settings and live feedback
     needed to judge whether the settings are useful; and
   - the source selector and sound-to-light settings use authoritative server state and
     v2 feedback, not modal-local derived state.
4. Delete each v1 route with a caller grep.

## Definition of done

- configuration.ts, desk.ts, and the v1 parts of programming.ts are deleted; all listed v1
  routes gone; desk lock (software + OSC), speed groups, user switching, command history,
  audit, shutdown all verified working.
- Speed Group tap, Shift/hold settings entry, modal dirty-close behavior, title-bar
  immediate actions, input-settings ownership, source selection, recursion rejection, and
  sound-to-light feedback are covered on the migrated v2 surface.

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e -- tests/09-desk-lock*.spec.ts
npm run test:e2e -- tests/<speed-group-focused-spec>.spec.ts
npm run test:e2e            # full suite gate
npm run test:desktop-smoke  # shutdown path
```

## Decisions

1. Ordinary Speed Group tap means tap tempo. Settings are reached only by Shift-click,
   Shift-tap, or long press/hold.
2. Desk-level audio input selection belongs in Desk Settings Input, not inside each Speed
   Group settings modal.
3. Speed Group source is one of Manual, another Speed Group, or Sound to Light; recursive
   Speed Group references are invalid.

## Execution

Claimed 2026-07-24. The recorded decisions are resolved; no maintainer decision is open.

## Result

Completed 2026-07-24.

- Added typed, authenticated v2 desk-management routes for configuration, Matter status,
  Speed Group settings/actions/observations, shutdown, global master, desk lock, users,
  command history, audit, and programmer inspection/clear. Intent routes carry request
  identity and duplicate requests cannot execute concurrently.
- Removed the listed v1 routes and migrated production clients, bench helpers, desktop
  smoke, update/highlight helpers, and acceptance callers. A caller grep for the retired
  route families is empty.
- Made Speed Group source selection authoritative and persistent across Manual, another
  Speed Group, and Sound to Light, including legacy migration, directed-chain resolution,
  direct/indirect cycle rejection, and manual takeover from every playback/control
  surface.
- Changed ordinary Speed Group activation to tap tempo; Shift/hold opens settings. The
  modal now owns Apply and immediate half/double/pause title actions, exact dirty-close
  choices, source-specific fields, and live level feedback. Desk-wide microphone and
  input selection now live under Desk Setup → Network & Inputs.
- Verification passed: `cargo fmt --all -- --check`; source-size ratchet; full server
  tests (475 passed, 1 ignored, with the sandboxed CITP socket test passing separately);
  `npm run test:unit` (277 files, 2,008 tests); focused desk-lock/sound-to-light E2E
  (5 passed); repeated playback-state isolation E2E (3 passed); desktop smoke (2 passed);
  and the full E2E gate (286 passed, 10 intentionally skipped).
