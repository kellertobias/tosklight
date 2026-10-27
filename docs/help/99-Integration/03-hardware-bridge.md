# Hardware bridge API

A native extension connects a physical controller or sensor to ToskLight. The host supervises an executable package and exchanges typed messages through a private framed channel. An OSC surface uses the separate [OSC interface](02-osc.md).

An extension declares its controls and capabilities. Installation bindings map those controls to desk actions, including programmer keys, encoders, navigation, and playbacks. The host supplies authoritative feedback and owns the desk state. A reconnect starts from a complete feedback snapshot.

Packages must match the installation's approval and device identity before they run. Ordinary device connection and health checks are covered in [Native extensions](../20-ToskLight-Control/10-Show-Setup/08-native-extensions.md).

The [native-extension SDK](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/native-extension-sdk.md) defines manifests, framing, handshake, controls, feedback, telemetry, timecode, and lifecycle. It also states the protocol's draft status and provides conformance commands and simulator examples. Built-in USB DMX output is separate from this hardware-input API.
