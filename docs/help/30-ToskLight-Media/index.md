# ToskLight Media Server

ToskLight Media Server is the video, image, text, generated-visual, and effect playback product in the ToskLight bundle. One server process runs the media engine and serves a browser administration interface; the picture itself is presented on one or more configured monitor or off-screen outputs.

The shipped baseline creates one Main output. The configuration and runtime can describe additional outputs, but every production must verify its actual display, resolution, presentation rate, audio device, and GPU capacity.

Use these chapters in order:

1. [Start the Media Server](01-quick-start.md) — open administration, select a display, run the test pattern, and distinguish live controls from restart-required configuration.
2. [Build the Media Library](02-library-and-uploads.md) — choose addressed slots, upload or replace content, and monitor conversion jobs.
3. [Configure Outputs and Displays](03-outputs-and-displays.md) — monitor/off-screen targets, resolution, rate, audio, personalities, and network addresses.
4. [Operate Outputs, Master, and Layers](04-playback-and-layers.md) — take control, choose content and masks, transform layers, and release them back to DMX.
5. [Generated Sources and Effects](05-generated-sources-and-effects.md) — text, visualizers, audio-reactive content, and effect configuration.
6. [Connect ToskLight Desk to Media](09-connect-the-desk.md) — patch Media personalities, match DMX, configure CITP/MSEX, use previews, and diagnose no-CITP operation.

CITP/MSEX and ToskLight show discovery are different systems. Media advertises CITP/MSEX for output, library, thumbnail, and preview information. ToskLight Desk currently uses a manually configured Media endpoint after a Media fixture is patched; the PreViz Rig Editor can discover CITP servers while configuring media surfaces.
