# Default show Pixel companion

This portable configuration supplies the live media for the shipped ToskLight default show. It uses Pixel's generated Kaleidoscope and City Tunnel sources in bank 250, so there are no clips to import. The three Program outputs use stable identities matched by the show's media sources. Fixture 1001 drives projection at 9.177 and the header through an explicit mirrored route to 10.177; the rear LED panorama uses fixture 1002 at 9.335. The three LED walls map separate thirds of that panorama.

The local launch below writes only to the separate demo-media data directory. Stop another Pixel instance before starting: the companion needs loopback Art-Net 6454, CITP 4809 and HTTP 8080. The show sends sACN 5568 to the Visualizer; Pixel reserves its unused sACN receiver on 5569 so both applications receive their own packets.

1. In a repository build, run `npm run build:media` once.
2. Run `npm run demo:media`. The command prints its configuration path and preserves later edits to that copy.
3. In Control, open a fresh default show or **Load Clean Built-in Default**. This creates a separate show; existing customized defaults are preserved.
4. Run **Start**, Page 1, position 102. It opens the Sunstrip cells and sends generated source addresses and dimmers to Pixel. Open the standalone Visualizer for live media; embedded Stage shows the rig geometry. Frame the stage with the normal Visualizer camera controls; the show also includes offstage feature examples.

Verify that Pixel reports projection on universe 9 address 177 and header on universe 10 address 177 and LED panorama on universe 9 address 335, with two layers each. The standalone Visualizer should show different parts of the animated panorama on the three LED walls. If it reports **Media offline**, check that this companion is running and CITP is listening on `127.0.0.1:4809`. A black wall before Start means no cue has selected its content yet.

Stop with Ctrl+C or Pixel's Quit menu. To undo the local setup, stop Pixel and remove only the demo-media directory printed by the launcher. Normal Pixel settings and saved shows are separate.

Release bundles also include this folder. Copy it to a writable location and point the bundled Pixel executable at `media-server.json` using `MEDIA_CONFIG`. This direct release launch variant is a draft until exercised on the release platform; the repository launcher is the verified development path.
