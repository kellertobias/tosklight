# Native extension SDK and conformance guide

This is the external implementer reference for the draft ToskLight native-extension protocol.
The protocol remains draft until the Windows runtime proof tracked separately from TL-66 passes.
An extension is a supervised executable package, not a dynamic library, browser plugin, or public
HTTP/OSC client.

The normative Rust data types, validation, and bounded frame codec live in
`crates/light/contracts/extensions`. The host/package implementation lives in
`crates/light/adapters/extensions`. External implementations may use any native language as long as
they reproduce the wire and lifecycle rules below.

## Package layout and approval

One immediate child of the effective Extensions folder is one package:

```text
com.example.surface/
  extension.json
  bin/surface-macos-aarch64
  bin/surface-windows-x86_64.exe
  LICENSE
```

`extension.json` is strict schema version 1. It declares a reverse-DNS `id`, semantic package
version, vendor/licence, protocol and host-API ranges, every package file with its lowercase
SHA-256 digest, one executable per supported OS/architecture, capabilities, logical controls,
telemetry channels, device matches, feedback features, multiplicity, and input-rate limits. Paths
must be relative normal components. Symlinks, unlisted files, missing files, duplicate IDs, digest
mismatches, unsupported targets, and unknown fields reject the package before execution.

The installation-owned `extensions.json` is separate from shows. It contains:

- `version: 1`;
- `approved_packages`, mapping the exact manifest ID to its current package digest; and
- `instances`, each with a stable local ID, extension ID, enabled flag, logical desk ID/alias,
  optional stable device identity, extension settings, and `control_bindings`.

A changed digest is unapproved until the file explicitly names the new digest. Missing or malformed
configuration never prevents ToskLight readiness; it prevents affected launches and appears in
diagnostics. Never bind a device by a transient `/dev/tty*` or COM number when a serial or other
stable identity exists. One physical identity has one owner.

## Private framed channel

The host launches the selected executable with an opaque credential in its environment, not its
arguments. Standard input and output carry messages; standard error is bounded diagnostic text.
Each frame is:

```text
payload_length:u32-be | UTF-8 JSON payload
```

The maximum payload is 1 MiB. Every envelope carries the draft numeric protocol version and a
strictly increasing direction-local sequence. Unknown optional JSON fields are tolerated; an
unknown required capability or unsupported protocol major is rejected.

Handshake order is fixed:

1. host sends `HostHello` with a fresh per-launch challenge;
2. extension sends `ExtensionHello` with the exact extension ID, configured instance ID, approved
   package digest, capability set, and credential-derived challenge response;
3. host validates identity, digest, protocol/API compatibility, and exact capabilities;
4. host sends `Configure`, including the logical desk, settings, control bindings, telemetry
   declarations, and complete authoritative feedback snapshot; and
5. extension applies that snapshot before processing device input and may send a `Health` report
   with `ready` status. There is no separate `Ready` message and the host does not wait for one.

No input, telemetry, timecode, or feedback delta is valid before this sequence completes. A
deadline, malformed/oversized frame, sequence error, invalid identity, stalled pipe, or queue
overflow faults only this child. Restarts use bounded exponential backoff and a new challenge and
snapshot. Shutdown is requested gracefully, then the child is killed and reaped after the bounded
deadline.

## Control-surface capability

Manifest control IDs describe device-facing logical controls; installation `control_bindings` map
them to host-owned canonical intents. The supported intent families are Programmer keys,
modifiers, navigation, Highlight, encoders, current-page and explicit-page Playbacks, Speed Groups,
Grand Master, Blackout, and shared desk commands. Extensions may not supply an OSC path, HTTP route,
show object, user ID, or raw command string.

Inputs use one instance-wide monotonic `input_id`, an optional device timestamp, and exactly one of:

- button press or release;
- absolute normalized value from `0.0` through `1.0`; or
- non-zero relative delta.

The host rejects stale/duplicate IDs, undeclared controls, impossible values, and value/intent type
mismatches. It supplies the desk and `extension` action source and invokes the same application
services or desk-local event transport used by the software and attached hardware. Current-page
and explicit-page Playback addresses remain distinct.

Feedback is host authority. The initial full snapshot and every replacement snapshot have a
monotonic revision. Ordered deltas name their base and resulting revisions. Each control state can
carry availability, enabled/selected/warning/error, off/dim/on/slow-blink/fast-blink lamp state,
semantic colour plus resolved RGB, normalized motor/ring value, ring style, and short text. On
reconnect or overflow, discard local guesses and replace them from the next full snapshot.

Device degradation is deterministic: RGB hardware uses resolved RGB; indexed-colour hardware maps
that RGB to its fixed palette using a stable nearest-colour rule; monochrome hardware maps active
colour to on/dim/blink without inventing Cue or Programmer state. The extension translates feedback
to device packets but never calculates page, Playback, Programmer, Highlight, or output state.

## Telemetry and timecode capabilities

A telemetry declaration names a stable channel ID, label, quantity, explicit unit, value kind,
inclusive numeric bounds where applicable, precision, expected interval, and allowed quality
flags. Samples carry a monotonic sample ID, channel ID, source/device timestamp when available,
typed value, and quality. Host receive time is assigned by the host. Undeclared, mismatched,
non-finite, out-of-range, duplicate, or non-monotonic samples are rejected or counted invalid;
history and logs are bounded. Missing sequence numbers, excess rate, reported stale quality, and
elapsed expected intervals appear in health. Telemetry never enters the engine or DMX scheduler.

A package with only `timecode_source` may publish normalized SMPTE fields and cannot invoke controls
or telemetry. The host validates frame bounds and sequence/time monotonicity, then enters the
existing timecode priority, loss, and fallback router with the extension instance as source.

Macros, MQTT, real controller packages, and real sensor packages are outside this SDK. Macro code
does not receive process, USB, HID, serial, MIDI, filesystem, or socket access through an extension.

## Conformance commands

From the ToskLight repository root:

```sh
cargo test -p light-extensions-contract --locked
cargo test -p light-extensions-host --test supervised_child --locked
cargo clippy -p light-extensions-contract -p light-extensions-host --all-targets --locked -- -D warnings
```

The portable synthetic child in `crates/light/adapters/extensions/tests/support` is the executable
example. Its test modes cover authenticated handshake, snapshot-before-delta ordering, typed
control and telemetry, malformed and oversized frames, stale/lost/excess-rate samples, crash and
restart recovery, bounded queues/logs, and graceful or forced shutdown. External repositories
should port these cases against their codec and run them on every supported OS/architecture.

The host contract tests are not physical-device acceptance. A controller repository still owns
real HID/MIDI/serial/RTP lifecycle and feedback tests. Built-in USB DMX is a separate core output
boundary documented in `docs/engineering/native-extensions-and-usb-dmx.md`.
