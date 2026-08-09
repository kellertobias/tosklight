# DMX Output and Universe Routes

ToskLight renders logical universes and sends them through configured Art-Net, sACN, or built-in USB DMX routes. DMX input is not provided by these output transports.

## Configure the engine

In **Desk Setup > Outputs**, choose a 40-60 Hz frame rate, the output bind address, and backup retention. Backup retention limits how many automatic recovery checkpoints are kept; configure the checkpoint interval under **Shows & recovery**. Bind to the interface used by the isolated lighting network. Save and restart when requested.

![Output engine and universe-route configuration](../assets/screenshots/workflows/desk-setup-output-engine.png)

## Create routes

Open **Desk Setup > Outputs > Routes**. A route maps one logical show universe to an Art-Net or sACN destination universe, or to one claimed local USB DMX endpoint. Create, edit, enable, disable, and verify routes beside the output-engine configuration rather than inside the DMX monitor.

Choose **Add route** to create one, or **Edit route** beside a versioned route to change its protocol, logical universe, destination universe, delivery mode, address, minimum universe size, or enabled state. **Art-Net Broadcast** uses the global `255.255.255.255:6454` destination. **Art-Net Unicast** requires an IPv4 address and port. **sACN Multicast** derives `239.255.x.y:5568` from the destination universe, while **sACN Unicast** requires an IPv4 address and port. Art-Net does not offer Multicast, and sACN does not offer Broadcast.

The output bind address selects the lighting-network interface. Use a specific IPv4 address on a multi-interface desk when the operating system must not choose the egress interface. An unavailable address prevents the output engine from starting with an actionable bind error; `0.0.0.0` deliberately leaves interface selection to the operating system. Runtime diagnostics report the configured bind address plus every route's resolved delivery mode and socket destination.

New routes start with a minimum of 128 slots. Every enabled route emits a frame on every output tick even when its logical universe has no patch; that idle payload contains zeros and is at least the configured minimum size. A patched fixture extends the payload through its complete footprint, and every patched channel contains its fixture default or zero when no default is configured.

## Configure USB DMX

Connect the interface, then use **Desk Setup > Outputs > USB DMX endpoints > Scan USB devices**. Choose the exact discovered USB serial device, give it a stable Endpoint ID, choose **ENTTEC USB Pro v1.44** or **Open DMX (FTDI)**, and select **Claim endpoint**. ToskLight stores the strong USB serial identity when available; otherwise it binds the current physical port identity and will not silently fall back to the first similar device.

After the endpoint reports Ready, add a route, choose **USB DMX endpoint**, select its Endpoint ID, and assign one logical universe. Endpoint identity and health belong to this desk installation; the show route stores only the portable Endpoint ID. A show can therefore move to another desk whose operator has deliberately claimed a matching local Endpoint ID.

Built-in support is intentionally limited to host-timed FTDI Open DMX interfaces and the documented ENTTEC DMX USB Pro v1.44 framing family. Open DMX continuously generates BREAK, mark-after-break, and the complete frame at 40 Hz. USB Pro interfaces retain their last buffered frame when output stops; Open DMX shutdown cannot promise a final electrical state. Disconnecting or failing one USB endpoint does not block Art-Net, sACN, or another endpoint. Diagnostics show Online/Reconnecting state, accepted frames, reconnect attempts, the last error, dropped frames, and the final shutdown outcome.

Do not select a driver based only on a similar product name. Unsupported, ambiguous, or duplicate device identities remain Offline with an actionable diagnostic. Electrical BREAK/MAB timing, isolation, and behavior after unplug still require validation with the actual interface before a production show.

For backward compatibility, a historical route with an explicit destination migrates to Unicast. A historical Art-Net route without a destination migrates to Broadcast; a historical sACN route without a destination migrates to Multicast. The explicit mode then survives save/reload, show switching, and restart. Disabling keeps the mapping in the show but stops its output, so another independent desk can own that destination universe. Art-Net stops without a final black frame; sACN sends its required stream-termination burst to the route's resolved Multicast or Unicast destination. Removing a route requires explicit confirmation.

## Verify output

The Universe view shows the value for every DMX slot and identifies the patched fixture channel. Select a channel to see its fixture, attribute, DIP-switch address, and raw value. Diagnostic overrides write raw output outside normal programming; release every override after testing.

Before a show, confirm frame rate, packets sent, send errors, bind interface, route enablement, delivery mode, resolved socket destination, universe mapping, and representative fixture movement. Output is not proved merely because the programmer shows a value.
