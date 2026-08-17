<p align="center">
  <img src="assets/branding/ToskLight%20Control.png" alt="ToskLight Control application icon" width="128" height="128">
</p>

<h1 align="center">ToskLight</h1>

<p align="center">
  A professional show-lighting desk for productions that need real desk concepts without a
  full-size desk budget.
</p>

> [!CAUTION]
> **ToskLight is not yet a release candidate.** Published builds are available for testing, but
> the software and its show-file format may still change between versions.

Industry-standard lighting desks are excellent tools and worth every penny. Smaller productions,
however, should not have to learn throwaway workflows simply because their budget is smaller.
ToskLight bridges that gap: it brings professional concepts such as a command line, Programmer,
tracking, Groups, Presets, Cuelists, Playbacks, and portable show files to macOS, Windows, Linux,
and browser-connected desks.

<p align="center">
  <strong><a href="https://kellertobias.github.io/tosklight/">Operator manual, product demo, and downloads →</a></strong>
</p>

<p align="center">
  <a href="https://kellertobias.github.io/tosklight/">
    <img src="https://kellertobias.github.io/tosklight/screenshots/application-overview.png" alt="ToskLight operator desktop with Fixture Sheet, 3D Stage, Groups, Presets, and Playbacks" width="49%">
  </a>
  <a href="https://kellertobias.github.io/tosklight/">
    <img src="https://kellertobias.github.io/tosklight/screenshots/stage-3d.png" alt="ToskLight 3D Stage visualizer" width="49%">
  </a>
</p>

The screenshots above are generated from the current application stories and published with the
[ToskLight GitHub Pages site](https://kellertobias.github.io/tosklight/). The site also hosts the
[operator manual](https://kellertobias.github.io/tosklight/manual/), the generated product demo,
and current release downloads.

## Product family

| Icon | Application | Role |
| --- | --- | --- |
| <img src="assets/branding/ToskLight%20Control.png" alt="ToskLight Control application icon" width="72" height="72"> | **ToskLight Control** | Lighting-control software and operator interface. |
| <img src="assets/branding/ToskLight%20Architect.png" alt="ToskLight Architect application icon" width="72" height="72"> | **ToskLight Architect** | CAD-based venue and rig planning with live 3D visualization. |
| <img src="assets/branding/ToskLight%20Pixel.png" alt="ToskLight Pixel application icon" width="72" height="72"> | **ToskLight Pixel** | Media server for video, images, text, generated visuals, and effects. |

**ToskLight Desk** is reserved for the future physical hardware desk.

## Features

- Command-line and keypad operation with a live LTP Programmer.
- Ordered fixture Groups, reusable Presets, sparse tracked Cues, Cuelists, Playbacks, and group
  masters.
- Fixture Sheet, Channels and DMX inspection, plus built-in 2D and 3D Stage views.
- Dynamics, fade/follow/timecode playback, ArtTimeCode, OSC, native extensions, and attached OSC hardware.
- Art-Net and sACN output, including explicit routing and health diagnostics.
- GDTF-style fixture packages, multi-head fixtures, multi-patch Stage instances, MVR workflows,
  and CITP/MSEX media-server integration.
- One authoritative desk shared by the Tauri desktop app, browser clients, keyboard, OSC, and
  attached hardware surfaces.

## Development

Build and open the complete desktop application through the authoritative development path:

```sh
npm run build:open
```

This builds the frontend, Rust server, and Tauri applications, starts the app-owned server, checks
its readiness, and opens ToskLight. For later launches of that existing build, use `npm run open`;
it does not rebuild or stop an already-ready server.

| Task | Command | Result |
| --- | --- | --- |
| Build the operator manual | `npm run manual` | Verified PDF and deployable HTML manual under `.artifacts/generated/manual/` |
| Bundle release artifacts | `npm run bundle` | Desktop packages and standalone server archives under `.artifacts/release/` |
| Bundle and install locally | `npm run bundle:install` | Builds the archives and installs/opens the macOS application |
| Build the public site | `npm run pages:generate` | Landing page, manual, Code Safari, semantic test catalog, and release assets under `.artifacts/generated/pages/` |
| Run the headless development server | `npm run dev:headless` | Embedded browser UI at `http://127.0.0.1:5000` |

See [Build and test commands](docs/engineering/build-and-test-commands.md) for the complete command
catalog and [Manual and help screenshots](docs/help/99-Development/04-manual-and-help-screenshots.md)
for the documentation workflow.

### Supported operating systems

| Platform | Desktop application | Standalone/headless server |
| --- | --- | --- |
| macOS | Apple Silicon (M1 and later) | Universal macOS archive |
| Windows | 64-bit installer | Windows AMD64 archive |
| Linux | x86_64 AppImage and Debian package | Linux AMD64 archive |
| Raspberry Pi | Use the browser desk from another computer | Pi 4 or Pi 5, 64-bit Raspberry Pi OS, ARM64 archive |

The macOS testing applications are ad-hoc signed so Gatekeeper can verify that each completed app
bundle is intact, but they are not yet Apple Developer-ID signed or notarized. The macOS bundle
includes `sign-macos-apps-locally.sh` to apply and verify a fresh ad-hoc signature on the Mac that
will run Control, Architect, or Pixel; no Apple identity is needed. The other release builds are
unsigned.
Cross-compiling the non-macOS server archives requires the Rust targets used
by the build script, `zig`, and `cargo-zigbuild`. Native device and protocol integrations run as
separately approved extension packages; OSC and network output remain built in.

## How ToskLight is structured

ToskLight is a Rust workspace with TypeScript/React operator interfaces hosted by Tauri. Every
control surface enters the same application command path before the domain model and immutable
engine snapshots resolve Programmer, Playback, Group, Cue, and Dynamic contributions. Output runs
on a fixed-deadline scheduler and publishes Art-Net or sACN without waiting for browser projection
or visualization work.

- `crates/light/domain/` owns Programmer, Playback, engine, and output semantics.
- `crates/light/adapters/headless/` owns REST, WebSocket, OSC, sessions, persistence, and server
  orchestration.
- `apps/light-desktop/` is the ToskLight Control operator application.
- `apps/light-hardware-controls/` is the sibling application for attached controls.
- `apps/ui-library/` contains shared operator components and deterministic Storybook surfaces.
- `tests/` contains the semantic Playwright acceptance suite and shared test bench.

Read the [Code Safari](https://kellertobias.github.io/tosklight/safari/) for the dependency rules
and a guided route through the codebase.

### Show files and desk data

ToskLight stores portable shows as versioned SQLite `.show` files. They contain fixtures, patch,
Groups, Presets, Cuelists, Cues, Dynamics, routes, and other show-owned objects and can move between
desks.

`desk.sqlite` is deliberately separate. It stores desk users, settings, screens, the show-library
index, the active show, retained revisions, and durable Programmer recovery. A show file must never
be treated as a copy of the desk database. Persisted changes follow the
[backward-compatibility acceptance criteria](docs/acceptance-criteria.md): existing valid shows
must load or migrate without losing the original unless a compatibility break is explicitly
accepted.

## Tests and engineering documentation

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e-ui
npm run test:e2e -- tests/<focused-spec>.spec.ts
npm run test:desktop-smoke
```

The suite separates Rust and TypeScript unit/architecture checks, API acceptance, real-browser UI
behavior, deterministic Storybook screenshots, generated help screenshots, product-demo
generation, packaged desktop checks, and release performance evidence.

- [Semantic test catalog](https://kellertobias.github.io/tosklight/semantic-tests/semantic-test-catalog.html)
  — searchable, human-readable contracts compiled statically from the Playwright suite.
- [Code Safari](https://kellertobias.github.io/tosklight/safari/) — guided architecture,
  operator-action, persistence, testing, and Rust/Tauri tours.
- [Agent engineering runbooks](docs/engineering/) — API, build, model, performance, and test
  contracts used while changing the repository.
- [Human-readable acceptance scenarios](docs/testing/) — operator-facing behavior and surface
  parity.

## Scale and limits

The canonical demo is a realistic, release-blocking packaged-desktop show with **231 controllable
fixtures**, **264 physical lighting instances**, **2,988 occupied DMX slots**, 38 Groups, 30 Presets,
30 Dynamics, seven Cuelists, and 13 Playbacks.

| Profile | Contract |
| --- | --- |
| Typical show / built-in Stage | Around 300 fixtures or fewer; the 3D Stage is expected to remain real-time. |
| Supported interactive ceiling | Exactly 1,000 physical instances at 60 Hz on sufficiently fast supported hardware. Programmer, Fixture Sheet, Playbacks, navigation, and output remain usable and real-time; Stage may reduce detail or stutter, but must remain bounded. |
| Output profiles | A representative supported show prefers 16 or fewer substantially occupied universes. Up to 32 occupied universes is the supported upper stress profile; 64 universes is optional capacity evidence. |
| Beyond the supported profile | 2,000- and 4,000-fixture headless runs are breakpoint probes, not interactive Stage claims. In one Apple M5 Max measurement, 2,000 fixtures sustained 60.20 ticks/s; 4,000 reached 39.63 ticks/s with drops, deferrals, and missed deadlines. Those numbers are evidence from that machine, not universal maxima. |

Fixture count alone is not enough to characterize a show. Release evidence also varies occupied
universes, active Cuelists and fades, Dynamics, and Dynamic lane count. A 20-Dynamic workload is
representative, not a product maximum.

<details>
<summary>Detailed performance and responsiveness contract</summary>

### Output and control priority

Engine evaluation and DMX/network output have absolute priority over API projection,
serialization, browser rendering, Fixture Sheet work, and visualization:

- A client, stalled connection, or expensive projection must never delay an output tick or change
  the cadence of a running Cue fade or Dynamic.
- Engine and output code must not await visualization or client-specific work.
- Non-critical publication uses bounded latest-value delivery and discards superseded samples
  before they create backpressure.
- Software, keyboard, OSC, and attached-hardware actions must reach their authoritative outcome
  and, when applicable, the first output frame containing the change within two configured output
  ticks at rates up to 60 Hz, or four ticks above 60 Hz.

Go and other Playback triggers are time-critical. Immediate local feedback accompanies the
authoritative action, but optimistic presentation never substitutes for the output-tick budget.
An operated encoder, fader, or control must respond continuously without jumping while the event
stream reconciles it.

### Warm bundled UI and sampled telemetry

The bundled desktop keeps ordinary operator surfaces warm. Fixture Sheet, Fixture Built-ins,
Preset Built-ins, Group Built-ins, Cuelist Built-ins, and Dynamic Built-ins must appear without a
loading screen or blocking fetch. A previously unused Stage, very large Timecode editor, or another
genuinely heavy cold surface may show explicit loading progress. A browser-connected desk may take
approximately 500 ms for equivalent cold navigation; the bundled frontend remains the primary
responsiveness contract.

- Current Cue and Playback state, visible Playback faders, command line, selection, and controls
  being operated update immediately.
- Cue fade progress animates locally from authoritative start, duration, pause, speed, and
  completion events.
- Values changed only by an external fade, Cue, or Dynamic may use a 5 Hz latest-value sample and
  skip intermediate values while remaining eventually consistent.
- Fixture Sheet opens and scrolls from warm identity, structure, base/programming values, and
  cached state. Newly visible rows may initially show cached values, but refresh within 100–200 ms.
- Fixture Sheet shows stable base/programming values plus Dynamic identity and state; the DMX
  window owns high-cadence inspection of resolved output.
- Hidden surfaces, off-screen rows, and invisible columns do not subscribe to high-frequency
  values.

These rules allow telemetry and visualization to become less detailed under load while the
operator's input path, Cue/Playback state, and physical output remain immediate and deterministic.

</details>

## License

ToskLight is free to use, copy, modify, and distribute, including for commercial productions and
paid installation, operation, support, or maintenance. You may not sell ToskLight itself or bundle
it with hardware sold as a product without a separate written license. Distributed modified
versions must publish their complete source under the same license; branding restrictions and the
no-warranty terms also apply.

Read the binding [ToskLight Community License](LICENSE) and the plain-language
[license FAQ](docs/license-faq.md).
