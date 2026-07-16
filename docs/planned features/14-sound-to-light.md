# Sound to Light

Add sound-reactive control by allowing a Speed Group to derive its speed from live audio. The existing Speed Group control should open a configuration menu where the operator can enable sound input and configure how the selected Speed Group responds.

## Audio source and analysis

The configuration must allow the operator to select an available audio input, such as a microphone or line input. It should show whether the source is available and receiving a usable signal, and provide enough live feedback to configure the response without guessing.

The operator must be able to choose which part of the sound is analyzed. This should support useful frequency regions or configurable frequency ranges so, for example, a Speed Group can follow a kick drum in the low frequencies rather than vocals or high-frequency content. Planning should define whether the analysis derives tempo/BPM, detects individual beats or transients, follows signal level, or offers these as separate modes.

Relevant controls may include input gain, threshold, sensitivity, smoothing, noise gating, minimum and maximum accepted tempo, and a hold or fallback behavior when no reliable sound is detected. The UI should expose only controls that have clear operator meaning and provide an understandable visualization of the incoming level, selected frequency content, and detected beat or tempo.

## Speed Group mapping

The detected sound result drives the selected Speed Group through the same authoritative Speed Group state used by Cuelist Chasers, Speed Master playbacks, and command-line speed controls. The operator can apply a multiplier or divider so the group runs faster or slower than the detected sound—for example `0.5x`, `1x`, `2x`, or another supported ratio.

Planning must define how manual Learn, Double, Half, Pause, Speed Master faders, and sound-derived updates interact; whether sound control temporarily takes ownership or continuously updates the learned rate; and how the desk avoids abrupt or unstable tempo changes. Disabling Sound to Light should return the Speed Group to a predictable manual state rather than silently losing the last useful rate.

## Scope, persistence, and safety

Sound-to-Light configuration should be saved with the appropriate Speed Group or show configuration, while machine-specific audio-device selection may require a desk-local mapping so a show remains portable to hardware with different device names. Missing or disconnected audio inputs must produce visible status and a deterministic fallback without blocking other Speed Groups.

Before implementation, define microphone permissions, multi-desk and remote-browser ownership of audio capture, server-side versus client-side analysis, latency, reconnect behavior, calibration, and how recorded or test audio can be used for deterministic verification. No sound-derived value should bypass the existing Grand Master, Blackout, playback, or output safety rules.
