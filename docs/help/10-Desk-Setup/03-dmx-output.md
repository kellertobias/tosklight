# DMX Output and Universe Routes

ToskLight renders logical universes and sends them through configured Art-Net or sACN routes. USB DMX and DMX input are extension points, not current output choices.

## Configure the engine

In **Desk Setup > Outputs**, choose a 40-44 Hz frame rate, the output bind address, and backup retention. Bind to the interface used by the isolated lighting network. Save and restart when requested.

![Output engine and universe-route configuration](../assets/screenshots/workflows/desk-setup-output-engine.png)

## Create routes

Open **Desk Setup > Outputs > Routes**. A route maps one logical show universe to an Art-Net or sACN destination universe and optional destination address; multicast is used when there is no explicit destination. Create, edit, enable, disable, and verify routes beside the output-engine configuration rather than inside the DMX monitor.

Choose **Add route** to create one, or **Edit route** beside a versioned route to change its protocol, logical universe, destination universe, address, or enabled state. New Art-Net routes require an address and port. For backward compatibility, a migrated historical Art-Net route whose destination was absent uses the standard `255.255.255.255:6454` broadcast; sACN with no destination uses multicast. Removing a route requires explicit confirmation and immediately terminates that route before the output snapshot is refreshed.

## Verify output

The Universe view shows the value for every DMX slot and identifies the patched fixture channel. Select a channel to see its fixture, attribute, DIP-switch address, and raw value. Diagnostic overrides write raw output outside normal programming; release every override after testing.

Before a show, confirm frame rate, packets sent, send errors, bind interface, route enablement, universe mapping, and representative fixture movement. Output is not proved merely because the programmer shows a value.
