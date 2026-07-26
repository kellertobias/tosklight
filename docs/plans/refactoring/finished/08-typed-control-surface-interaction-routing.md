# Typed Control-surface Interaction Routing

## Goal

Replace DOM queries, synthetic clicks, CSS-class ownership, and global window events used for SET,
Store, Update, Preload, Clear, Undo, keypad, and context-menu routing with explicit typed intents.

Estimated effort: 0.4–0.7 Codex day.

## Queue dependency

Doing. Plan 02 has stabilized reusable callback contracts, application adapters, modal ownership,
and the hardware/software component split, and plan 07b has stabilized typed command and event
names.

## Required work

1. Characterize active-surface precedence and every software, keyboard, context-click, OSC, and
   attached-hardware entry path.
2. Introduce one application-owned interaction registry/owner with typed target capabilities and
   deterministic focus/activation rules.
3. Make buttons and shortcuts call intents directly; reusable UI components emit callbacks only.
4. Replace `document.querySelector`, delayed DOM clicks, and global Update events.
5. Preserve mutation-only undo, modal precedence, hardware/software layout differences, and exact
   operator labels.

## Acceptance and verification

- No action depends on whether a CSS class or button happens to be mounted.
- The same active target is chosen across touch, mouse, keyboard, OSC, and hardware.
- Missing or ambiguous targets produce visible safe feedback, never a silent action.
- Component, interaction-owner, modal, focused Playwright, OSC/hardware, and desktop checks pass.

## Result

### Changes

- Added one application-owned, typed control-surface interaction registry for SET, Update, file
  operations, desk shortcuts, and playback configuration. Owners have explicit priorities;
  equal-priority ambiguity and missing ownership reject the action and emit visible
  `light:command-error` feedback.
- Registered active preset, cue, playback, patch, cuelist, file-manager, and Update owners from
  application state. Touch, mouse, keyboard, context-menu, OSC, attached-control, and server
  inputs now enter the same typed routing boundary while preserving same-desk isolation.
- Removed CSS ownership markers, DOM target queries, delayed/synthetic clicks, the global SET and
  Update action events, the global desk-action bridge, and the document-wide context-menu
  interception. Reusable inputs now request keyboards, number editors, and pickers through typed
  callbacks.
- Made modal capture authoritative in `ModalStack`, retained mutation-only Undo behavior, and kept
  software-only and attached-control layouts on their existing component boundaries.
- Routed playback and Speed Group context actions from the exact card/control that received the
  pointer event. File-operation arming remains available when the File Manager opens after CPY,
  MOV, DEL, or SET, with one claimed manager owning ENTER and ESC.

### Tests

- `npm run typecheck --workspace @tosklight/light-desktop` and
  `npm run typecheck --workspace @tosklight/ui` — passed.
- Ten focused desktop Vitest files covering the registry, Numeric Pad, Update workflow,
  Playback Fader Bank, Playback Tools, Cuelists, command-line shortcuts, and File Manager — 174
  tests passed after formatting.
- Focused shared-UI tests for Programmer Keypad, Modal Stack, common controls, and Playback Cards
  — 45 passed; focused hardware-control routing tests — 3 passed.
- `node --test tools/test-command-boundaries.test.mjs` — 12 passed.
- Focused Rust tests for OSC keys, typed UI/OSC desk isolation, command backspace, programmer
  mutation checkpoints, and authoritative changed/no-op projections — passed.
- `tests/04-osc-api-and-cross-surface.spec.ts`, `tests/21-completion-coverage.spec.ts`, and
  `tests/33-record-and-update-menu-colors.spec.ts --workers=1` — 6 passed, covering API, OSC,
  Highlight, encoder display, Update, and workflow colors.
- `npm run open` — built and bundled both macOS applications and launched ToskLight. The packaged
  server returned HTTP 200 from readiness in 73 ms and bootstrap in 4 ms. On the real desktop
  surface, SET visibly armed the exact preset targets and exited through the selected target
  without a synthetic click.

### Limitations

- The documented `npm run test:desktop-smoke` command is absent from the current root scripts; the
  authoritative `npm run open` build, launch, live endpoint, log, and visible interaction path was
  used instead.
- The repository-wide desktop Vitest run currently has two unrelated concurrent failures:
  Virtual Playbacks expects the former unavailable-card label, and Groups expects the former
  built-in marker. Plan 08 focused tests pass.
- `npm run test:unit` and `npm run test:architecture` stop in the concurrent semantic-catalog
  preflight. The direct architecture checker reports only the already tracked concurrent CSS,
  Fixture Placement modal, bench-inventory, and semantic-documentation findings; it reports no
  Plan 08 boundary violation.
- Biome still reports pre-existing accessibility and exhaustive-dependency findings in the
  concurrently changing shared choice/text-input components. Type checks and focused behavior
  tests pass, and this plan does not claim those broader cleanup items.

### Commit

- `feat(desktop): route control-surface interactions explicitly`
