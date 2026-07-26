# Shared UI

`@tosklight/ui` is the private repository-owned React component package for
ToskLight operator surfaces. Its production components are extracted from the
Control UI and are rendered directly by both the application adapters and
Storybook; stories must not maintain parallel visual implementations.

The contained Storybook application lives under `storybook/`. Its configuration discovers
low-level stories colocated under this package's `src/` tree and functional stories colocated
under `apps/light-desktop/src/`. Deterministic fixtures and provider harnesses belong under the
contained Storybook directory; production visual components do not.

The package owns reusable rendering, local interaction, operator geometry,
typed view models and callbacks, and package styles. Product state, server and
Tauri access, persistence, feature controllers, and window registration remain
under `apps/light-desktop`.

## Commands

Run these from the repository root:

```sh
npm run test:ui-package   # package typecheck and Vitest suite
npm run storybook         # interactive development server on port 6006
npm run storybook:build   # static output in the shared artifact tree
npm run test:storybook    # static build plus serial real-Chrome story gate
```

The Storybook gate enumerates every tracked story, requires an explicit
documentation-ready wrapper, rejects blank or unstable surfaces, console
errors, REST requests, and WebSockets, and exercises the desktop-grid and modal
stack contracts.

The contained Storybook application is the required review checkpoint for refactoring
plan 02. Full application adoption and documentation-screenshot migration must
not proceed until the package and its stories are accepted.
