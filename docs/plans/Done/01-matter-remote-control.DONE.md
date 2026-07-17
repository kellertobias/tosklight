# Matter Remote Control

## Status

**Implementation status: Complete.** The desk-persistent enable setting, stable global page/playback endpoint topology, faderless playback support, authoritative OnOff/Level writes, bidirectional runtime feedback, persisted fabric/node identity, commissioning window, UDP/mDNS transport, UI, help, and focused/paired automated coverage are implemented.

Production distribution still requires replacing rs-matter's development VID/PID and device-attestation credentials with the product's CSA credentials and completing the corresponding certification process. Commissioning and subscription behavior with an independent certified controller on an unrestricted host remains an external release/interoperability gate; it is not simulated by the browser acceptance suite.

Add the ability to enable acting as a "Matter Device" or "Matter Bridge". This needs to be configurable in the Desk Settings (not show settings, this is a desk persistent setting accross multiple shows). If this setting is enabled, this feature is enabled. If the setting is disabled, this feature is disabled.

When the feature is enabled, we expose every playback (desk-independent, that is we use the global playback numbers: page/playback, not depending on whatever page is currently active on the given desk) as an individual Matter light. The Matter light's dimmer value controls the corresponding playback fader, allowing standard Matter remotes and controllers to operate playbacks.

We need bidirectional value updates (that is because our tracking might turn off an active playback and this must turn off the given matter light). If a given playback is empty, we do not expose a matter device for it.
