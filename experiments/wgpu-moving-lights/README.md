# wgpu Moving Lights POC

A deliberately small, native Tauri + Rust `wgpu` experiment. It renders:

- two visible moving-head fixtures, left and right;
- continuously animated pan, tilt, and color;
- volumetric-looking beams in party-style haze;
- a central cube lit by both fixtures;
- moving soft shadows on the floor and beam occlusion through the haze.

The whole scene is a fullscreen WGSL raymarcher. That is intentionally a proof-of-concept shortcut: it demonstrates the look and native rendering path without introducing a mesh scene graph, asset pipeline, or production shadow-map system.

## Run

From this directory:

```sh
cargo run
```

The experiment opens a plain Tauri window (no webview) and presents directly to it with `wgpu`. Resize the window freely; the lights animate automatically.

Build output is redirected to the repository's canonical `.artifacts/build/experiments/wgpu-moving-lights` directory.

## Scope

This folder is isolated from the production Cargo workspace and imports no ToskLight application code. It is a visual/technical spike, not a proposed replacement for the current Stage renderer.
