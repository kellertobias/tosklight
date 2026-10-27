# Discover and patch Pixel outputs

## Patched Media Servers

**Show Patch > Media Servers** opens with a table of every patched Media Server, one row each, ordered by fixture number. A server stays in the table while its network control is off; only its connection is paused.

| Column | Meaning |
| --- | --- |
| **Type** | **ToskLight Media** for a ToskLight Pixel Media Server, **CITP media server** for any other CITP-capable media server, or **No network control** when the profile offers none. |
| **Protocol** | **CITP** connects the desk to the server; **Off** stops the desk from talking to it. Only protocols the fixture profile supports are offered. |
| **IP address** and **CITP port** | Where the server listens for CITP. The address must be a literal IPv4 or IPv6 address, and the port must be 1–65535; the row explains a wrong value and keeps **Apply** disabled until it is fixed. |
| **HTTP port** | ToskLight Media only: the native-control and configuration endpoint on the same IP address, normally `8080`. It is independent of CITP; enter a custom port explicitly. |
| **Status** | The desk's own connection state: **Connected**, **Offline**, **Not checked**, **Checking…**, or **Off**. Below it, whether the last discovery **Found** the address on the network. An offline row shows the server's reason and what to check. |
| **Actions** | **Apply** stores the row's protocol and address. **Check connection** asks that server once whether it answers and updates **Status**. **Refresh Thumbnails** reconnects and fetches that server's media thumbnails. **Start live preview** shows the server's output under the row. |

The status follows the desk as it changes: when a connection attempt succeeds or fails, every open Show Patch updates without refreshing. When you apply a new address, the row checks the server once at that address. Each row works on its own; refreshing or applying one server never disables another row or clears its message.

**⚙ > Media Servers > Clear Thumbnail Cache** drops every Media Server thumbnail the desk has cached, for example after replacing media on a server. Live previews and the patch stay as they are; use **Refresh Thumbnails** on a row to fetch its thumbnails again.

## Discovering a ToskLight Pixel Media Server

Open **Show Patch > Media Servers** to discover ToskLight Pixel Media servers on the local network. **Refresh Discovery**, in its own group at the top right of the window beside the view tabs, repeats the search without restarting either application. Each output is shown by its Media Server and output name. Below the name are the server's IP address, its type (**ToskLight Media**), its CITP port, whether it is **Online**, and the desk's connection to it (**Connected**, **Offline**, **Not checked**, or **Not connected (not patched)**). Next comes the output's current configuration:

- the suggested DMX `universe.address`
- the **2 layers** or **8 layers** personality
- the protocol and universe it listens to (for example **listens to sACN 101**)
- whether its tempo follows the Playback BPM channel or a desk Speed Group

Configured ToskLight Media endpoints are checked on their stored HTTP ports during **Refresh Discovery**. Unconfigured discovery searches standard CITP `4809` and HTTP `8080`, not custom ports. Configure a custom endpoint in the patched row first. CITP does not advertise or determine its HTTP port.

The suggested desk universe is the one whose output route (**Setup › Outputs**) sends what the server listens to. When no desk route sends it, the card says so. A discovered output remains explicitly **Not patched** until you choose an action; discovery alone never changes the show.

The status beside each output tells you what to do next:

- **Patched**: a desk fixture controls this output with the matching personality, at the server's address, and a desk output route delivers that universe to the server.
- **Mode differs**: the desk fixture uses the other personality. Every layer after the first would be misaddressed, so the card explains the difference, and **Patch suggested** switches the desk fixture to the output's personality.
- **Endpoint differs**: the desk fixture controls the server at another IP address or CITP port than discovery found. **Patch suggested** updates the desk endpoint.
- **Address differs**: the desk sends the fixture's universe or address somewhere other than where the output listens. The card names both sides. **Patch suggested** moves the desk patch to the server; **Patch address** moves both.
- **Not received**: no desk output route sends the fixture's universe onto the network, or the fixture is unpatched, so the server receives nothing. Add an output route under **Setup › Outputs**, or choose **Patch address**.
- **Needs update**: the Media Server reports a personality the desk cannot patch, or is too old to describe its outputs in the current format (for example, it still offers the retired channel layouts). The patch actions stay disabled. Update ToskLight Media, or choose 2 or 8 layers under its **Settings > Network & DMX**, then **Refresh Discovery**.
- **Unavailable**: the server answered discovery but not its configuration API. Check that it is running and reachable on its configured HTTP port (default `8080`), then refresh.

**Patch suggested** creates the matching ToskLight Pixel fixture, or updates the existing one, at the desk universe and address that reach the output. It also sets the fixture's CITP endpoint to the discovered server. Patch actions preserve the stored HTTP port when IP and CITP still match. Resolve conflicting output bindings or endpoints before patching. It never changes the Media Server. The normal Patch footprint and collision checks run before the desk accepts it; a refusal is shown on the card and says that the Media Server was not changed.

**Patch address** lets you choose another desk universe and address. Before anything changes, the desk looks up how it sends that universe. When no enabled network output route sends it, nothing changes and the card asks you to add the route. Otherwise:

1. The desk patch is validated and stored.
2. The selected Media Server output moves to that address. Its DMX input also moves to the route's protocol and universe, for example **Art-Net 1** when desk universe 1 is sent as Art-Net universe 1. When a universe is sent both ways, the server keeps the protocol it already uses.
3. The card confirms both sides, or says that the server listens there only after its next restart.

If the Media Server answers with a different address than requested, the card says so rather than reporting success. If the remote update fails, the desk restores its previous patch (or removes the newly created fixture) and asks you to **Refresh Discovery** to confirm the server's address. If restoring also fails, the card says that the desk and the server may differ. It never silently reports mismatched addresses as patched. When the Media Server refuses the change, the message says why and what to do: a value it rejected (such as an 8-layer block that no longer fits the universe) names that value, a change it could not save asks you to check free space and write access for its configuration folder, and an output it no longer has asks you to refresh discovery.

Native Media controls (effects and text) act on the output the fixture was patched to. If that output has since been removed from the Media Server, the desk reports it instead of controlling a different screen; refresh discovery and patch the output again.

The Media Server may report that its DMX input change needs a restart; Show Patch keeps that state visible. An unreachable server or discovery failure does not disable the ordinary fixture-library and manual Media Server patch workflows.

Pixel also publishes GDTF and desk-native personality downloads in its management interface under
**DMX > Connect to Console**. Use those files when patching Pixel from MagicQ,
grandMA2, grandMA3, or another GDTF-compatible desk. The Layer and Master personalities come from
the same canonical channel layout as Pixel's DMX receiver; patch every Layer consecutively and one
Master immediately after them.
