# 10c2e4 — Virtual Playback Preload semantic migration

## Outcome

Migrate the ordinary `PRELOAD-004` UI case.

## Scope

- `tests/preloadVirtualPlaybackContracts/virtualPlaybackPreloadScenarios.ts`

## Done gate

- Virtual Preload retains blind/timed behavior, feedback, release, and disabled
  domain behavior.
- Existing API, OSC, supplemental, and intentionally deferred boundaries remain
  unchanged.
- Focused API/UI/OSC cases, architecture, inventory, and stress pass.

## Result

- Added a typed Playback oracle for shared activation timestamps.
- Replaced the ordinary paired UI half with semantic-world `PRELOAD-004`
  coverage for virtual-only capture, visible pending feedback, the 2.5-second
  shared transition, physical and Programmer disabled-domain behavior, and
  visible release.
- Retained the API half, exact-timing supplemental, and intentionally skipped
  detailed-feedback UI supplemental unchanged.
- Verified the focused semantic UI case, retained API and supplemental cases,
  20-run parallel UI stress, control-ui build, semantic documentation,
  architecture, source-size, inventory, formatting, and diff checks.
