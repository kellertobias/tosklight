# Native Hardware Extensions, USB DMX, and Telemetry

## Status and ownership

Planning only. Do not implement this plan until its open hardware-identification items are resolved
and the operator explicitly moves the relevant phase into an execution queue.

This plan defines three related but deliberately separate product boundaries:

1. a versioned host for native hardware extensions installed without rebuilding ToskLight,
   replacing built-in native MIDI, RTP-MIDI, and generic MIDI mapping;
2. built-in FTDI-based USB DMX output owned by the normal output engine; and
3. hardware telemetry that Macros may consume and publish through the capability-scoped MQTT API
   defined in [Macros](../refactoring/pending/32-macros-and-scheduled-macros.md).

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
- Isolate extension crashes, hangs, malformed messages, and incompatible versions from the server,
  Programmer, playback, and output scheduler.
- Admit typed sensor streams so a Macro Service can show a custom pane and publish data to MQTT
  without launching a helper binary itself.
- Add the user's FTDI USB DMX device as a built-in, health-reported output route with the same
  lifecycle and safety expectations as Art-Net and sACN.

## Non-goals

- Loading third-party dynamic libraries into the ToskLight process.
- An extension marketplace, automatic internet downloads, or unreviewed self-update.
- Allowing extensions to mutate raw show JSON, read databases, or bypass typed services.
- Letting extensions generate or receive DMX frames.
- Rendering extension-supplied HTML, CSS, React, browser JavaScript, or native windows.
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

The extension owns its device protocol and transport library. ToskLight owns supervision, operator
approval, logical desk association, action authorization, state projection, rate limits, health,
logs, and cleanup. Extensions never call public HTTP or OSC as an internal shortcut.

### Retained core protocols

Keep HTTP/REST, WebSocket, OSC, the internal typed application-service boundary, and protocols
owned by core product applications such as Media. Art-Net and sACN remain core DMX output, and the
confirmed FTDI USB DMX transport joins that output boundary. Existing non-MIDI timecode sources
remain core where their product owner requires them.

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

Desk Settings shows the exact effective path and provides **Open Extensions Folder**. On start and
on **Rescan Extensions**, inspect immediate child folders only. Do not execute partially copied
files. Installation/update validates a complete staged package and atomically replaces its folder.

Each folder contains `extension.json`, platform executables, licenses/notices, and optional
read-only assets. The stable manifest `id` is the installation identity; the folder name cannot
create a second identity for the same extension.

### Manifest and trust

The manifest declares:

- stable reverse-DNS ID, name, vendor, version, description, and license/source links;
- extension protocol and minimum/maximum host API versions;
- entry executable, OS, architecture, and SHA-256 for every artifact;
- `control_surface`, `telemetry_source`, `timecode_source`, or a declared combination;
- requested MIDI, HID, serial, or combined transport classes;
- strongest available device matches: USB VID/PID, HID usage, MIDI endpoint identity, serial USB
  VID/PID and serial number, with operator-selected fallback only when unavoidable;
- logical controls, feedback features, telemetry channels, settings, multiplicity, and limits; and
- reverse-engineering notice and optional signature metadata.

Unsupported required APIs, missing executables, digest mismatches, duplicate IDs, and invalid
declarations remain disabled with actionable errors.

Discovery is automatic; first execution is not silently trusted. New or materially changed
packages require local approval showing executable digests, capabilities, device matches, and
source/license links. Grants are installation-owned and never travel in a show. Locally approved
unsigned packages may be supported initially but must be labeled. Platform quarantine, signing,
udev/group, HID, and serial-permission failures need specific guidance.

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

Extensions enumerate candidates but do not claim ambiguous matches. Desk Settings binds one
physical identity to one extension instance and, for a control surface, one desk alias/ID. Prefer
USB serial numbers or stable identities over `/dev/tty*`, COM numbers, or transient OS paths.

One device has one owner. Report conflicts with another extension, Hardware Controls, or built-in
USB DMX. Hotplug reconnect uses the stored identity; an indistinguishable replacement requires
confirmation.

The extension executable owns transport behavior:

- MIDI may open input and output endpoints for controller-specific bidirectional protocols;
- HID may use reports, feature reports, usage information, and VID/PID/serial matching;
- serial may configure baud, parity, flow control, and framing; and
- combined transports must be declared.

MIDI ports are extension-owned after migration. ToskLight does not enumerate them, open them, show
them as a built-in input, or feed a generic mapping table. One extension may deliberately implement
a generic configurable MIDI adapter as its own package, but its transport, mapping model,
persistence, UI declarations, compatibility, and MIDI feedback remain that extension's concern.

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
containing each old mapping name, MIDI bytes, and target action, then remove/disable the unsupported
mapping according to the repository's persisted-show migration policy. The plan is not complete
until that policy is chosen explicitly and tested with a legacy fixture.

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

Macros receive telemetry only with explicit instance/channel grants. A Macro Service can subscribe,
derive bounded state, publish a declarative custom pane, and separately publish to an approved MQTT
binding. Do not expose generic byte streams to JavaScript. Device commands, if needed, are declared
typed telemetry actions with schemas and permission checks.

A `timecode_source` publishes normalized timecode only. It cannot invoke control actions or publish
telemetry unless the manifest separately requests those capabilities. Invalid frame numbers,
non-monotonic/duplicate packets, loss, reconnect, source priority, and fallback use the existing
authoritative timecode service rather than extension-specific desk state.

## GM1356 first telemetry extension

Implement GM1356 as a separately distributed `telemetry_source`, not a spawned Macro helper and not
in the main repository. Preserve its license/notices and source link.

The linked reference identifies USB VID `0x64bd`, PID `0x74e3`, USB HID, eight-byte reports,
CONFIGURE `0x56`, MEASURE `0xb3`, A/C weighting, fast/slow, hold-max, and ranges. Its README calls
the measurement hundredths of a dB while its C implementation and independent implementations
divide the first two bytes by ten. Resolve this using physical capture, not assumption.

The spike must:

- capture descriptors and known-display/report pairs from the user's meter;
- verify report-ID handling on each platform;
- verify `/10` versus `/100`, byte order, flags, cadence, loss, and acknowledgements;
- test macOS, Linux, and Windows separately;
- expose `sound_level` in `dB`, weighting, response speed, range, max mode, connection, and quality;
  and
- reconnect without reporting a false zero or stale-good sample.

## Macro MQTT integration

The canonical Macro plan owns MQTT. The SPL workflow is:

1. the GM1356 extension publishes typed `sound_level` samples;
2. a permitted Macro Service subscribes;
3. its declared pane renders level, status, history, or thresholds;
4. the Macro publishes selected samples/aggregates to an approved MQTT binding; and
5. Macro failure stops MQTT/pane work but does not disconnect the meter extension.

MQTT is not simulated through HTTP and Macros cannot open raw sockets. Broker addresses, TLS,
credentials, topic grants, QoS, retain, payload/rate limits, and audit are installation-owned.
Macro source receives an opaque binding name, not a password. Process execution remains prohibited.

## Built-in FTDI USB DMX output

FTDI USB DMX is a core output transport. It consumes final DMX frames from the same bounded output
scheduler as Art-Net/sACN and reports through Output Runtime/DMX diagnostics. It cannot read
Programmer state or run controller/telemetry code.

### Required identification spike

Do not select a library or wire protocol from "FTDI-based" alone. Record product/vendor, VID/PID,
serial, FTDI chip, OS enumeration, and whether it is Open DMX/bit-banged FTDI, Enttec DMX USB
Pro-compatible framed serial, or another protocol. Record universe count, refresh, break and
mark-after-break behavior, latency, buffering, receive support, and OS drivers. Implement only the
confirmed protocol; later protocols get another core driver behind the same endpoint abstraction.

### Persistence and safety

Keep portable show intent separate from installation hardware identity. Refactor the network-shaped
`OutputRoute` into a compatible tagged destination model:

- existing Art-Net/sACN routes retain exact behavior and migration coverage;
- a portable USB-DMX route references a stable installation endpoint ID and universe; and
- the endpoint maps to driver kind and stable USB identity, never a transient device path.

A missing endpoint leaves the route intact, sends nothing, and reports an error. Never fall back to
the first FTDI device. One transmitter cannot serve two active claims.

Use a bounded latest-frame queue per device and keep FTDI calls off engine locks. Report sent
frames, coalescing/drops, last success/error, reconnect state, and refresh rate. Deliberately test
final-zero and latch behavior on disable, show close, shutdown, and loss; do not promise blackout
when hardware cannot guarantee it. Reconnect sends the newest authoritative frame.

Desk Setup > Outputs owns endpoint assignment/routes. The DMX pane remains monitoring, override,
and diagnostics. Setup actions need visible progress and actionable errors.

## Operator surfaces and API

Add **Desk Settings > Extensions** with effective folder actions, package/version/trust/capability
details, approval, detected/assigned devices, desk association, conflicts, health, latest sample,
restart count, bounded logs, Restart, and control/feedback/telemetry previews.

Add extension and telemetry failures to **Running & Output** without conflating them with DMX or
Macro errors. Failed controllers leave software controls working. Failed telemetry becomes
explicitly stale/disconnected.

Follow `docs/engineering/api-rules.md`: request-identified object intents for safe management
actions, typed events for health/hotplug/telemetry, scoped snapshots and gap repair, generated wire
types, and one external conformance schema/SDK. Cover one previous protocol minor, reject unsupported
major versions, tolerate optional fields, and reject unknown required capabilities.

Executables, grants, physical identities, broker secrets, and logs never enter portable shows.

## Delivery phases

### Phase 0: hardware and protocol proofs

- Identify the exact FTDI DMX device/protocol.
- Capture GM1356 reports and resolve scaling/platform behavior.
- Prototype framed handshake, crash isolation, snapshot recovery, and one HID round trip.
- Freeze v1 manifest/RPC only after supported-target proof.

### Phase 1: extension host and conformance SDK

- Implement discovery, validation, approval, supervision, IPC limits, logs, health, claims, and UI.
- Test with a synthetic conformance extension under `.artifacts/`, not a production controller.
- Verify install/update/rescan without a ToskLight rebuild.

### Phase 2: extension-owned MIDI and removal of core MIDI

- Prove extension-delivered USB MIDI control input/output and normalized MIDI Time Code without
  using REST or OSC as an internal bridge.
- Prove an extension-delivered RTP-MIDI timecode source only if RTP-MIDI remains a desired external
  package; it is not retained in core merely for compatibility.
- Remove all core native MIDI/RTP-MIDI code, configuration, UI, dependencies, feature flags, tests,
  and documentation listed above.
- Apply the explicit legacy MIDI-mapping migration/rejection policy with recovery evidence.

### Phase 3: bidirectional control surfaces

- Complete logical control/action/feedback contracts and mapping UI.
- Prove press/release, modifiers, pages, faders, encoders, lamps, blink, RGB degradation, reconnect,
  and desk isolation.
- Build the reverse-engineered controller in its separate repository/package.

### Phase 4: telemetry, GM1356, panes, and MQTT

- Add telemetry contracts and the external GM1356 extension.
- Add Macro telemetry/MQTT APIs in the canonical Macro plan.
- Build the SPL Macro Service/pane and prove stale/reconnect/MQTT behavior.

### Phase 5: built-in USB DMX

- Add endpoint persistence, compatible route schema, confirmed FTDI driver, scheduler integration,
  setup UI, diagnostics, recovery, and physical wire verification.
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
7. USB MIDI and optional RTP-MIDI work only through installed extensions and reach typed services
   without REST/OSC loopback.
8. Legacy MIDI mappings follow the explicit backed-up migration/rejection policy and are never
   silently reinterpreted as an extension assignment.
9. Physical press/release reaches the same typed action and desk state as software/OSC.
10. Page, Playback, Programmer, Highlight, encoder, fader, and status changes produce correct full
   snapshots and ordered on/dim/blink/RGB deltas.
11. Indexed/monochrome color degradation is deterministic; reconnect never shows guessed state.
12. Two desks keep partial interaction/page state isolated under existing authority rules.
13. Ambiguous MIDI/HID/serial devices are never claimed arbitrarily.
14. Extension timecode participates in existing source priority, loss, and fallback semantics.
15. Physical display/report fixtures prove GM1356 scaling, flags, loss, and reconnect on supported OSes.
16. Telemetry is typed, bounded, scoped, and absent from engine/output locks.
17. A permitted Macro renders the SPL pane and visibly handles stale/disconnected/resumed state.
18. MQTT grants enforce broker, TLS/secrets, topic, QoS/retain, payload, rate, queue, and cancellation.
19. Macros still cannot execute processes or open USB/MIDI/HID/serial/files/raw sockets.
20. Existing Art-Net/sACN routes retain behavior through schema migration.
21. USB DMX resolves only through its installation endpoint and never guesses a device.
22. Physical capture proves slots 1/512, zeros, changes, refresh, disable, shutdown, loss, and reconnect.
23. Stalled USB DMX cannot degrade Programmer, network DMX, UI, OSC, or output benchmarks.
24. Diagnostics do not promise blackout behavior the hardware cannot guarantee.
25. Focused schema, conformance, API/gap, process-fault, hotplug, and driver tests pass per platform.
26. Synthetic extension E2E is joined by final physical controller, GM1356, and FTDI acceptance.
27. `npm run open`, readiness/logs, Extensions settings, SPL pane, MQTT receiver, hardware feedback,
    and captured DMX provide real operator-path proof.

## Documentation deliverables

- Operator help for installing, approving, assigning, diagnosing, updating, and removing extensions.
- External SDK/protocol reference and conformance commands.
- Platform permission/trust guidance for MIDI, HID, serial, unsigned packages, and USB DMX.
- Migration notes that MIDI and RTP-MIDI moved out of core and identify the replacement extension
  packages where available.
- Controller-extension repository template for protocols kept outside the main source.
- Help for the exact supported FTDI USB DMX device/protocol and its limitations.
