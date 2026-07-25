# Complete Shared Control-surface Contracts

## Goal

Extend `packages/light-controls` beyond the numeric keypad so desktop, Hardware Controls, tests,
and future applications share stable operator intent and physical-layout contracts without
importing another application's internals.

Estimated effort: 0.3–0.6 Codex day.

## Queue dependency

Pending, blocked until plans 07 and 08 establish the final typed command, event, Highlight, and
interaction names that this cross-application package must expose. Starting earlier would create a
second temporary compatibility contract.

## Required work

1. Inventory stable shared keypad IDs, OSC action/path mappings, Highlight actions, playback
   addressing, encoder actions, and attached physical-layout metadata.
2. Move only proven cross-application contracts into public package exports.
3. Keep transport connections, runtime state, application controllers, and rendering in their
   owning applications.
4. Replace desktop tests that deep-import Hardware Controls internals.
5. Add architecture rules preventing application-to-application source imports.

## Acceptance and verification

- Both applications and root tests consume package exports.
- Current software/hardware action names, layout order, page semantics, and OSC vocabulary remain
  exact.
- Package unit/type tests, both application builds/tests, architecture checks, and hardware
  interaction acceptance pass.
