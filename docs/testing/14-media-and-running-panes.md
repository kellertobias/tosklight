# Media and Running Panes

These scenarios are the operator acceptance contract for the capability-gated Media operating surface and the authoritative Running overview.

## MEDIA-001 — eligibility and unavailable state

Given a show without a patched physical media-server master, configured CITP/MSEX connection, and logical layer heads, **Media** is absent from Built-ins and **Open Window**. When an eligible fixture exists, both choices appear. A previously saved Media pane remains in the Desktop when its server disconnects, its patch disappears, or a required capability becomes unavailable; it explains that state and does not silently select another server or layer.

## MEDIA-002 — advertised preview and selection identities

Given an eligible physical master whose advertised composite source, logical layer, fixture head, and CITP source IDs differ, the Media window shows the advertised Program output and each advertised layer status without substituting any of those IDs. Loading, stale, failed, and unsupported feedback remains outside the actual preview. Touching one layer replaces the authoritative desk selection with that exact logical head, and an external selection change is reflected without a Media-only selection split.

## MEDIA-003 — atomic touch browsing

Given live Folder A / File X, touching Folder B changes only the draft browser and fetches B's files and thumbnails. Programmer values, DMX, and Undo history remain unchanged. Touching File Y commits Folder B / File Y through one grouped Programmer mutation and one Undo step, with no observable Folder B / File X state. Cancelling, switching server or layer, disconnecting, losing the library revision, or receiving a rejected write leaves the live pair unchanged.

## MEDIA-004 — capability-derived controls and persistence

Folder/File and Mask Folder/Mask File encoders remain immediate. The touch browser presents a discoverable **Media / Mask** choice only when masks are advertised. Secondary controls follow fixture and connection capabilities, and a native ToskLight Media Server action appears only behind its advertised capability. Restarting the desktop, changing show, disconnecting, and reconnecting preserve the pane's stable server, layer, browser, section, and secondary-region configuration without changing portable show data.

## RUNNING-001 — containment, deduplication, and identity

Start one Cuelist through several assignments or control surfaces, with a Dynamic contained in it; start one independent Dynamic, Timecode, and Macro. Running shows exactly four rows. The Cuelist row uses the Cuelist's own number and name plus its current Cue, not an assignment number, and suppresses the contained Dynamic. The other rows use their own stable identities and show **Cue —**.

## RUNNING-002 — filters and live reconciliation

All is the default. A pane's **Running kind** setting persists All, Cuelists, Dynamics, Timecodes, or Macros independently from other Running panes; the built-in exposes the same choices in its header. Start, Cue change, pause, resume, completion, release, cancellation, and stop update or remove rows without reopening. Every empty filtered view names the selected kind.

## RUNNING-003 — exact Off convergence

For each row kind, press **Off** and verify only the named runtime is released, stopped, or cancelled. Repeated presses while the request is pending submit one action. The same final runtime state and row removal result when the transition originates from software, a Virtual Playback, attached hardware, keyboard, OSC, WebSocket, or HTTP.
