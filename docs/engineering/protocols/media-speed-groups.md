# Media Server Speed Groups

Make Pixel’s synchronized playback follow Control’s Speed Groups instead of each layer’s BPM.

## Connect the tempo

1. In Pixel, open **Settings > Network & DMX** and enter the **Speed Groups** listen address,
   usually `0.0.0.0:4810`. Restart Pixel after changing this address. Leave it empty to disable reception.
2. In **DMX input**, set the output’s **Synchronized playback follows** to a **Light desk Speed Group**
   from A to E. This choice applies immediately.
3. In Control, patch the **Media Server** fixture and set its CITP endpoint to Pixel’s address.
   Control sends the tempo to port `4810` on that address while a show is open.

Choose **Each layer’s Playback BPM channel** when you want separate per-layer tempo instead.
An output follows one tempo source at a time.

## Check reception

The settings show the followed desk, connection status and each group’s BPM and running state.
A paused group holds the current frame; resuming continues from there. A tempo change keeps
playback continuous.

After 1.5 seconds without messages, Pixel reports the desk as lost and keeps the last tempo.
It does not switch to the layers’ BPM. Check the network and sender before continuing.

For custom senders, see the [Speed Group protocol reference](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/protocols/02-media-speed-groups.md).
