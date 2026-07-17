# Consolidated Development Artifacts

Consolidate repository-local build products, caches, generated documents, test evidence, scratch files, and development runtime data beneath one ignored `.artifacts/` directory. A normal checkout should show source, configuration, documentation, and the repository entry-point scripts at its root, with `.artifacts/` as the only root-level home for files created by local development.

This is a development-environment change, not a change to the operator-facing show-lighting product. It must not change installed application data locations, portable show contents, release archive layouts, or runtime behavior.

## Canonical layout

Use purpose-based subdirectories so that individual classes of output can be inspected, archived, or cleaned independently:

```text
.artifacts/
  build/
    cargo/
    frontend/
  cache/
    pnpm-store/
  generated/
    manual/
      html/
      pdf/
  release/
  runtime/
    light-data/
  test/
    coverage/
    playwright-report/
    results/
    visual-inspection/
  tmp/
```

The mapping replaces the current repository-root `target/`, `.pnpm-store/`, `output/`, `artifacts/`, `light-data/`, `test-results/`, `playwright-report/`, `coverage/`, and `tmp/` locations. Equivalent generated directories owned by individual workspace packages should use the appropriate shared subtree where their tools allow an output path to be configured reliably.

`crates/output/` is Rust source code and is explicitly outside this feature. Installed application data, user-selected show locations, system temporary directories, and external tool caches outside the checkout are also outside this consolidation.

Package installation trees such as `node_modules/` may remain beside their package manifests when Node resolution requires them. The package-manager content-addressed store and downloadable cache belong under `.artifacts/cache/`; this feature does not require changing package managers solely to eliminate a tool-required `node_modules/` directory.

## One path contract

Define the layout once in a repository-owned helper or shared configuration rather than repeating path strings across shell, TypeScript, Rust, Python, and CI files. Repository entry points such as `./build`, `./dev`, and `./test` must initialize the same absolute artifact root before invoking Cargo, npm or pnpm, Tauri, Playwright, and documentation tools.

The implementation must route at least:

- Cargo and Tauri build products through `CARGO_TARGET_DIR` into `.artifacts/build/cargo/`;
- frontend build output into `.artifacts/build/frontend/` where the consuming server and Tauri configuration can read it without copying it back to a root-level directory;
- the pnpm store, when pnpm is used, into `.artifacts/cache/pnpm-store/`;
- generated PDF and HTML manuals into `.artifacts/generated/manual/`;
- distributable application and server archives into `.artifacts/release/`;
- Playwright attachments, traces, screenshots, videos, HTML reports, coverage, and assembled visual-inspection media into `.artifacts/test/`;
- debug desk databases, logs, PIDs, shows, and fixture-library data into `.artifacts/runtime/light-data/`; and
- repository-owned staging and short-lived scratch files into `.artifacts/tmp/`.

Tool configuration, application launch code, process cleanup, log messages, help text, CI upload steps, release publishing, desktop smoke tests, and visual-recording assembly must consume the canonical paths. No consumer should depend on the old root locations or reconstruct the new layout independently.

Existing supported overrides remain supported. In particular, an explicit `LIGHT_DATA_DIR` continues to override the repository development default, and an explicitly supplied tool output directory must either be honored or rejected with a clear error rather than silently ignored. Paths must remain safe when the checkout contains spaces.

## Development runtime versus installed runtime

Only development data associated with this checkout moves to `.artifacts/runtime/light-data/`. Packaged macOS, Windows, and Linux applications must continue to use their established per-user application-data locations. A release archive must not contain the enclosing `.artifacts` hierarchy or accidentally include local databases, logs, test results, caches, or scratch files.

The Tauri development application and a server started by `./dev`, `./build open`, a desktop smoke test, or an E2E fixture must agree on the same runtime directory. Server discovery, bundled-binary copying, process matching, readiness diagnostics, and the reported log path must use the configured Cargo target and runtime roots rather than hard-coded `target/` or `light-data/` paths.

## Migration and compatibility

The first implementation must account for existing untracked root-level directories without deleting developer data.

- Before using the new runtime location, detect an existing root `light-data/`. If the new runtime directory is absent, offer or perform a documented one-time move that preserves every database, show, fixture-library file, log, and recovery file. If both locations contain data, stop and report the conflict; never merge or choose one silently.
- Build products, test results, generated manuals, release archives, caches, and scratch files may be regenerated, but migration tooling must not delete them implicitly. Provide a clearly described cleanup or move path and leave ambiguous files untouched.
- Keep the legacy ignore entries during a transition period so an older tool invocation cannot make generated data appear as source changes. Once all supported entry points have been verified, `.gitignore` can make `/.artifacts/` the primary rule while retaining narrowly justified compatibility ignores.
- Update development documentation and command output that currently directs developers to `target/`, `output/`, `artifacts/`, `test-results/`, or `light-data/`.

Moving runtime data must be an explicit, recoverable operation. The implementation must not treat `.artifacts/` as wholly disposable merely because the directory is ignored: `.artifacts/runtime/` can contain the active development show and desk state.

## Cleanup behavior

Add a repository entry point for selective cleanup. The default cleanup removes reproducible build output, caches, test evidence, generated manuals, release packages, and scratch files, but preserves `.artifacts/runtime/`. Removing development runtime data requires a separate explicit command, a confirmation that names the exact directory, and a warning that local shows and desk state are included.

Cleanup must refuse unresolved, empty, root, home-directory, or checkout-root targets. It should use the shared path contract and must not follow an artifact subdirectory that has been replaced with a symlink outside the checkout unless that external target was explicitly configured and confirmed.

## Acceptance coverage

Implementation is complete when:

- a clean checkout can run unit tests, focused Playwright tests, manual generation, desktop smoke, `./dev`, `./build open`, and release archive generation without recreating any of the superseded root-level generated directories;
- Cargo, both Tauri applications, the standalone server, Playwright, visual recording, manual tooling, and CI all read and write the documented `.artifacts/` subdirectories;
- `./build open` reaches readiness, opens the app built beneath `.artifacts/build/cargo/`, and reports the real log beneath `.artifacts/runtime/light-data/`;
- an existing development `light-data/` is preserved through the one-time migration, while a two-location conflict fails safely with actionable instructions;
- explicit data and build-path overrides still work, including from a checkout path containing spaces;
- release and CI uploads select the intended files from the new locations and contain no local runtime data or enclosing `.artifacts` path components;
- default cleanup preserves runtime data, explicit runtime cleanup is guarded, and safety checks reject broad or unresolved targets; and
- after commands finish, the repository root contains no generated `target/`, `output/`, `artifacts/`, `light-data/`, `test-results/`, `playwright-report/`, `coverage/`, `tmp/`, or `.pnpm-store/` directory.

Verification should start with focused configuration and path-resolution tests, then exercise the repository wrappers and CI-equivalent commands. Finish with desktop smoke and the authoritative `./build open` path, checking both readiness and the actual filesystem locations created by each workflow.
