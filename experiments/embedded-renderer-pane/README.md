# Embedded Renderer Pane

The experiment TL-68 leaves open: can the native visualizer live **inside** a Tauri pane with the
desk's ordinary web UI drawing over it, or does the desk have to keep opening a separate window?

The plan sets the bar explicitly. A native surface merely floating above a WebView is not
acceptable — menus, selection overlays, status, dialogs and other web UI must be able to draw
*above* the rendered image where the pane contract requires it.

## Run

From this directory:

```sh
cargo run
```

Build output goes to the canonical `.artifacts/build/experiments/embedded-renderer-pane`.

## The arrangement

The obvious approach — a webview window with a native surface layered on top — fails the bar by
construction: nothing in the web UI can ever draw over the native surface. So this does it the
other way round.

1. The window is a plain native `tauri::Window` with **no webview of its own**, and `wgpu`
   presents straight to it.
2. A child webview is added **on top** of that window with `Window::add_child` (the `unstable`
   feature), sized to fill it, with a transparent background.
3. The web side owns the layout. A `ResizeObserver` on the pane element reports its rectangle, and
   the renderer scissors itself to exactly that rectangle, clearing everything outside it to
   transparent.

Web UI is therefore above the rendered image *by construction*, and the renderer physically cannot
write a pixel outside the pane — the scissor is the contract, not a convention.

## What the window demonstrates

The chrome is deliberately made of the things that have to be able to draw over a 3D pane:

- a dropdown menu that opens across the pane,
- a modal dialog and its scrim,
- dashed selection marquees positioned over the pane,
- a sidebar and status bar that the renderer must not touch,
- **Narrow the pane**, which changes the CSS grid and so moves and resizes the rendered rectangle
  without either side knowing the other's sizing rules.

The status bar reports the pane rectangle, the forwarded input count, the frame count and the
camera, so the claims are visible in the window rather than asserted here.

## Findings

### Confirmed

- **Overlay and z-order hold.** Web UI drawn in the transparent child webview appears correctly
  over the animating native surface. Confirmed by hand at the running window on macOS
  (Darwin 25.5.0, Apple silicon). This is the finding the open question turned on, and it is the
  answer the plan needed: the embedded arrangement is not ruled out.
- **Clipping is structural.** The renderer is scissored to the reported rectangle. The sidebar,
  bar and status bar are untouched because the renderer cannot reach them, not because it is
  careful.
- **A transparent child webview over a native window works at all**, which the earlier
  `wgpu-moving-lights` experiment did not establish — that one opened a Tauri window with no
  webview, so it proved native presentation and nothing about coexistence.

### The real design consequence: input

A `WKWebView` on top swallows mouse events across the whole window, and CSS `pointer-events: none`
does **not** change AppKit hit-testing — it is a web-layer concept and the native view is still the
hit-test winner. So pane input cannot fall through to the surface underneath.

This experiment does not fight that. The web side captures pane input and forwards it as commands
(`orbit`, `zoom`), and the readout counts what arrived. That is the right direction anyway once the
renderer is a supervised helper process, since the events would have to cross an IPC boundary
regardless — but it is a real consequence to carry into the design rather than a detail:

- every camera drag and pick is an IPC round trip, so the helper protocol needs a cheap, coalescing
  input path, not one message per `pointermove`;
- the desk's existing pane input handling cannot simply be pointed at a native surface; it has to
  be re-expressed as forwarded intent;
- anything that wants native-speed direct manipulation (marquee-picking at frame rate, say) needs
  measuring before it is promised.

### Not yet established

The plan requires the embedded route to pass on **every** supported desktop platform before the web
renderer can be removed. This experiment has only been run on macOS, and only some of the checklist
has been exercised:

| Check | macOS | Windows | Linux |
| --- | --- | --- | --- |
| HTML overlay / z-order | confirmed by hand | not run | not run |
| Pane clipping | confirmed by hand | not run | not run |
| Resize and layout change | implemented, needs a pass | not run | not run |
| DPI, including moving between displays | implemented, needs a pass | not run | not run |
| Fullscreen | not implemented | not run | not run |
| Input routing | forwarded by design; latency unmeasured | not run | not run |
| Focus | not exercised | not run | not run |
| Accessibility | not exercised | not run | not run |
| Crash isolation of a helper process | out of scope here — one process | — | — |

Crash isolation is deliberately not in this experiment. It runs the renderer in-process on a
thread, which the desk must never do; proving the pane arrangement and proving the supervised
helper are separate problems, and this one is about pixels.

## What this does and does not decide

It removes the reason to assume the embedded route is impossible. It does not yet earn the removal
of the web renderer, which the plan gates on the full checklist across macOS, Windows and Linux —
and on the same isolated helper, not a thread.

## Scope

Isolated from the production Cargo workspace and importing no ToskLight application code, like the
other experiments here. It is a technical spike, not proposed code.
