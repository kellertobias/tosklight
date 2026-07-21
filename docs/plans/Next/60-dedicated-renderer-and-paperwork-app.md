# Dedicated Renderer and Paperwork App

## Status

**Specification only.** This plan records a future sibling application and shared-rendering workflow. It does not implement rendering, show-file editing, API connections, paperwork generation, or UI changes.

## Goal

Create a dedicated renderer as a separate application that can read ToskLight show files, connect to the ToskLight API for live values, and use a rendering technology suited to richer visualization than the desk's embedded Stage view.

This app should support high-quality rendering features such as fog simulation, particle effects, richer materials, and other visual effects that are not appropriate for the live desk surface.

## Cross-functional workflow

The renderer should also support layout and paperwork workflows:

- read show files and possibly generate or update show files from the patch side;
- provide a 2D view that still uses the 3D fixture and venue models as its source;
- use that 2D projection for lighting plots and paperwork;
- allow show layout and paperwork work in the separate app; and
- expose a compatible paperwork workflow from the desk so the operator can work from either surface.

The separate app may need the shared frontend component, window-system, and layout libraries from the shared-frontend plan.

## Acceptance coverage

1. The renderer can load a show file without requiring the main desk UI to be open.
2. Live values can be read from the ToskLight API when connected, while offline show-file inspection remains possible.
3. 2D paperwork views derive from the same fixture, model, and position data as the 3D view.
4. Desk-side and renderer-app paperwork workflows use compatible data and vocabulary.
5. The richer renderer remains separate from live desk operation so expensive rendering features cannot compromise the control surface.
