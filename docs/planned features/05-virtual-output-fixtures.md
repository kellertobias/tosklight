# Virtual Output Fixtures and TOSC Studio Integration

Explore virtual output fixtures that let the light desk control systems beyond conventional lighting output. These fixtures would present external video and GPIO functions through the desk's existing fixture, programmer, cue, and playback workflows while translating their attributes into the appropriate external control protocol.

Potential virtual output fixture families include:

- OBS Studio control over its WebSocket API, with fixture attributes representing useful OBS actions and state such as scenes, sources, transitions, streaming, and recording.
- ATEM video switcher control, with fixture attributes representing switcher functions such as program and preview selection, transitions, keys, and other useful controls supported by the target ATEM model.
- GPIO control for wired GPIO hardware and for remote wireless GPIO nodes. The wireless design could use LoRa (Long Range) links so GPIOs can be placed away from the desk or network infrastructure.

This could allow major parts of [TOSK Studio](https://github.com/kellertobias/tosk-studio) to be embedded into the light desk and may eventually make the separate TOSC Studio application unnecessary for supported workflows. Before implementation, review TOSC Studio's behavior and decide which capabilities fit naturally into the desk, which should remain separate, and whether shared components or protocol adapters can be reused.

Planning should define the virtual fixture model, capability discovery, attribute mapping, bidirectional state feedback, connection and authentication settings, failure and reconnect behavior, and how device-specific differences are represented. It should also determine whether these integrations are desk-persistent outputs, show-persisted fixtures, or a combination of both, and how missing or offline external devices behave during programming and playback.
