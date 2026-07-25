# Typed Control-surface Interaction Routing

## Goal

Replace DOM queries, synthetic clicks, CSS-class ownership, and global window events used for SET,
Store, Update, Preload, Clear, Undo, keypad, and context-menu routing with explicit typed intents.

Estimated effort: 0.4–0.7 Codex day.

## Queue dependency

Pending, blocked until plan 02 stabilizes reusable callback contracts, application adapters,
modal ownership, and the hardware/software component split. This plan changes the same desktop
controls, modal paths, interaction stories, and focused UI verification.

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
