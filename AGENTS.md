# ToskLight agent guidance

## Scope and intent

Treat this repository as a professional show-lighting control desk, not a generic dashboard. Preserve the operator model, exact control-surface behavior, persisted show compatibility, and parity between software, keyboard, OSC, and attached hardware paths.

Honor the narrowest requested scope. If the request says to edit planning or testing Markdown only, do not implement the feature or executable test until the user expands scope.

## Sources of truth

- For fixture-profile and fixture-package work, use the repository-owned skill at `.agents/skills/build-light-fixtures/SKILL.md` and its referenced contract.
- Read the relevant numbered files under `docs/help/` for operator-facing behavior.
- Treat `docs/help/30-Programmer/01-command-line.md` and the current command-line help file at that location as the command and keypad contract.
- Treat `docs/testing/` scenario Markdown as the acceptance contract. When implementing one named scenario, keep executable Playwright coverage under root `tests/` and do not silently expand to every scenario.
- Read `docs/acceptance-criteria.md` before changing persisted show or desk data.
- Follow `docs/engineering/api-rules.md` for every new or reworked HTTP/WS route and client call site; bring violating routes you touch into compliance in the same chunk.
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

- `crates/light/domain/engine`: resolved values, render state, output and transition behavior
- `crates/light/domain/programmer`: programmer state and merge semantics
- `crates/light/domain/playback`: cue/playback behavior
- `crates/light/adapters/headless`: REST, WebSocket, OSC, sessions, persistence and server orchestration
- `apps/light-desktop`: main Tauri/web operator interface
- `apps/light-hardware-controls`: sibling hardware-control application
- `tests`: root Playwright acceptance coverage
- `tests/bench`: shared E2E bench helpers
- `docs/help`: operator help and manual source
- `docs/testing`: human-readable acceptance scenarios
- `.artifacts/runtime/light-data`: local development data and current server log

## Generated artifacts and temporary work

- Put every repository-owned generated file, build product, test result, report, runtime scratch
  directory, ad-hoc configuration, profiling workspace, and temporary directory under `.artifacts/`
  or one of its canonical subdirectories.
- Use `tools/artifact-paths.sh`, `tools/artifact-paths.mjs`, `tools/artifact_paths.py`, or
  `npm run --silent artifact-path -- <name>` instead of inventing paths. Temporary work belongs
  under the resolved `LIGHT_TMP_DIR`.
- Do not create or direct tools to legacy root locations such as `target/`, `output/`,
  `test-results/`, `playwright-report/`, `coverage/`, `dist/`, `storybook-static/`, or `tmp/`.
- The undotted root `artifacts/` directory must not exist. Use `.artifacts/`; never add
  `artifacts/` back to `.gitignore` or use it as a release, visual, or temporary destination.
- A temporary Playwright configuration must import and extend the repository configuration, or
  explicitly use the canonical Playwright results and report paths. The directory containing a
  temporary config does not determine where Playwright writes its output.
- Preserve explicit caller overrides such as `LIGHT_ARTIFACTS_DIR`, the focused `LIGHT_*_DIR`
  variables, `LIGHT_DATA_DIR`, and `CARGO_TARGET_DIR`; an explicit override is an intentional
  external destination, not a repository default.
- Dependency installations, toolchain caches, tracked generated documentation/assets, and
  CI-service files such as `$GITHUB_OUTPUT` and `$GITHUB_STEP_SUMMARY` are not repository build
  artifacts. Keep their established ownership and workflows.
- The only approved root directories are `.agents`, `.artifacts`, `.cargo`, `.forgejo`, `.git`,
  `.github`, `.show`, `.tour`, `apps`, `assets`, `crates`, `docs`, `experiments`, `node_modules`,
  `tests`, and `tools`. Regular root files are allowed. Use `npm run clean:root` to move any other
  root entry into recoverable storage under `.artifacts/cleanup/repository-root/`.
- Use `npm run clean:artifacts` to remove generated artifact trees while preserving
  `.artifacts/runtime` and root-cleanup recovery.

## Verification

Start with the smallest relevant checks, then widen according to risk:

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e-ui
npm run test:e2e -- tests/<focused-spec>.spec.ts
npm run test:desktop-smoke
npm run open
```

For direct package checks, use the current repository scripts and manifests. Prefer `cargo fmt` for Rust formatting; do not run standalone `rustfmt` against workspace files.

When real operator behavior changed, `npm run open` is the authoritative desktop path. After launch:

```sh
curl -fsS http://127.0.0.1:5000/api/v2/readiness
```

Inspect `.artifacts/runtime/light-data/light-headless.log` first for app-owned server startup/runtime problems. If readiness is healthy but the app appears stuck, time `/api/v2/readiness` and `/api/v2/bootstrap` separately.

If the app looks stale, verify the bundle opened by the current `build` script before reworking UI code.

## Documentation and screenshots

- The Markdown files under `docs/help` remain the source of truth for both in-app help and the PDF manual.
- Use `npm run manual` to generate and verify the manual.
- Use `npm run test:help-screenshots` only when intentionally refreshing help screenshots.
- Check screenshot diffs visually and keep them tied to stable, representative operator states.

## Working tree and commits

- Preserve unrelated user changes in a dirty worktree.
- Keep implementation, generated documentation/screenshots, and unrelated cleanup in sensible topic commits when the user requests commits.
- Do not rewrite or discard existing work without explicit authorization.
- Before handoff, compare the result against every literal acceptance criterion in the request.

## Delegation

For a large task, use available subagents for independent bounded work such as codebase discovery, test-contract review, compatibility audit, or visual regression review. In Codex, request general-purpose subagents. In Claude Code, use the Agent/subagent mechanism. Do not hardcode a provider-specific model name. Give each worker the applicable raw files and a concrete question; keep integration, mutations to shared live state, and final acceptance review with the primary agent.
