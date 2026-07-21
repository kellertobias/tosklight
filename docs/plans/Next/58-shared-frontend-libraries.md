# Shared Frontend Libraries

## Status

**Specification only.** This plan records a future frontend architecture cleanup. It does not implement packages, move components, change build tooling, or alter runtime behavior.

## Goal

Extract reusable frontend foundations from the application into shared libraries so the desk UI, sibling applications, and future tools can build from the same component and layout system.

The target structure is:

- one reusable component and window-system library for forms, dialogs, modals, panes, windows, common controls, and operator-facing UI primitives;
- one reusable application-layout library for the rough desk/app shell, window placement, pane layout, navigation structure, and responsive hardware-connected versus software-only layout rules; and
- the actual control UI application, which should contain product behavior, data binding, command handling, and desk-specific workflows rather than generic UI infrastructure.

## Boundaries

Layout-related and visual-system decisions belong in the new shared libraries. Special-case application code should remain only where it is behavior-specific, domain-specific, or tied to ToskLight runtime state.

The cleanup must not create a generic dashboard style. The extracted libraries still serve the professional lighting-desk operator model and must preserve existing touch, keyboard, hardware-connected, and desktop interaction constraints.

## Acceptance coverage

1. Shared forms, dialogs, window chrome, pane containers, and common controls have one authoritative implementation.
2. Application-level shell and layout primitives are reusable without importing ToskLight behavior.
3. ToskLight-specific behavior remains in the app layer and does not leak into generic layout components.
4. Existing UI surfaces retain their geometry, labels, mode boundaries, and hardware/software parity after extraction.
5. Future sibling apps can reuse the libraries without depending on desk runtime state.
