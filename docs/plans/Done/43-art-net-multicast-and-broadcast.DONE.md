# Art-Net Multicast and Broadcast Delivery

## Status and scope

Resolve the “ArtNET Multicast/Broadcast” reminder by making network-output delivery mode explicit and protocol-correct. Art-Net DMX output supports **Broadcast** or **Unicast** routes; sACN supports **Multicast** or **Unicast** routes. Do not label an Art-Net broadcast destination as multicast.

## Route configuration

The route editor presents a delivery-mode choice appropriate to the selected protocol:

- **Art-Net Broadcast** uses a documented broadcast destination. Planning must choose global `255.255.255.255` versus an interface-directed subnet broadcast and expose interface selection if needed for reliable multi-NIC desks.
- **Art-Net Unicast** requires a destination IP and port.
- **sACN Multicast** derives the standards-defined multicast destination from the destination universe and does not ask for an IP.
- **sACN Unicast** requires a destination IP and port.

Persist delivery mode explicitly in new route data. Migrate a legacy route with an explicit destination to Unicast. A legacy Art-Net route without a destination retains Broadcast behavior, and a legacy sACN route without a destination retains Multicast behavior. Disabled routes remain configured but emit nothing.

The UI must validate broadcast capability, address family, port, destination universe, interface availability, and stale revisions before Save. Runtime diagnostics must show the actual resolved socket destination and delivery mode.

## Acceptance criteria

1. The editor shows only protocol-valid delivery modes and only requests a destination for Unicast.
2. New and migrated routes preserve delivery mode through Save/Reload, show switching, and restart.
3. Packet-capture tests verify actual Art-Net broadcast/unicast and sACN multicast/unicast destinations and payload equality.
4. Multi-interface and unavailable-interface behavior is deterministic and produces actionable errors.
5. Disable, re-enable, edit, and remove preserve existing output-ownership and stream-termination semantics.
6. Help uses **Art-Net**, **Broadcast**, **Unicast**, **sACN Multicast**, and **sACN Unicast** consistently.
