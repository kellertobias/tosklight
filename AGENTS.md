# ToskLight agent guidance

## Scope and intent

Treat this repository as a professional show-lighting control desk, not a generic dashboard. Preserve the operator model, exact control-surface behavior, persisted show compatibility, and parity between software, keyboard, OSC, and attached hardware paths.

Honor the narrowest requested scope. If the request says to edit planning or testing Markdown only, do not implement the feature or executable test until the user expands scope.

## Sources of truth

- Read the relevant numbered files under `docs/help/` for operator-facing behavior.
- Treat `docs/help/30-Programmer/01-command-line.md` and the current command-line help file at that location as the command and keypad contract.
- Treat `docs/testing/` scenario Markdown as the acceptance contract. When implementing one named scenario, keep executable Playwright coverage under root `tests/` and do not silently expand to every scenario.
- Read `docs/acceptance-criteria.md` before changing persisted show or desk data.
- Keep the first H1 in help Markdown suitable as the visible title.
- Update mirrored or generated documentation only through the repository's documented workflow.

## Operator semantics

- An unpatched fixture remains part of the show. It can be selected, programmed, stored in groups and cues, and displayed in fixture/stage views; only DMX output is suppressed until it is patched again.
- Distinguish an intentionally stored empty group from an absent or deleted group.
- Skip missing group IDs in a range instead of treating them as stored empty groups.
- Preserve ordered group membership where order affects value spreading or operator intent.
- Programmer values use LTP semantics unless a documented control path specifies otherwise. Playback/cue HTP behavior must not be generalized to the programmer.
- One Tauri application and its attached OSC hardware form one desk with one shared command line and authoritative desk state. A different desk alias remains isolated.
- Distinguish current-page playback addressing from explicit-page playback addressing and test page changes.

## UI and control surfaces

- Implement literal acceptance criteria for wording, geometry, placement, sizing, alignment, visibility, and behavior.
- Preserve parity across every explicitly named mode, including hardware-connected and software-only layouts.
- Validate the exact physical interaction path the user describes; adjacent click handlers or keyboard-only behavior are not proof.
- A dedicated Tauri surface requested as a separate app remains a sibling desktop app launched from ToskLight, not an embedded pane.
- Avoid silent actions. Long-running setup, download, import, or processing actions need visible progress and actionable error state.
- Keep touch targets appropriate for the desk surface and avoid desktop-only hover assumptions for required actions.

## Persistence and compatibility

- Do not call persisted-file or schema work complete until old show behavior is migrated and tested, or support for old files is explicitly rejected.
- Preserve seeded/default data migrations where existing installations depend on them.
- Keep portable show files and desk-level data separated according to the current architecture.
- Test recovery behavior for malformed or legacy active shows when touching startup/load paths.

## Repository map

- `crates/engine`: resolved values, render state, output and transition behavior
- `crates/programmer`: programmer state and merge semantics
- `crates/playback`: cue/playback behavior
- `crates/server`: REST, WebSocket, OSC, sessions, persistence and server orchestration
- `apps/control-ui`: main Tauri/web operator interface
- `apps/hardware-controls`: sibling hardware-control application
- `tests`: root Playwright acceptance coverage
- `apps/control-ui/e2e/bench`: shared E2E bench helpers
- `docs/help`: operator help and manual source
- `docs/testing`: human-readable acceptance scenarios
- `light-data`: local development data and current server log

## Verification

Start with the smallest relevant checks, then widen according to risk:

```sh
./test unit
./test e2e-api
./test e2e-ui
./test e2e tests/<focused-spec>.spec.ts
./test desktop-smoke
./build open
```

For direct package checks, use the current repository scripts and manifests. Prefer `cargo fmt` for Rust formatting; do not run standalone `rustfmt` against workspace files.

When real operator behavior changed, `./build open` is the authoritative desktop path. After launch:

```sh
curl -fsS http://127.0.0.1:5000/api/v1/readiness
```

Inspect `light-data/light-server.log` first for app-owned server startup/runtime problems. If readiness is healthy but the app appears stuck, time `/api/v1/readiness`, `/api/v1/health`, and `/api/v1/bootstrap` separately.

If the app looks stale, verify the bundle opened by the current `build` script before reworking UI code.

## Documentation and screenshots

- The Markdown files under `docs/help` remain the source of truth for both in-app help and the PDF manual.
- Use `./build manual` to generate and verify the manual.
- Use `./test help-screenshots` only when intentionally refreshing help screenshots.
- Check screenshot diffs visually and keep them tied to stable, representative operator states.

## Working tree and commits

- Preserve unrelated user changes in a dirty worktree.
- Keep implementation, generated documentation/screenshots, and unrelated cleanup in sensible topic commits when the user requests commits.
- Do not rewrite or discard existing work without explicit authorization.
- Before handoff, compare the result against every literal acceptance criterion in the request.

## Delegation

For a large task, use available subagents for independent bounded work such as codebase discovery, test-contract review, compatibility audit, or visual regression review. In Codex, request general-purpose subagents. In Claude Code, use the Agent/subagent mechanism. Do not hardcode a provider-specific model name. Give each worker the applicable raw files and a concrete question; keep integration, mutations to shared live state, and final acceptance review with the primary agent.

