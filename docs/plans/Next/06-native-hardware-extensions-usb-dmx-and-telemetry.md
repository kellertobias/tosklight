# Native Hardware Extensions, Open USB DMX Protocols, and Telemetry

## Status and ownership

Planning only. The extension host, synthetic conformance packages, and built-in USB DMX drivers are
not implemented. USB DMX implementation begins only after the open-protocol survey below defines
the supported core drivers. The operator must still explicitly move the relevant phase into
execution.

This is the sixth item in the current [Next plan order](README.md), after the consolidated canonical
attribute, encoder-layout, settings, and control-screen work and before Macros. Phase 0 remains the
required entry gate; queue position does not substitute for identifying the open USB DMX protocol
families and proving their implementations with automated tests.

This plan defines three related but deliberately separate product boundaries:

1. a versioned, UI-less host for native hardware extensions installed without rebuilding
   ToskLight, replacing built-in native MIDI, RTP-MIDI, and generic MIDI mapping;
2. built-in USB DMX output for major openly documented protocol families, owned by the normal
   output engine; and
3. typed hardware telemetry that Macros may later consume and publish through the capability-scoped MQTT API
   defined in [Macros](11-macros-and-scheduled-macros.md).

It does not make DMX output an extension capability, put reverse-engineered controller protocols
in the main ToskLight source tree, or give Macro JavaScript process, USB, serial, HID, MIDI, socket,
or filesystem access.

## Goals

- Install or update a controller integration without rebuilding ToskLight.
- Support bidirectional attached desks over USB MIDI, raw USB HID, and USB serial, including every
  press/release, fader, encoder, motor, display, lamp, blink, and RGB-color state the hardware can
  represent.
- Keep controller-specific and reverse-engineered protocols in separately distributable packages.
- Remove native MIDI and RTP-MIDI transport, discovery, parsing, mapping, settings, and build
  dependencies from the ToskLight core once extensions provide their replacement boundary.
- Automatically discover valid packages from a predictable Extensions folder.
- Reuse the authoritative typed control-surface actions and feedback projections already shared by
  software, OSC, and ToskLight Hardware Controls.
- Let a native adapter provide normalized external-timecode frames through a typed
  `timecode_source` interface, so MIDI- or RTP-MIDI-based adapters can supply
  timecode without restoring direct MIDI support in the core.
- Isolate extension crashes, hangs, malformed messages, and incompatible versions from the server,
  Programmer, playback, and output scheduler.
- Admit typed sensor streams for later capability-scoped Macro consumption without launching a
  helper binary from Macro code.
- Support the practical major USB DMX protocol families whose complete wire behavior is openly
  documented, with the same lifecycle and safety expectations as Art-Net and sACN.

## Non-goals

- Loading third-party dynamic libraries into the ToskLight process.
- An extension marketplace, automatic internet downloads, or unreviewed self-update.
- Allowing extensions to mutate raw show JSON, read databases, or bypass typed services.
- Letting extensions generate or receive DMX frames.
- Rendering extension-supplied HTML, CSS, React, browser JavaScript, or native windows.
- Shipping a controller-specific, GM1356, generic MIDI, or RTP-MIDI extension in this repository.
- Providing native extensions with settings pages, custom panes, or extension-defined UI in the
  first version; configuration is file-based.
- Running arbitrary executables from Macros.
- Retaining generic MIDI-message-to-action mapping in the main application after the extension
  migration.

## Architectural decision

Use **supervised out-of-process native extension packages**, not in-process Rust/C dynamic
libraries.

An extension package is a folder containing a declarative manifest plus one executable for each
supported platform. ToskLight starts the selected executable as a child and communicates over a
private, framed, versioned RPC channel. A crash or ABI mismatch therefore ends one adapter rather
than the desk process. The protocol must work with independently built Rust, C/C++, Swift, or other
native executables; it must not expose Rust trait objects or a compiler-specific ABI.

The extension owns its complete device and transport implementation. It may internally use MIDI,
RTP-MIDI, HID, serial, USB, or networking without the host knowing or modeling that transport.
ToskLight owns supervision, configured enablement, logical desk association, action authorization,
state projection, rate limits, health, logs, and cleanup. Extensions never call public HTTP or OSC
as an internal shortcut.

### Retained core protocols

Keep HTTP/REST, WebSocket, OSC, the internal typed application-service boundary, and protocols
owned by core product applications such as Media. Art-Net and sACN remain core DMX output, and each
accepted openly documented USB DMX protocol joins that output boundary. Closed or insufficiently
documented dongle protocols remain external native-extension concerns. Existing non-MIDI timecode
sources remain core where their product owner requires them.

Extensions call the same typed application services internally through the extension host. They do
not translate hardware input into REST requests or OSC packets, and they do not subscribe to OSC
feedback to discover lamp state. This preserves one action/feedback authority without making a
public network protocol the plugin ABI.

## Package discovery and installation

### Effective Extensions folder

Resolve one operator-visible **Extensions folder** through a platform helper:

- portable/headless archives use `extensions/` beside the ToskLight executable;
- signed application bundles use the installation application-data folder because writing beside
  a signed bundle may be unavailable or invalidate platform trust; and
- development uses a canonical path below `.artifacts/runtime/`, resolved through repository
  artifact-path helpers rather than a new root directory.

The operator documentation and startup diagnostics report the exact effective path. On start and
on an explicit headless/desktop rescan command, inspect immediate child folders only. Do not
execute partially copied files. Installation/update validates a complete staged package and
atomically replaces its folder. No Desk Settings page is added for native extensions in this
version.

Each folder contains `extension.json`, platform executables, licenses/notices, and optional
read-only assets. The stable manifest `id` is the installation identity; the folder name cannot
create a second identity for the same extension.

### Manifest and trust

The manifest declares:

- stable reverse-DNS ID, name, vendor, version, description, and license/source links;
- extension protocol and minimum/maximum host API versions;
- entry executable, OS, architecture, and SHA-256 for every artifact;
- `control_surface`, `telemetry_source`, `timecode_source`, or a declared combination;
- optional descriptive transport/device metadata for diagnostics; this does not constrain how the
  executable implements its transport;
- strongest available device matches when the host participates in assignment: USB VID/PID, HID
  usage, MIDI endpoint identity, serial USB VID/PID and serial number;
- logical controls, feedback features, telemetry channels, configuration-schema version,
  multiplicity, and limits; and
- reverse-engineering notice and optional signature metadata.

Unsupported required APIs, missing executables, digest mismatches, duplicate IDs, and invalid
declarations remain disabled with actionable errors.

Discovery is automatic; first execution is not silently trusted. A versioned installation-owned
configuration file explicitly enables an extension digest, binds its desk/device identity where
needed, and supplies extension-defined configuration values. New or materially changed packages
remain disabled until that file approves their new digest. Grants and configuration never travel
in a show. Locally approved unsigned packages may be supported initially but must be reported as
such in diagnostics. Platform quarantine, signing, HID, and serial-permission failures need
specific documentation.

## Extension lifecycle and isolation

Introduce an application-owned `ExtensionHost` in the headless/server process. The desktop is a
management client, so headless and desktop installations behave alike.

For each enabled package/device instance, the host:

1. creates a private authenticated IPC channel;
2. starts the child with opaque instance/channel tokens and no show secrets in arguments;
3. requires an identity, digest, protocol, capability, and instance handshake;
4. sends the full logical-desk/telemetry configuration and feedback snapshot;
5. accepts validated typed messages until shutdown or failure;
6. requests graceful release, then terminates an unresponsive child after a bounded timeout; and
7. records bounded logs, health, crashes, protocol errors, drops, and restart backoff.

Use length-prefixed messages over stdin/stdout or a private local socket/pipe. Standard error is
logs only. Cap frame size, queue depth, rate, log size, and handshake time. Reject invalid controls
without mutation. Repeated crashes back off and eventually require operator restart.

Server restart, show change, desk reassignment, disable, and disconnect have explicit idempotent
cleanup. A show change refreshes feedback but does not reinstall or reapprove a package.

## Device claims and configuration

Extensions may enumerate candidates but do not claim ambiguous matches. The installation
configuration file binds one physical identity to one extension instance and, for a control
surface, one desk alias/ID. Prefer USB serial numbers or stable identities over `/dev/tty*`, COM
numbers, or transient OS paths.

One device has one owner. Report conflicts with another extension, Hardware Controls, or built-in
USB DMX. Hotplug reconnect uses the stored identity; an indistinguishable replacement requires
confirmation.

The extension executable owns transport behavior. The following are examples, not host capability
requests or a closed transport list:

- MIDI may open input and output endpoints for controller-specific bidirectional protocols;
- HID may use reports, feature reports, usage information, and VID/PID/serial matching;
- serial may configure baud, parity, flow control, and framing; and
- combined transports must be declared.

An extension may implement RTP-MIDI or another network protocol internally. ToskLight neither
provides an RTP-MIDI extension nor models RTP-MIDI as a special host transport.

MIDI ports are extension-owned after migration. ToskLight does not enumerate them, open them, show
them as a built-in input, or feed a generic mapping table. One extension may deliberately implement
a MIDI adapter in another repository, but no generic configurable MIDI extension is part of this
plan.

## Remove built-in MIDI and RTP-MIDI

Delete the core implementations rather than leaving a disabled parallel path:

- remove native `midir` discovery/input and its `native-midi` Cargo features and dependency
  propagation from control, engine, headless runtime, application binaries, CI, and archives;
- remove the in-core RTP-MIDI session/parser/input implementation and bind lifecycle;
- remove `MidiControlInput`, `RtpMidiInput`, `ControlEvent::Midi`, `ControlTrigger::Midi`, MIDI
  `ActionSource`/wire variants, action-source routing, mapping tests, and MIDI-specific input
  startup/restart logic;
- remove `midi_inputs` and `rtp_midi_bind` from installation configuration, API/wire patches,
  generated clients, setup state, Storybook fixtures, and **Network & Inputs**;
- remove MIDI/RTP-MIDI default timecode-source entries and documentation claims; and
- update release dependencies so Linux builds no longer need ALSA development packages solely for
  MIDI and portable builds no longer need a special no-native-MIDI feature split.

Keep the generic typed timecode service, source priority/fallback model, and the OSC/ArtTimeCode or
other non-MIDI sources still owned by core. Add `timecode_source` to the extension protocol so a
USB MIDI or RTP-MIDI package can publish normalized SMPTE frames with source identity, frame rate,
sequence, and receive time. The core validates and routes those frames exactly as it does other
typed timecode input; the extension owns MIDI parsing and network session behavior.

Keep OSC control mappings as an operator-facing core OSC feature. Narrow the persisted
`ControlMapping` trigger schema to OSC rather than retaining MIDI variants or a generic byte-message
abstraction. Before removal, inventory existing shows containing MIDI mappings. Since an extension
package and its installation approval cannot be inferred from a raw MIDI status/data pair, do not
silently translate them. Create a recovery backup and an operator-visible migration report
containing each old mapping name, MIDI bytes, and target action, then remove the unsupported
mappings. Do not migrate them into an extension, leave a disabled compatibility path, or block
loading the rest of the show. Test this destructive schema migration with a legacy show fixture
under the repository's persisted-show migration policy.

## Typed control-surface protocol

A `control_surface` declares stable logical control IDs and types, not OSC paths or hardware packet
offsets. Initial types are buttons; absolute/motor faders; relative/absolute encoders and presses;
wheels; monochrome, variable-intensity, indexed-color, and RGB lamps; encoder rings; and short
text/value displays.

Configured IDs map to canonical typed intents: Programmer keys, modifiers, navigation, Highlight,
encoders, current/explicit-page Playbacks, Speed Groups, masters, and shared application commands.
Missing actions must be added to the shared contract first; never use synthetic clicks or raw
command strings.

Inputs carry instance/control ID, monotonic sequence, optional device time, and press, release,
absolute value, or relative delta. The host supplies authoritative desk/user context and source,
rejects stale or impossible input, and preserves press/release ordering and held state.

ToskLight pushes a full feedback snapshot after handshake/reconnect and ordered deltas afterward.
Per logical control, feedback answers:

- available, enabled, selected/active, warning, or error;
- lamp off/on/dim/blink with a bounded named pattern;
- semantic color plus resolved RGB where supported;
- fader/motor and encoder-ring values/styles;
- short labels or numeric text; and
- projection revision and desk/show generation.

The extension converts this projection into device packets and never calculates Cue, Playback,
Programmer, Highlight, or page state itself. Coalesce superseded feedback but preserve edge-sensitive
transitions. On overflow or reconnect, replace deltas with a snapshot. RGB hardware gets resolved
RGB; indexed hardware gets a deterministic palette match; monochrome gets semantic on/dim/blink.

## Typed telemetry protocol

A `telemetry_source` declares stable channel IDs, label, unit, scalar type, range, precision,
expected interval, and quality flags. Samples carry instance/channel ID, sequence, optional device
time, receive time, value, and quality. The host validates range/rate, retains a bounded latest
value/history, and publishes typed events. Telemetry never enters engine or DMX rendering.

The later Macro system receives telemetry only with explicit instance/channel grants. A Macro
Service may subscribe, derive bounded state, publish a declarative custom pane, and separately
publish to an approved MQTT binding. Do not expose generic byte streams to JavaScript. Device
commands, if needed, are declared typed telemetry actions with schemas and permission checks.

A `timecode_source` publishes normalized timecode only. It cannot invoke control actions or publish
telemetry unless the manifest separately requests those capabilities. Invalid frame numbers,
non-monotonic/duplicate packets, loss, reconnect, source priority, and fallback use the existing
authoritative timecode service rather than extension-specific desk state.

## Synthetic telemetry conformance extension

Do not build or bundle the GM1356 integration in this repository. It is a future external extension
and physical acceptance belongs to its own repository. The host must nevertheless prove that such
hardware can be supported without changing ToskLight.

Build a deterministic synthetic telemetry extension solely for automated conformance tests. It
publishes representative scalar channels, units, quality flags, cadence changes, stale/loss state,
reconnects, malformed frames, excessive rates, and typed device actions. It runs under the same
supervision and framed protocol as a real extension but owns no physical-device protocol and ships
only as test tooling under canonical artifact/test paths.

The conformance suite, protocol schema, SDK/example package, and documentation must be sufficient
for a separate repository to implement GM1356-class HID telemetry on macOS and Windows. Linux
native-extension support is deferred; built-in core USB DMX remains cross-platform.

## Macro telemetry and MQTT boundary

The canonical Macro plan owns telemetry consumption, Macro panes, and MQTT. This plan ends at the
typed telemetry host contract and synthetic conformance proof. A later external device extension
publishes typed samples; a permitted Macro Service may subscribe, render its own declarative pane,
and publish selected samples or aggregates to an approved MQTT binding.

MQTT is not simulated through HTTP and Macros cannot open raw sockets. Broker addresses, TLS,
credentials, topic grants, QoS, retain, payload/rate limits, and audit are installation-owned.
Macro source receives an opaque binding name, not a password. Process execution remains prohibited.

## Built-in open-protocol USB DMX output

Each accepted open-protocol USB DMX driver is a core output transport. It consumes final DMX frames
from the same bounded output scheduler as Art-Net/sACN and reports through Output Runtime/DMX
diagnostics. It cannot read Programmer state or run controller/telemetry code.

### Required open-protocol survey

Before driver implementation, identify the small set of major protocol families whose complete
wire behavior can be implemented from redistributable primary documentation. At minimum, evaluate
Open DMX/bit-banged FTDI and the openly documented Enttec DMX USB Pro framing family, including
compatible devices from other vendors. Add other major open families only when they are materially
distinct and bounded enough to implement and verify without vendor code or reverse engineering.

For each accepted family, record framing, universe count, refresh, break and mark-after-break
behavior, latency, buffering, receive support, VID/PID matching limits, serial identity, drivers,
and macOS/Windows/Linux behavior. A protocol with unavailable, incomplete, confidential, or
reverse-engineered-only documentation is not built into core; its device belongs in an external
native extension. Supporting several documented families must not become a generic serial-byte
configuration UI.

### Persistence and safety

Keep portable show intent separate from installation hardware identity. Refactor the network-shaped
`OutputRoute` into a compatible tagged destination model:

- existing Art-Net/sACN routes retain exact behavior and migration coverage;
- a portable USB-DMX route references a stable installation endpoint ID and universe; and
- the endpoint maps to one accepted core protocol driver and stable USB identity, never a
  transient device path.

A missing endpoint leaves the route intact, sends nothing, and reports an error. Never fall back to
the first FTDI device. One transmitter cannot serve two active claims.

Use a bounded latest-frame queue per device and keep USB-driver calls off engine locks. Report sent
frames, coalescing/drops, last success/error, reconnect state, and refresh rate. Deliberately test
final-zero and latch behavior on disable, show close, shutdown, and loss; do not promise blackout
when hardware cannot guarantee it. Reconnect sends the newest authoritative frame.

Desk Setup > Outputs owns endpoint assignment/routes. The DMX pane remains monitoring, override,
and diagnostics. Setup actions need visible progress and actionable errors.

## Diagnostics and API

Native extensions receive no settings page, custom pane, configuration form, or extension-supplied
UI in this version. Installation, digest approval, device/desk assignment, and extension-specific
values are edited in documented versioned configuration files. Configuration parse/validation
errors must name the file and field and must never start a partially valid extension.

Application-owned diagnostics may report extension and telemetry failures through the existing
Running & Output/status vocabulary without rendering extension-provided content. Logs remain
bounded and available through the documented runtime log location or headless diagnostics. Failed
controllers leave software controls working; failed telemetry becomes explicitly stale/disconnected
to typed consumers.

Follow `docs/engineering/api-rules.md`: request-identified object intents for safe management
actions, typed events for health/hotplug/telemetry, scoped snapshots and gap repair, generated wire
types, and one external conformance schema/SDK. Cover one previous protocol minor, reject unsupported
major versions, tolerate optional fields, and reject unknown required capabilities.

Executables, grants, physical identities, broker secrets, and logs never enter portable shows.

## Delivery phases

### Phase 0: hardware and protocol proofs

- Complete the USB DMX open-protocol survey and record which families belong in core.
- Prototype framed handshake, crash isolation, snapshot recovery, one synthetic control-surface
  round trip, and one synthetic telemetry stream.
- Freeze v1 manifest/RPC only after macOS and Windows host proof.

### Phase 1: extension host and conformance SDK

- Implement discovery, validation, approval, supervision, IPC limits, logs, health, claims,
  versioned configuration-file loading, and application-owned diagnostics. Native extensions
  receive no configuration UI.
- Test with synthetic control-surface and telemetry conformance extensions under `.artifacts/`, not
  a production controller.
- Verify install/update/rescan without a ToskLight rebuild.

### Phase 2: transport-neutral extension proof and removal of core MIDI

- Prove extension-delivered typed control input/output without using REST or OSC as an internal
  bridge. The synthetic extension may simulate MIDI-like controls, but ToskLight does not provide
  or special-case a MIDI/RTP-MIDI package.
- Remove all core native MIDI/RTP-MIDI code, configuration, UI, dependencies, feature flags, tests,
  and documentation listed above.
- Apply the explicit legacy MIDI-mapping migration/rejection policy with recovery evidence.

### Phase 3: bidirectional control surfaces

- Complete logical control/action/feedback contracts and configuration-file mapping schemas.
- Prove press/release, modifiers, pages, faders, encoders, lamps, blink, RGB degradation, reconnect,
  and desk isolation.
- Publish the conformance SDK/example and documentation needed for separately maintained
  controller repositories; no physical controller package ships here.

### Phase 4: telemetry host conformance

- Add telemetry contracts and the synthetic telemetry extension.
- Prove values, units, quality, bounds, rate limiting, stale/loss state, reconnect, device actions,
  malformed input rejection, and supervision on macOS and Windows.
- Leave Macro subscriptions, panes, MQTT, GM1356, and other real device packages to their owning
  later plan or external repository.

### Phase 5: built-in open-protocol USB DMX

- Add endpoint persistence, compatible route schema, every accepted open-protocol driver,
  scheduler integration, setup UI, diagnostics, recovery, and automated driver/framing tests on
  macOS, Windows, and Linux.
- The operator owns later physical dongle/output acceptance; absence of currently connected
  hardware is not a blocker for completing the automated protocol implementation.
- Keep this independently shippable from extension and Macro work.

## Verification and acceptance

1. Adding a valid package to the Extensions folder and rescanning works without rebuilding.
2. Invalid, changed, duplicate, incompatible, or unapproved packages never execute.
3. Crashing, hanging, flooding, malformed, or oversized extensions cannot block or exhaust ToskLight.
4. Disable, restart, shutdown, show change, update, and unplug clean up idempotently.
5. Grants/device bindings remain installation-owned; shows without removed MIDI mappings load
   unchanged, while affected legacy shows follow acceptance item 8.
6. Core binaries and schemas contain no native MIDI/RTP-MIDI transport, parser, settings, feature
   flags, `midir`/ALSA dependency, or MIDI trigger/event variant after migration.
7. A synthetic extension emulates transport-neutral control input and feedback through typed
   services without REST/OSC loopback; no MIDI or RTP-MIDI package is required.
8. Legacy MIDI mappings are reported, backed up, and removed without being silently reinterpreted
   as an extension assignment or preventing the rest of the show from loading.
9. Synthetic press/release reaches the same typed action and desk state as software/OSC.
10. Page, Playback, Programmer, Highlight, encoder, fader, and status changes produce correct full
   snapshots and ordered on/dim/blink/RGB deltas.
11. Indexed/monochrome color degradation is deterministic; reconnect never shows guessed state.
12. Two desks keep partial interaction/page state isolated under existing authority rules.
13. Ambiguous device identities are never claimed arbitrarily.
14. Extension timecode participates in existing source priority, loss, and fallback semantics.
15. The synthetic telemetry extension proves scaling, units, quality flags, loss, reconnect,
    malformed input rejection, and typed device actions on macOS and Windows.
16. Telemetry is typed, bounded, scoped, and absent from engine/output locks.
17. The host API and SDK are documented well enough for controller and GM1356-class integrations
    to be implemented and tested in separate repositories without changing the host protocol.
18. Extensions still cannot grant Macro code process, USB, MIDI, HID, serial, file, or raw-socket
    access.
19. Existing Art-Net/sACN routes retain behavior through schema migration.
20. USB DMX resolves only through its installation endpoint and never guesses a device.
21. Automated protocol/framing tests prove slots 1/512, zeros, changes, refresh, disable, shutdown,
    loss, reconnect, and each accepted protocol's documented edge cases on every supported OS.
22. Stalled USB DMX cannot degrade Programmer, network DMX, UI, OSC, or output benchmarks.
23. Diagnostics do not promise blackout behavior the hardware cannot guarantee.
24. Focused schema, conformance, API/gap, process-fault, hotplug, and driver tests pass per platform.
25. `npm run open`, readiness/logs, application-owned diagnostics, synthetic hardware feedback, and
    synthetic USB-device tests provide the repository-owned proof. Later physical dongle and
    controller acceptance belongs to the operator and external extension owners.

## Documentation deliverables

- Operator help for installing, approving, assigning, diagnosing, updating, and removing extensions.
- External SDK/protocol reference and conformance commands.
- Platform permission/trust guidance for HID, serial, unsigned packages, and USB DMX, plus guidance
  for external extension authors that choose to use MIDI or RTP-MIDI internally.
- Migration notes that built-in MIDI and RTP-MIDI were removed, including the backup/report path
  for discarded legacy mappings; do not promise a replacement package.
- Controller-extension repository template for protocols kept outside the main source.
- Help for every accepted built-in open USB DMX protocol family, compatible-device matching limits,
  and known lifecycle/blackout limitations.
