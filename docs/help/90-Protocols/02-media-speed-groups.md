# Media Server Speed Groups

ToskLight Control is the authority for Speed Group tempo. A ToskLight Media Server only
**receives** Speed Groups; it never publishes them. Synchronized play modes on an output that
follows a Speed Group retime to the tempo the desk sends.

Use this interface only on a trusted lighting network.

## Why OSC

No widely implemented lighting protocol carries several independent, numbered tempo groups:

| Protocol | What it carries | Why it does not fit |
| --- | --- | --- |
| Art-Net, sACN | DMX levels | An 8-bit channel is too coarse for BPM and carries no phase, group identity, or sender order. |
| Art-Net TimeCode, MTC, SMPTE LTC | Time position | Position, not tempo. |
| MIDI Beat Clock | One tempo, 24 pulses per beat | One tempo per port, jittery, and it needs a MIDI transport. |
| Ableton Link | One shared session tempo and phase | One tempo per network, not five groups, and an external SDK. |
| CITP/MSEX | Media libraries, thumbnails, previews | Has no tempo message. |
| Pioneer Pro DJ Link | DJ deck tempo | Proprietary, and one master tempo. |

OSC is widely implemented, but it has no standard tempo namespace. Speed Groups therefore use
the ToskLight message below, over OSC 1.0 on UDP.

## Configure

1. On the Media Server, open **Settings > Network & DMX** and enter a **Speed Groups** listen address.
   The usual value is `0.0.0.0:4810`. Leave it empty to not receive Speed Groups. The listener
   starts the next time the Media Server starts.
2. In the same tab's **DMX input** section, set **Synchronized playback follows** for each output to a **Light
   desk Speed Group** from A to E. This applies immediately. **Each layer's Playback BPM
   channel** keeps the per-layer DMX tempo instead. An output follows exactly one of the two
   sources, never both.
3. In ToskLight Control, patch the ToskLight **Media Server** fixture and set its CITP endpoint.
   The desk sends every Speed Group ten times a second to UDP port `4810` on that endpoint's IP
   address, for as long as a show is open.

**Settings > Network & DMX** shows reception live:

- whether the server is off, listening, receiving, or has lost the desk;
- which desk it follows, from which address, and when it last heard from it;
- each group's BPM and whether it is running, paused, or stale;
- how many messages were accepted and refused, with the reason for the latest refusals.

## Message

| Address | Type tags | Arguments |
| --- | --- | --- |
| `/tosklight/speed-group` | `,siiffi` | source, sequence, group, BPM, beat phase, running |

| Argument | Type | Meaning |
| --- | --- | --- |
| source | string | The sender's identity for this run. A restarted desk uses a new one. |
| sequence | int32 | At least `0`, and increasing with every message from this source. |
| group | int32 | Speed Group number from `1` to `64`. ToskLight Control sends A–E as `1`–`5`. |
| BPM | float32 | The group's effective tempo, from `0` to `999`. |
| beat phase | float32 | Position within the current beat, from `0` up to but not including `1`. |
| running | int32 | `1` while the group runs, `0` while it is paused. |

A receiver also accepts float64 (`d`) for BPM and beat phase, and `T`/`F` for running. An OSC
bundle may carry any number of these messages; its time tag is ignored and each message applies
on arrival.

Reference datagram: source `desk`, sequence `1`, group `1`, 120 BPM, beat phase `0.5`, running:

```text
2f746f736b6c696768742f73706565642d67726f757000002c736969666669006465736b00000000000000010000000142f000003f00000000000001
```

## Receiver rules

- **One sender.** The first source heard is followed. While it is live, a message from another
  source is refused as a competing sender.
- **Order.** From the followed source, a message whose sequence is not higher than the last one
  applied is refused, so a late datagram never steps the tempo backwards.
- **Freshness.** A group is live for 1.5 seconds after its last message. After that the output
  keeps the last tempo, reports the group as stale and the desk as lost, and does not fall back
  to the Playback BPM channel.
- **Reconnect.** Once the followed source has been silent for 1.5 seconds, the next valid message
  is accepted whatever its sequence or source. A different source replaces all groups.
- **Pause.** A paused group holds synchronized playback on its current frame. Resuming continues
  from that frame.
- **Continuity.** A tempo change continues from the frame on screen. The same sequence of tempos
  produces the same frames whatever the output's frame rate.
- **Invalid data.** A message with another address, wrong type tags, a truncated
  packet, a group outside `1`–`64`, a BPM outside `0`–`999`, a non-finite value, a beat phase
  outside `0`–`1`, or an empty source is refused and shown to the operator. It never changes a
  tempo.
