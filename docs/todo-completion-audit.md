# TODO completion audit

This audit maps every `TODO.md` area to authoritative implementation and executable evidence. It is
updated together with the final production gates; prose alone is not considered completion proof.

| Area | Implemented evidence | Automated evidence |
| --- | --- | --- |
| Group masters | `GroupDefinition` fader relationship; normalized, per-logical-head scaling before encoding; HTP maximum across assigned masters; grand master/blackout follow | Engine tests for 50/75% arbitration, unassigned groups, sibling-head isolation and final masters |
| CITP media servers | Fixture-profile capability, parent-owned patch endpoint, inherited heads, MSEX 1.0–1.2 negotiation, GETh/EThn, RqSt/StFr, fragment validation, bounded LRU caches, REST status/media, Setup preview/offline UI | Media protocol fixtures for modern/legacy negotiation, thumbnails, preview, fragments, malformed data and eviction; server endpoint/cache test |
| Empty/template groups | Empty `GroupDefinition`, portable `programming`, session `group_values`, preset `group_values`, cue `group_changes`, dynamic `group_ids` | Engine tests prove stored and session group intent becomes active after later membership |
| Membership changes | Live resolution for group programmer/preload/presets/cues/dynamics, logical-head expansion, revision history/undo, explicit detach and replacement confirmation | Active cue survives recompilation and gains members; show undo test; selection refresh tests |
| Selection macros | Ordered vectors, odd/even/every-Nth with explicit zero-based offset, live derived expressions, recursive cycle rejection | Programmer deterministic-order, derivation-update and cycle tests; invalid-rule snapshot validation |
| Frozen groups | Static expression, repeated `GROUP id GROUP id` grammar, long-press menu, persisted frozen metadata and explicit refresh | Server grammar test proves frozen membership remains fixed and live membership refreshes |
| Alignment | Left/right/center/out over ordered normalized values, discrete rejection, unsupported list, physical metadata, shortest wrap, inversion and curves at encoding | Server wrap/order tests and fixture encoder inversion/curve tests |
| Preload storage | Separate pending/active fixture and group values, arbitrary revisioned preset/cue targets, active-preload fallback when Store is pressed with an empty pending programmer, explicit store modes, idempotent pending clear | Programmer isolation test, server preset revision tests, exact 4-dimmer/6-profile/16-RGBW template-show scenario, and Playwright active-scene/store/clear-to-DMX path |
| UI follow-up | Contextual Special Dialog, full-height 20-key numpad, single dynamic mode icon, unified show control/clock, sub-500 ms dock animation, one visible pane title, responsive pools, adjacent Preload/Release | React tests plus Playwright built-in/dialog, mode-frame, Stage gestures, desktop/tablet fit and preload paths |

## Release gates

The completion gate is all of the following from a clean invocation:

```sh
cargo fmt --all --check
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p light-control-ui
cd apps/control-ui
npm run typecheck
npm test -- --run
npm run build
npm run test:e2e
```

The 64-universe desktop benchmark must also report `PASS`. Raspberry Pi capacity remains a
hardware-specific measured profile, not a software TODO that can be truthfully certified on macOS.

Last full audit: 2026-07-12. All listed gates passed: 65 Rust tests, 14 frontend tests, six
Playwright end-to-end paths, strict Clippy, production web build, release server build, and release
Tauri application bundle. The end-to-end programmer path registers the audit receiver, verifies the
typed command event, and confirms the resulting DMX channel value.

The template-show scenario patches four named front dimmers, six 16-bit pan/tilt RGB profile movers,
and sixteen RGBW pars without native dimmers. It verifies empty group-relative preset/cue intent,
assigned group masters, virtual-dimmer white output, blind preload isolation, immediate Preload GO,
active-preload cue storage, cue recall while another preload is pending, SQLite backup/reopen, and
automatic cue expansion after two profiles are patched and added to the live group.
