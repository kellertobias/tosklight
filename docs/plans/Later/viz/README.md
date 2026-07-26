# Visualizer Planning Documents

This folder is the planning home for the ToskLight rig-planning and high-quality visualization application.

## Documents

- [`rig-planner-visualizer-application-plan.md`](./rig-planner-visualizer-application-plan.md) is the canonical product, architecture, data-model, delivery and acceptance plan.
- [`architecture-decisions.md`](./architecture-decisions.md) records the important technical decisions and the reasoning behind them.

The earlier [`../Next/60-dedicated-renderer-and-paperwork-app.md`](../Next/60-dedicated-renderer-and-paperwork-app.md) is the short seed plan from which this specification grew. Where the two differ, the plan in this folder is authoritative.

## Status

These documents are specifications only. They do not prove that a feature is implemented or tested.

The current delivery priorities are:

1. Shared fixture browser, unpatched placement and patching.
2. Synchronized top, side, front/back and isometric planning views.
3. The separate Rust high-quality renderer.
4. Complete spatial rig modeling.
5. Optional connectivity, cabling, inventory, load calculations and paperwork.

## Update rules

- Keep detailed requirements and phase gates in the canonical application plan.
- Record durable technology or ownership decisions in the architecture-decision document.
- Reuse the existing ToskLight fixture model, Rust crates and `@tosklight/ui` from `apps/ui-library`; do not describe parallel replacements.
- Preserve the distinction between a decision, an open question and a deferred feature.
- When implementation begins, add evidence and links rather than changing planned work to “done” without verification.
