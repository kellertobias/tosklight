# Developing Native Hardware Extensions

This chapter is the implementer handoff for ToskLight native hardware extensions. Give it to the
developer of a MIDI surface, HID controller, sensor, or timecode reader together with the current
ToskLight source tree. An extension may be written in any native language that can read and write
length-prefixed JSON on standard input and output.

The protocol is currently **draft version 1**. Pin an extension release to protocol 1 and host API
1, test it against the exact ToskLight release that will run it, and treat a future protocol or host
API version as a deliberate compatibility update.

## Choose the correct boundary

Native extensions are supervised child processes. They are not dynamic libraries, browser
plugins, Macro code, or public REST/OSC clients.

| Hardware requirement | Extension capability and direction |
| --- | --- |
| Buttons, faders, wheels, and encoders controlling the desk | `control_surface`; extension to host `control_input` |
| Lamps, RGB keys, motor faders, encoder rings, and displays reflecting desk state | `control_surface`; host to extension `feedback_snapshot` and `feedback_delta` |
| Measurements such as temperature or battery state | `telemetry_source`; extension to host `telemetry_sample` |
| A permission-gated command sent to a sensor or controller | `telemetry_source`; host `device_action_request`, extension `device_action_result` |
| SMPTE timecode received from hardware or a protocol | `timecode_source`; extension to host `timecode_sample` |
| Art-Net, sACN, Open DMX, or ENTTEC-compatible DMX output | Not an extension; use ToskLight's built-in output routes |

An extension never receives engine objects, fixture values, or DMX frames. A control extension
reports physical actions and renders host-owned feedback; it must not calculate Programmer,
Playback, page, Highlight, or show state. Vendor-specific USB, HID, serial, MIDI, or network I/O
stays inside the extension process.

## Build one package

Each immediate child of the effective **Extensions folder** is one complete package. The folder
name is conventional; identity comes from `extension.json`.

```text
com.example.surface/
  extension.json
  bin/surface-macos-aarch64
  bin/surface-windows-x86_64.exe
  LICENSE
```

Every regular file except `extension.json` must appear in `files`, and no undeclared file or
symlink may be present. Each `sha256` is the lowercase SHA-256 of that file's bytes. On Unix, make
the executable executable before packaging. Use `macos`, `windows`, or `linux` for `os`, and
`aarch64` or `x86_64` for the common architectures.

This shows every required manifest field for a bidirectional controller. The digest placeholders
make it intentionally invalid until they are replaced. Also replace the example identity and
device match. Remove the Windows artifact and file when shipping a macOS-only package, or vice
versa.

```json
{
  "manifest_version": 1,
  "id": "com.example.surface",
  "name": "Example Surface",
  "vendor": {
    "name": "Example Hardware",
    "url": "https://example.com"
  },
  "version": "1.0.0",
  "description": "Example bidirectional ToskLight control surface",
  "license": {
    "name": "Proprietary",
    "url": "https://example.com/license"
  },
  "source_url": null,
  "protocol": { "minimum": 1, "maximum": 1 },
  "host_api": { "minimum": 1, "maximum": 1 },
  "files": [
    {
      "path": "bin/surface-macos-aarch64",
      "sha256": "REPLACE_WITH_64_LOWERCASE_HEX_CHARACTERS"
    },
    {
      "path": "bin/surface-windows-x86_64.exe",
      "sha256": "REPLACE_WITH_64_LOWERCASE_HEX_CHARACTERS"
    },
    {
      "path": "LICENSE",
      "sha256": "REPLACE_WITH_64_LOWERCASE_HEX_CHARACTERS"
    }
  ],
  "artifacts": [
    {
      "os": "macos",
      "architecture": "aarch64",
      "executable": "bin/surface-macos-aarch64"
    },
    {
      "os": "windows",
      "architecture": "x86_64",
      "executable": "bin/surface-windows-x86_64.exe"
    }
  ],
  "capabilities": ["control_surface"],
  "controls": [
    { "id": "go", "kind": "button" },
    { "id": "master-1", "kind": "motor_fader" },
    { "id": "encoder-1", "kind": "relative_encoder" },
    { "id": "label-1", "kind": "text_display" }
  ],
  "telemetry_channels": [],
  "device_actions": [],
  "device_matches": [
    {
      "transport": "hid",
      "vendor_id": 4660,
      "product_id": 22136,
      "serial_number": null,
      "usage_page": 1,
      "usage": 5,
      "endpoint_identity": null
    }
  ],
  "transport_metadata": null,
  "feedback_features": [
    "availability",
    "enabled",
    "selected",
    "warning",
    "error",
    "lamp",
    "blink",
    "rgb_color",
    "motor_value",
    "encoder_ring",
    "text"
  ],
  "configuration_schema_version": 1,
  "multiplicity": "multiple",
  "limits": {
    "maximum_input_rate_hz": 500,
    "maximum_telemetry_rate_hz": 100
  },
  "reverse_engineered": false,
  "signature": null
}
```

Control kinds are `button`, `absolute_fader`, `motor_fader`, `relative_encoder`,
`absolute_encoder`, `wheel`, `lamp`, `rgb_lamp`, `encoder_ring`, and `text_display`. Capabilities are
`control_surface`, `telemetry_source`, and `timecode_source`. Multiplicity is `single` or
`multiple`. Manifest JSON is strict: unknown top-level package fields reject the package. Package
IDs have at least three lowercase reverse-DNS labels. Local control, channel, action, and instance
IDs are 1–96 ASCII letters, digits, `-`, `_`, `.`, or `:`. A device match must name a transport and
either a vendor ID or a stable endpoint identity.

The package digest shown by ToskLight is not just the executable digest. It hashes the exact
manifest bytes and the sorted file path/digest declarations. Install the package, rescan, and copy
the digest reported by ToskLight into installation approval; do not guess or independently approve
a digest supplied by the vendor.

## Configure and approve an instance

The operator owns `extensions.json` in the Light data folder. It is separate from the package and
from portable shows. A control-surface instance must name a logical desk. A physical device should
use a stable serial or endpoint identity, never a transient `/dev/tty*` path or `COM` number.

```json
{
  "version": 1,
  "approved_packages": {
    "com.example.surface": "REPLACE_WITH_DIGEST_REPORTED_BY_TOSKLIGHT"
  },
  "instances": [
    {
      "id": "surface-left",
      "extension_id": "com.example.surface",
      "enabled": true,
      "desk_id": "main",
      "device": {
        "identity": "hid:1234:5678:SERIAL-001",
        "transport": "hid"
      },
      "settings": {
        "brightness": 0.8
      },
      "control_bindings": {
        "go": {
          "kind": "playback_current",
          "slot": 1,
          "control": "button_one"
        },
        "master-1": {
          "kind": "playback_current",
          "slot": 1,
          "control": "master"
        },
        "encoder-1": {
          "kind": "encoder",
          "index": 1
        }
      },
      "device_action_permissions": []
    }
  ]
}
```

The keys in `control_bindings` must be controls declared by the manifest. The available canonical
intent shapes are:

- `{"kind":"programmer_key","key":"record"}` with keys `zero` through `nine`, `plus`,
  `minus`, `point`, `at`, `enter`, `clear`, `group`, `cue`, `record`, `delete`, `copy`, `move`,
  `set`, and `time`;
- `{"kind":"modifier","modifier":"shift"}`;
- `{"kind":"navigation","action":"up"}` with `up`, `down`, `left`, `right`, `page_up`,
  `page_down`, `menu`, or `escape`;
- `{"kind":"highlight","action":"toggle"}` with `toggle`, `previous`, `next`, or `all`;
- `{"kind":"encoder","index":1}`;
- `{"kind":"playback_current","slot":1,"control":"button_one"}`;
- `{"kind":"playback_explicit","page":2,"slot":1,"control":"master"}`;
- `{"kind":"speed_group","group":"A","control":"tap"}`;
- `{"kind":"grand_master"}` or `{"kind":"blackout"}`; and
- `{"kind":"desk_command","command":"stage"}` with `home`, `stage`, `fixtures`, `channels`,
  `groups`, `presets`, `cues`, `dynamics`, `playbacks`, `setup`, or `help`.

Playback controls are `button_one`, `button_two`, `button_three`, and `master`. Speed Group controls
are `tap`, `double`, `half`, and `level`. Current-page and explicit-page Playback bindings are
different contracts; never turn one into the other inside the extension.

## Process launch and private channel

ToskLight launches one process per enabled instance and clears its ambient environment. Do not
depend on `PATH`, a shell, the current working directory, user configuration directories, or other
inherited variables. ToskLight supplies:

- `TOSKLIGHT_EXTENSION_ID`;
- `TOSKLIGHT_EXTENSION_INSTANCE_ID`;
- `TOSKLIGHT_EXTENSION_PACKAGE_DIGEST`;
- `TOSKLIGHT_EXTENSION_CHANNEL_CREDENTIAL`;
- `TOSKLIGHT_EXTENSION_LAUNCH_ATTEMPT`; and
- `TOSKLIGHT_EXTENSION_DEVICE_IDENTITY` when the instance has a device binding.

Standard input receives host frames. Standard output must contain only extension frames. Write
bounded human-readable diagnostics to standard error. The host bounds queues and logs and may
restart a process after a protocol error, crash, or stall.

Every frame is four bytes of unsigned big-endian payload length followed by one UTF-8 JSON object:

```text
payload_length:u32-be | JSON payload
```

The maximum JSON payload is 1 MiB. Host-to-extension and extension-to-host directions each start
their own `sequence` at 0 and increment it by exactly one. There are no newlines or delimiters
between frames. Reads may fragment one frame or combine several, so use an incremental decoder.

```json
{
  "version": 1,
  "sequence": 0,
  "message": {
    "type": "host_hello",
    "body": {
      "host_name": "ToskLight",
      "host_instance_id": "HOST_INSTANCE_ID",
      "supported_versions": [1],
      "requested_capabilities": ["control_surface"],
      "channel_challenge": "FRESH_OPAQUE_CHALLENGE"
    }
  }
}
```

Message and enum names use lowercase `snake_case`. Message objects are tagged by `type` and put
their fields under `body`. New optional message fields may be added; ignore fields you do not use,
but reject an unknown required capability or unsupported protocol version.

## Authenticate and complete the handshake

The order is exact:

1. Read host sequence 0, a `host_hello` containing `host_name`, `host_instance_id`,
   `supported_versions`, `requested_capabilities`, and a fresh `channel_challenge`.
2. Compute `channel_response` as lowercase SHA-256 over: credential byte length as unsigned
   64-bit big-endian, credential UTF-8 bytes, challenge byte length as unsigned 64-bit big-endian,
   then challenge UTF-8 bytes.
3. Send extension sequence 0, an `extension_hello` whose identity, instance, package digest, and
   capabilities exactly match the launch environment and manifest.
4. Read host sequence 1, `configure`. It contains only the enabled capabilities and permissions,
   installation settings, control bindings, telemetry declarations, device actions, and—for a
   control surface—a complete authoritative feedback snapshot.
5. Apply the complete configuration and snapshot before reading hardware input. The extension may
   then send `health` with status `ready`; there is no separate `ready` message and the host does
   not wait for one to complete the protocol handshake.

Example extension hello:

```json
{
  "version": 1,
  "sequence": 0,
  "message": {
    "type": "extension_hello",
    "body": {
      "extension_id": "com.example.surface",
      "extension_instance_id": "surface-left",
      "extension_version": "1.0.0",
      "package_digest": "THE_APPROVED_PACKAGE_DIGEST",
      "selected_version": 1,
      "capabilities": ["control_surface"],
      "channel_response": "THE_COMPUTED_SHA256"
    }
  }
}
```

Never print the credential or response. A fresh challenge and credential are used for another
launch.

## Wire message reference

The table is exhaustive for protocol 1. “Host” means ToskLight and “extension” means the supervised
vendor process.

| Message `type` | Direction | Body |
| --- | --- | --- |
| `host_hello` | Host to extension | `host_name`, `host_instance_id`, `supported_versions`, `requested_capabilities`, `channel_challenge` |
| `extension_hello` | Extension to host | `extension_id`, `extension_instance_id`, `extension_version`, `package_digest`, `selected_version`, `capabilities`, `channel_response` |
| `configure` | Host to extension | `enabled_capabilities`, `feedback`, `telemetry_channels`, `device_actions`, `control_bindings`, `settings` |
| `control_input` | Extension to host | `input_id`, `occurred_at_micros`, `control` |
| `feedback_snapshot` | Host to extension | `context`, `revision`, `controls` |
| `feedback_delta` | Host to extension | `context`, `base_revision`, `revision`, `changes` |
| `telemetry_sample` | Extension to host | `sample_id`, `observed_at_micros`, `channel_id`, `value`, `quality` |
| `device_action_request` | Host to extension | `request_id`, `action_id`, `parameters` |
| `device_action_result` | Extension to host | `request_id`, `action_id`, `status`, `detail`, `values` |
| `timecode_sample` | Extension to host | `sample_id`, `observed_at_micros`, `hours`, `minutes`, `seconds`, `frames`, `rate`, `drop_frame` |
| `health` | Extension to host | `status`, `detail`, `counters` |
| `shutdown` | Either direction | `reason`, `detail` |
| `protocol_error` | Either direction | `code`, `detail`, `rejected_sequence` |

`feedback` in `configure` is null unless `control_surface` is enabled. When enabled it is a full
snapshot:

```json
{
  "context": {
    "desk_id": "DESK_UUID_OR_CANONICAL_ID",
    "show_id": null,
    "show_generation": 42
  },
  "revision": 7,
  "controls": {
    "go": {
      "kind": "control",
      "value": {
        "available": true,
        "enabled": true,
        "selected": false,
        "warning": false,
        "error": false,
        "lamp": "off",
        "semantic_color": null,
        "resolved_rgb": null,
        "value": null,
        "ring_style": null,
        "text": "Playback 1 ButtonOne"
      }
    }
  }
}
```

Each telemetry declaration in the manifest and `configure.telemetry_channels` has this exact
shape:

```json
{
  "channel_id": "temperature",
  "label": "Device temperature",
  "quantity": "temperature",
  "unit": "degC",
  "value_kind": "number",
  "minimum": -20.0,
  "maximum": 100.0,
  "precision": 1,
  "expected_interval_micros": 1000000,
  "quality_flags": ["good", "stale"]
}
```

For a non-numeric channel, set `minimum`, `maximum`, and `precision` to null. An empty
`quality_flags` accepts only `good`. A telemetry-source session must declare at least one channel.

Each permission-gated device action in the manifest and `configure.device_actions` has this shape:

```json
{
  "action_id": "identify",
  "label": "Identify device",
  "required_permission": "device.identify",
  "parameters": {
    "duration_ms": "integer"
  },
  "result_values": {
    "accepted": "boolean"
  }
}
```

The typed value encodings used by telemetry and device actions are
`{"kind":"number","value":1.5}`, `{"kind":"integer","value":2}`,
`{"kind":"boolean","value":true}`, and `{"kind":"text","value":"ok"}`.

Shutdown reasons are `host_requested`, `extension_requested`, `reconfigure`, and
`protocol_failure`. Protocol-error codes are `unsupported_version`, `invalid_handshake`,
`capability_not_negotiated`, `invalid_sequence`, `invalid_payload`, and `frame_too_large`. A
protocol error normally ends the session; do not try to continue a frame sequence after it.

## Send hardware input to ToskLight

`input_id` is one strictly increasing counter for the complete instance. `occurred_at_micros` is a
device/source timestamp in microseconds; use `0` when the device has no timestamp. It is not
treated as host time. A button must send one press and one matching release—no repeated press while
held and no release without a press. Absolute values are finite normalized values from `0.0`
through `1.0`; relative deltas are non-zero integers.

```json
{
  "version": 1,
  "sequence": 1,
  "message": {
    "type": "control_input",
    "body": {
      "input_id": 1,
      "occurred_at_micros": 123456,
      "control": {
        "kind": "button",
        "control_id": "go",
        "pressed": true
      }
    }
  }
}
```

The other control bodies are
`{"kind":"absolute","control_id":"master-1","value":0.75}` and
`{"kind":"relative","control_id":"encoder-1","delta":-1}`. The host rejects undeclared or
unbound controls and input shapes that do not fit the binding. For example, Grand Master and a
Playback master require absolute input, while a normal Playback button requires button input.

The host supplies desk and action-source identity. Do not include a desk, user, OSC path, HTTP
route, raw command string, or show object in an input event.

## Render feedback to the device

`configure.feedback` is the initial full `feedback_snapshot`. A later snapshot replaces all local
feedback state. A `feedback_delta` is valid only when its `base_revision` equals the last applied
revision; apply all changes in order and advance to its `revision`. A change with `value: null`
removes that control's feedback. After reconnect, sequence failure, or an inability to apply a
delta, discard local guesses and wait for the next complete snapshot.

A control feedback value has this shape:

```json
{
  "kind": "control",
  "value": {
    "available": true,
    "enabled": true,
    "selected": true,
    "warning": false,
    "error": false,
    "lamp": "on",
    "semantic_color": "playback-active",
    "resolved_rgb": [0, 170, 255],
    "value": 0.75,
    "ring_style": "dot",
    "text": "Playback 1"
  }
}
```

Lamp states are `off`, `dim`, `on`, `blink_slow`, and `blink_fast`. Ring styles are `dot`, `bar`,
`spread`, and `pan`. Feedback may also be a `boolean`, `level`, `text`, or `rgb` value. Use
`resolved_rgb` directly on RGB hardware. Map it deterministically to the nearest fixed palette
colour on indexed hardware. Monochrome hardware may map active colour to lamp state, but must not
invent Cue, Programmer, Playback, or warning state. Missing optional features mean “unsupported”,
not permission to infer them.

This host-to-extension feedback path is the normal **output** side of a controller extension. It
drives vendor lamps, motors, rings, and displays. It is not a lighting-output or arbitrary-command
channel.

## Implement telemetry, device actions, or timecode

A telemetry manifest declares every channel with ID, label, quantity, explicit unit or
`unitless`, value kind, optional inclusive bounds and precision, expected interval, and allowed
quality flags. `value_kind` is `number`, `integer`, `boolean`, or `text`; quality is `good`, `stale`,
or `invalid`. Each `telemetry_sample` has a per-channel strictly increasing `sample_id`, non-zero
`observed_at_micros`, declared `channel_id`, matching typed `value`, and allowed `quality`.

```json
{
  "version": 1,
  "sequence": 2,
  "message": {
    "type": "telemetry_sample",
    "body": {
      "sample_id": 7,
      "observed_at_micros": 123456,
      "channel_id": "temperature",
      "value": { "kind": "number", "value": 22.5 },
      "quality": "good"
    }
  }
}
```

A device action is declared in the manifest with `action_id`, label, `required_permission`, and
complete parameter/result maps. The operator must place that permission string in the instance's
`device_action_permissions`; otherwise the action is omitted from `configure`. Reply to every
received `device_action_request` with the same `request_id` and `action_id`, status `completed`,
`rejected`, or `failed`, optional detail, and exactly the declared typed result values. Never
execute an undeclared action.

```json
{
  "version": 1,
  "sequence": 3,
  "message": {
    "type": "device_action_result",
    "body": {
      "request_id": 12,
      "action_id": "identify",
      "status": "completed",
      "detail": null,
      "values": {
        "accepted": { "kind": "boolean", "value": true }
      }
    }
  }
}
```

A `timecode_sample` contains a strictly increasing `sample_id`, non-zero timestamp, hours 0–23,
minutes and seconds 0–59, a valid frame number, rate `fps24`, `fps25`, `fps2997`, or `fps30`, and
`drop_frame`. Only `fps2997` uses `drop_frame: true`; all other rates use false. The host, not the
extension, owns source priority, loss, fallback, and show behavior.

## Health, shutdown, and recovery

An extension may send `health` with status `starting`, `ready`, `degraded`, or `failed`, optional
detail, and bounded numeric counters. This is diagnostic information, not desk authority.

On host `shutdown`, stop accepting hardware input, put the vendor device into its documented safe
disconnected state, release it, and exit promptly. ToskLight terminates and reaps a process that
misses the deadline. An extension may request shutdown with its own `shutdown` message, but the
host treats that as a failed session and may apply restart policy.

Malformed or oversized JSON, wrong frame sequence, wrong handshake identity or digest, an
unnegotiated message capability, stalled pipes, invalid control lifecycle, or a full inbound queue
faults only that child. ToskLight uses bounded exponential restart backoff and stops after repeated
failure. Each restart begins at sequence 0 with a new credential, challenge, configuration, and
full feedback snapshot. The extension must be able to reconstruct all state from those values.

## Conformance and vendor acceptance

The normative Rust transfer types and codec are in `crates/light/contracts/extensions`. The host
implementation is in `crates/light/adapters/extensions`. The portable executable example is
`crates/light/adapters/extensions/tests/support/synthetic_extension.rs`; it is the clearest
reference for framing, authentication, input, telemetry, health, device-action replies, and
shutdown.

From the ToskLight repository root, run:

```sh
cargo test -p light-extensions-contract --locked
cargo test -p light-extensions-host --test supervised_child --locked
cargo clippy -p light-extensions-contract -p light-extensions-host --all-targets --locked -- -D warnings
```

Before delivery, the vendor repository should also prove:

- partial and combined frame reads, exact sequence handling, and the 1 MiB limit;
- authentication failure, configuration validation, and no input before configuration;
- unplug/reconnect, path changes with the same stable identity, and ambiguous-device refusal;
- every physical button edge, fader/encoder range, debounce rule, and maximum input rate;
- complete snapshot replacement, delta ordering, lamps/blinks/RGB, motor pickup, rings, displays,
  and deterministic feature degradation on the real device;
- bounded logging and queues, device I/O stalls, crash/restart, graceful shutdown, and forced
  termination; and
- every supported OS/architecture with the real vendor driver and hardware.

Host conformance tests prove the ToskLight boundary, not the vendor protocol or electrical
hardware. Record the extension version, package digest, ToskLight version, operating system,
device firmware, and stable device identity in physical acceptance evidence.
