# Visualizer Planning Documents

This folder is the planning home for the ToskLight rig-planning and high-quality visualization application.

This product is separate from the efficient Stage visualizer embedded in
ToskLight. The embedded surface is specified by
[`../../refactoring/doing/21-efficient-built-in-stage-visualizer.md`](../../refactoring/doing/21-efficient-built-in-stage-visualizer.md)
and remains available without the Viz editor or renderer. Viz owns the
high-quality rendering ceiling; the built-in Stage owns low-latency Live and
Preload feedback inside the desk.

The preferred native-rendering direction to test is a Rust `wgpu` render core.
That core may serve the dedicated Viz renderer and, if the cross-platform spike
proves it practical, replace the pixels inside the built-in Stage viewport.
This does not replace the React application shell: React continues to own the
window, pane, toolbar, docking and layout, while a native GPU child surface
tracks the Stage viewport rectangle. It requires no frameset, nested WebView or
rendered-video stream.

## Documents

- [`rig-planner-visualizer-application-plan.md`](./rig-planner-visualizer-application-plan.md) is the canonical product, architecture, data-model, delivery and acceptance plan.
- [`architecture-decisions.md`](./architecture-decisions.md) records the important technical decisions and the reasoning behind them.
- [`network-connected-renderer-mvp.md`](./network-connected-renderer-mvp.md) defines the focused first standalone renderer slice, its lighting-desk API plus Art-Net/sACN provider, the future local planning-software provider boundary, and renderer Quick Settings.

The earlier [`60-dedicated-renderer-and-paperwork-app.md`](./60-dedicated-renderer-and-paperwork-app.md) is the short seed plan from which this specification grew. Where the two differ, the canonical application plan in this folder is authoritative.

Queued follow-up work lives with the other queued plans:
[`../../Next/08-viz-one-application-and-editor-demo-show.md`](../../Next/08-viz-one-application-and-editor-demo-show.md)
covers one application identity for the editor and the visualizer, the editor window that opens
white, and a demo show the editor can open and drive without DMX.

## Status

These documents are specifications only. They do not prove that a feature is implemented or tested.

The current delivery priorities are:

1. Shared fixture browser, unpatched placement and patching.
2. Synchronized top, side, front/back and isometric planning views.
3. The separate Rust high-quality renderer, with `wgpu` as the preferred
   candidate to prove.
4. Complete spatial rig modeling.
5. Optional connectivity, cabling, inventory, load calculations and paperwork.

## Update rules

- Keep detailed requirements and phase gates in the canonical application plan.
- Record durable technology or ownership decisions in the architecture-decision document.
- Reuse the existing ToskLight fixture model, Rust crates and `@tosklight/ui` from `apps/ui-library`; do not describe parallel replacements.
- Do not turn the built-in Stage into a client of the Viz renderer application
  or make either application a runtime dependency of the other. Sharing a
  renderer-core crate is allowed when both applications retain independent
  lifecycle, quality, failure and performance budgets.
- Preserve the distinction between a decision, an open question and a deferred feature.
- When implementation begins, add evidence and links rather than changing planned work to “done” without verification.
