# Native extensions and USB DMX implementation contract

This document records the Phase 0 decisions for TL-66. It is an implementation contract, not
operator help. Native extensions and USB DMX do not exist merely because this survey exists.

## Product boundaries

ToskLight has two separate integration boundaries:

- Native controller, timecode, and telemetry integrations are supervised, out-of-process extension
  packages. They exchange typed actions and projections with the host and never send or receive DMX
  frames.
- Openly documented USB DMX output is a built-in output transport. It consumes the same final
  logical-universe frames as Art-Net and sACN and never reads Programmer or Playback state.

Controller-specific and reverse-engineered protocols remain external packages. A USB DMX protocol
that cannot pass the documentation gate is unsupported until the product explicitly adds a DMX
extension capability; it must not be smuggled through the control-surface protocol.

## Accepted built-in USB DMX families

Phase 5 implements exactly these two families:

1. Open DMX-compatible host-timed FTDI UART output.
2. ENTTEC DMX USB Pro API v1.44 output, including devices whose vendor documentation explicitly
   claims compatibility with that baseline.

The driver selector is typed. It is not an arbitrary serial-byte editor, and VID/PID alone never
selects a driver or claims a device.

### Open DMX-compatible output

Open DMX is a USB-to-UART/RS-485 design without an on-device frame buffer. The host owns all DMX
timing. It is not FTDI asynchronous bit-bang mode.

The implementation contract is:

- one output-only universe;
- 250,000 baud, 8 data bits, no parity, 2 stop bits, no flow control;
- BREAK low for at least 92 microseconds;
- MARK-after-BREAK high for at least 12 microseconds;
- start code `0x00`, followed by exactly 512 channel bytes;
- 40 frames per second; and
- a bounded latest-frame queue that replaces obsolete pending frames.

Use the operating system's VCP/serial API and its BREAK control. Do not bundle or link FTDI D2XX
without a separately reviewed licence decision. CPU scheduling and USB latency mean the driver
cannot promise that every host frame appears electrically, or that unplug can transmit a final
zero frame. Diagnostics must say so. Reconnect sends the newest authoritative frame.

The current [OPEN DMX USB datasheet](https://cdn.enttec.com/pdf/assets/70303/70303_OPEN_DMX_USB_DATASHEET.pdf)
documents one output universe, host-processed output, VCP enumeration, and a maximum 40 fps. The
electrical timing values above deliberately meet DMX512-A transmitter minima rather than copying
historical example timing.

### ENTTEC DMX USB Pro v1.44-compatible output

The serial envelope is:

```text
0x7e | label:u8 | payload_length:u16-le | payload | 0xe7
```

Payload length is `0..600`. The core output path uses:

- label 3 to read widget parameters;
- label 4 to configure parameters;
- label 6 for output-only DMX; and
- label 10 to read the widget serial number.

A label-6 payload is `0x00` plus up to 512 DMX channel bytes. It has a documented payload range of
25–513 bytes. The driver sends a full 513-byte payload unless a future measured reason establishes
otherwise.

The label-4 parameter payload starts with the user-configuration length LSB/MSB, then BREAK and MAB
in 10.67-microsecond units, then refresh rate. Use these safe defaults:

- BREAK 9, approximately 96 microseconds;
- MAB 2, approximately 21.3 microseconds; and
- refresh 40 fps.

MAB 1 is not used because 10.67 microseconds is below the selected 12-microsecond minimum. The
device owns output timing and continually transmits its newest buffer. Host updates can be dropped;
one serial packet is not guaranteed to equal one emitted DMX frame. Any request other than label 6
or label 3 stops periodic output and returns the shared port to input, so unrelated probes are not
sent while output is active.

The complete baseline is the
[ENTTEC DMX USB Pro API v1.44](https://cdn.enttec.com/pdf/assets/70304/70304_DMX_USB_PRO_API.pdf).
ENTTEC's [buffering guidance](https://support.enttec.com/dmx/usbdmx-dmx-usb-pro-70304/dmx-usb-pro-api)
confirms latest-buffer-wins behavior and the absence of host-frame/output-frame synchronization.

The DMXKing ultraDMX Micro is accepted under this family because its
[vendor manual](https://dmxking.com/downloads/ultraDMX%20Micro%20User%20Manual%20%28EN%29.pdf)
declares ENTTEC-compatible behavior. Other products enter this family only with equivalent primary
vendor documentation. Baseline compatibility does not imply support for DMXKing enhanced
multi-universe messages or ENTTEC Pro Mk2 multi-port behavior.

## Explicit exclusions

- **uDMX** is not built into core. Its published USB requests are materially distinct, but the
  [official firmware](https://raw.githubusercontent.com/mirdej/udmx/master/firmware/main.c) emits
  an 88-microsecond BREAK and 8-microsecond MAB, below the selected transmitter minima. The
  published stack is GPL and the common VID/PID plus strings do not provide safe identity.
- **ENTTEC Pro Mk2 multi-port mode** is excluded until a complete redistributable primary protocol
  document is available. One-port v1.44 behavior is not evidence for two-port semantics.
- **DMXKing enhanced messages** are excluded from the baseline driver.
- Nicolaud/Sunlite, Daslight, Velleman, Eurolite, DMX4ALL, and other vendor protocols are excluded
  unless their vendor supplies a complete, versioned, redistributable wire specification.
- Third-party OLA, QLC+, or similar implementations are interoperability evidence, not the primary
  documentation gate and not a source to copy.

## Stable identity and hotplug

Installation configuration binds a stable endpoint ID to one physical device and driver kind.
Portable show routes refer only to that endpoint ID.

For both accepted families, retain:

- USB VID and PID;
- manufacturer and product strings when present;
- USB serial when present;
- ENTTEC label-10 serial when supported; and
- an installation-local port-topology hint only when no unique serial exists.

The label-10 serial is a four-byte unsigned binary integer, least-significant byte first.
`0xffffffff` is explicitly unprogrammed and not unique. Generic FTDI VID/PID is never sufficient
because EEPROM identity is configurable and unrelated FTDI products may share it. A path such as
`/dev/tty*` or `COM4` is a transient locator, never identity.

On hotplug, reopen only an exact configured identity. An ambiguous match stays offline and asks for
confirmation. Never fall back to the first device. One physical identity cannot satisfy two active
endpoint claims, including a claim held by a native extension or Hardware Controls.

## Persistence split

USB routes are portable show objects:

```text
logical universe -> stable endpoint ID
```

Installation data resolves:

```text
stable endpoint ID -> driver kind + stable USB/protocol identity
```

Do not store OS paths in a show. Missing installation endpoints leave portable routes intact, send
nothing, and publish an actionable diagnostic. Endpoint edits use request IDs, an expected
document revision, and typed object-intent updates under `docs/engineering/api-rules.md`.

The initial endpoint document is a separate revisioned installation setting rather than a
whole-`DeskConfiguration` rewrite. Real startup tests must cover legacy/missing data, malformed
data recovery, idempotence, and preservation of unrelated installation settings.

## Output integration seam

The renderer already hands a coherent route/frame snapshot to `NetworkOutput::send_routes` after
releasing the output-control lock. Phase 5 generalizes this delivery owner rather than adding USB
logic to Playback or the scheduler:

- network routes continue through Art-Net/sACN packet encoding and UDP sockets;
- USB routes enqueue the newest final logical frame into one worker per endpoint;
- enqueue is non-blocking and never performs USB/serial I/O on the render path;
- a failed USB endpoint does not stop healthy network routes, and a failed network route does not
  stop USB; and
- endpoint health joins the existing authenticated output/runtime diagnostics.

Open DMX workers generate BREAK/MAB and transmit each frame. Pro-family workers parse framed
replies, configure timing, and update the device buffer. Shutdown and unplug reporting must
distinguish “final frame confirmed”, “device retains last frame”, and “final output unknown”.

## Deterministic USB DMX verification

### Shared enumeration and lifecycle

Feed deterministic snapshots through a mockable enumerator:

```text
absent -> present -> path changed/same identity -> ambiguous duplicate -> unplug -> reconnect
```

Tests prove stable identity across path changes, no first-device fallback, duplicate-claim
rejection, bounded queue behavior, newest-frame resend, and isolation between routes. A PTY can
prove byte behavior but cannot prove physical BREAK/MAB timing and must never be presented as
electrical acceptance.

### Open DMX emulator

A fake BREAK-capable serial transport and fake clock verify:

- exact 250000/8N2/no-flow configuration;
- BREAK at least 92 microseconds and MAB at least 12 microseconds;
- exact start code and 512 channel bytes;
- 40 Hz pacing;
- coalescing under pressure; and
- unplug at every BREAK/write boundary without a false blackout claim.

### Pro-family emulator

An in-memory and PTY-capable emulator verifies:

- golden frames for labels 3, 4, 6, and 10;
- every input fragmentation boundary and multiple messages per read;
- garbage before `0x7e`, payloads over 600, bad terminators, and reconnect mid-frame;
- parameter/serial replies and unprogrammed serial handling;
- latest-buffer-wins behavior; and
- the documented output-mode stop caused by unrelated requests.

Physical acceptance on macOS, Windows, and Linux still requires a real accepted device, DMX
observation, disconnect/reconnect, disable, show close, and shutdown. Automation proves framing and
lifecycle; it does not invent electrical evidence.

## Native-extension migration seam

The extension host is a separate phase, but its Phase 0 boundary is now fixed:

- Keep portable OSC control mappings and typed timecode routing in core.
- Extension input enters through normalized typed actions/timecode, then calls existing application
  services with host-owned desk/user/source provenance.
- Feedback is a typed replaceable snapshot plus ordered deltas, not OSC packets and not engine
  access.
- Built-in native MIDI and RTP-MIDI implementations are removed after the host conformance suite;
  never run a replacement extension and another transport against the same input.
- MIDI show mappings cannot infer an extension package. Their destructive migration requires a
  recovery backup and operator-visible report before removing unsupported mappings.
- Installation migration preserves exact port/bind values, unrelated configuration, and readiness
  when an extension executable or physical device is missing.

The extension host uses a versioned private IPC contract, fake in-memory host tests,
manifest/digest validation, bounded queues, supervision, and health. Legacy transport removal is a
separate compatibility migration with recovery evidence, not an implicit side effect of discovery.
