# Sound to Light

These scenarios specify the Sound-to-Light input available from the existing Speed Group A–E controls. Sound analysis drives the same authoritative Speed Group rate consumed by Chasers, command-line speed controls, and Speed Master playbacks; it is not a separate browser-only speed.

The initial implementation supports one analysis mode: **Tempo / BPM**. The browser assigned to an input performs Web Audio capture, frequency-band metering, and transient/tempo analysis every 100 ms. The server owns configuration, validates observations, smooths accepted BPM, selects the authoritative manual/sound/held/fallback source, and applies the Sound multiplier followed by the Speed Master scale. Individual-beat triggering and level-following are deliberately outside this scenario.

## Persistence and capture ownership

- Frequency selection, gain, confidence threshold, smoothing, accepted BPM range, signal hold, multiplier, and enabled state are persisted authoritative Speed Group configuration. They contain no machine-specific audio device ID.
- The selected audio device ID is browser-local and keyed by desk plus Speed Group. Loading the same show on another machine therefore does not retain an unusable device identifier.
- Selecting an input requests browser/system microphone permission. An unassigned browser never captures audio. A browser with a stored assignment reopens it when the Speed Group is enabled; a denied, removed, or renamed source remains visibly unavailable and produces deterministic hold/fallback behavior.
- Sessions on the same desk may submit observations for their shared physical desk. A different desk cannot take over a Speed Group while another desk's short capture lease is active. The server's application clock, not a browser wall clock, determines lease and signal-hold expiry.
- Sound never bypasses Grand Master, Blackout, playback arbitration, or the normal output renderer. This scenario proves Speed Group source selection; the existing output scenarios remain the safety oracle for those later stages.

## SOUND-001 — A desk-local recorded input drives an authoritative Speed Group

**Priority:** P1  
**Primary layer:** Paired API/UI E2E with deterministic Web Audio input

**Starting show:** Load canonical `compact-rig.show`, immediately Save As a separate `sound-001-<surface>.show` working copy, and begin with Sound-to-Light disabled on Speed Group A.

**Detailed procedure:**

1. Switch the lower control section from Programmer to Playbacks, then open **Speed Group A** from Playback Tools.
   - **Expect:** The **Speed Group A · Sound to Light** modal opens. Opening the control does not perform a Learn tap or change the authoritative speed.
2. Install the test bench's deterministic recorded kick input, which produces stable transients at 120 BPM. Choose **Recorded kick track** for this browser.
   - **Expect:** Microphone permission is **Granted**, the source is **Capturing**, the selected-band signal becomes **Usable**, and live input/band meters update.
   - **Expect:** Merely previewing a selected source does not enable Sound-to-Light or publish authoritative observations before Apply.
3. Enable Sound-to-Light, choose **Custom range**, set 45–140 Hz, +6 dB gain, confidence 0.55, smoothing 0, accepted range 60–180 BPM, a 3.5-second signal hold, and a 2× Sound ratio. Apply.
   - **Expect:** The persisted configuration exactly matches those values and contains neither `device_id` nor `deviceId`.
4. Wait on server evidence rather than a fixed delay.
   - **Expect:** The authoritative source becomes `sound`, the accepted Sound BPM is between 115 and 125, and the effective Speed Group rate is between 230 and 250 BPM.
5. Reopen Speed Group A.
   - **Expect:** The modal shows **Capturing**, **Usable**, and a live **Sound · … BPM** authoritative source. The browser-local key `light.sound-to-light.device.<desk-id>.A` contains only the test input's local device ID.
6. Run the independent API variant with the same response configuration and a normalized 120 BPM observation.
   - **Expect:** The API and UI variants pass the same normalized state oracle.

**Assertions:** The recorded input reaches the browser analyzer, the browser publishes only after the saved enabled state is authoritative, the server selects and maps the detected tempo, and the device mapping remains separate from persisted response configuration.

**Pass condition:** A reproducible 120 BPM low-frequency source drives Speed Group A at about 240 BPM through one authoritative server state while its machine-specific input assignment remains local to the selected browser and desk.

## Manual controls, loss, and reconnect rules

- **Learn** is manual tap tempo. Its first tap immediately disables Sound-to-Light; later valid taps update the manual fallback from the rolling tap intervals.
- **Double** and **Half** change the Sound multiplier while Sound-to-Light is enabled. Outside Sound mode, they change the manual learned rate.
- **Pause** freezes phase advancement without discarding the current Sound rate. Resume continues from the retained rate.
- When a previously valid source becomes unusable, the group holds the last accepted Sound rate for the configured hold interval and then returns to its stored manual BPM. Low confidence and out-of-range tempos cannot take ownership.
- Direct command/API BPM entry disables Sound-to-Light and returns the group to manual ownership. Disabling Sound explicitly also restores the stored manual fallback.

## Failure follow-ups

| Case | Expected behavior | Diagnose first |
| --- | --- | --- |
| Permission denied | Visible **Permission denied** state and manual fallback; other groups remain usable. | Browser/system permission and `getUserMedia` error classification. |
| Selected device disconnected | Visible **Input unavailable**, held Sound rate for the configured interval, then manual fallback. | Stored desk/browser mapping and track-ended/device-change handling. |
| Quiet or wrong frequency band | Connected source remains visible, selected band reports quiet, and no low-confidence tempo takes ownership. | Frequency bins, input gain, band level, and confidence threshold. |
| Second browser on the same desk | Observations may continue as the same desk capture surface. | Desk identity and nondecreasing observation timestamps. |
| Different desk submits concurrently | The active capture lease rejects the other desk until expiry. | Server-owned desk capture lease and application clock. |
| Reconnect with saved assignment | The same browser reopens its local source when enabled; another browser remains unassigned. | Local-storage desk/group key and device enumeration. |
