# Media playback codec

**Decision: HAP Alpha is the playback format on macOS, Windows, and Linux alike. Import accepts
any format FFmpeg can read, decodes it out-of-process, and this repository owns the HAP encoder.**

## What the requirements forced

Four operator requirements decided this, not licensing:

1. transparency in video files;
2. speed changes;
3. backward playback;
4. no stuttering on modest hardware — a Raspberry Pi 5 with two layers.

Transparency is the decisive one: it eliminates H.264 and MJPEG, neither of which carries alpha in
practice. Backward playback and speed changes want **intra-only** frames, where every frame decodes
independently, so reverse, bounce, seek, and a frame-exact Once stop being the hardest part of
playback and become ordinary random access.

| Codec | Alpha | Intra-only | Decode cost | Licence |
| --- | --- | --- | --- | --- |
| H.264 | no | no | the Pi 5 has no hardware H.264 decoder at all | LGPL |
| MJPEG | no | yes | moderate | MIT |
| ProRes 4444 | yes | yes | heavy — DCT, roughly a core per 1080p layer on ARM | LGPL only; no permissive Rust decoder |
| VP9 / AV1 with alpha | yes | no | too slow at 1080p60 on a Pi | BSD |
| **HAP Alpha** | **yes** | **yes** | **Snappy plus a block decode — the cheapest of these** | **BSD** |

ProRes is dropped as a *playback* format on every platform, not only on Linux. It remains an
accepted *import* format. Dropping it on one platform would have made Linux a lesser product,
which the cross-platform contract forbids; dropping it everywhere is a product decision that keeps
all three identical.

Dropping ProRes costs nothing in reverse playback. Reverse works because frames are intra-only,
not because they are ProRes.

## Conversion

The condition on this decision was that any format converts efficiently into it. Measured rather
than assumed:

* **FFmpeg 8.1.1 from Homebrew decodes HAP but cannot encode it.** The HAP encoder needs
  `--enable-libsnappy` at build time, and stock builds do not carry it. Depending on the operator's
  FFmpeg to *produce* HAP is therefore fragile on every platform.

So the split is: **FFmpeg decodes, this repository encodes.** Import runs FFmpeg as a subprocess to
turn any source into raw RGBA frames — something every build can do, for every format — and the
HAP encoder here compresses those to BC3 blocks and Snappy. Both halves are small, permissively
licensed, and identical on all three operating systems, and neither depends on how the local
FFmpeg happened to be configured.

This also keeps the licence position clean: FFmpeg stays a separate process, so even a GPL build
of it raises nothing, and nothing copyleft is ever linked into ToskLight.

## Measured encode throughput

`cargo run --release -p media-codec --example encode_throughput`, on an Apple M5 Max:

| Setting | 1080p encode |
| --- | --- |
| texpresso default (iterative cluster fit) | 0.7 fps — 85 minutes for a one-minute clip |
| **Range fit, `rayon` enabled** | **260 fps, 4.3× realtime** |

Cluster fit spends roughly two orders of magnitude more time for a quality difference that does
not survive being composited, tinted, and dimmed, so import uses range fit. This is the setting
that makes the decision practical rather than merely correct.

## GPU support

HAP stores BC3 (DXT5) blocks, so a GPU that samples BC textures uploads them compressed and never
expands them. Verified with `cargo run -p media-render --example capabilities`:

| Machine | BC | ETC2 | ASTC |
| --- | --- | --- | --- |
| Apple M5 Max, Metal | yes | yes | yes |

Windows and desktop Linux GPUs support BC universally; it is the Direct3D native family. The open
question is the Raspberry Pi's VideoCore VII, whose Mesa driver is mobile-class and may expose only
ETC2 and ASTC.

That is a capability question, not a blocker. Startup already queries the adapter, so an output
either samples BC directly or decodes blocks into RGBA on the way in. Because import is ours, an
ASTC or ETC2 variant of the same container is available if measurement on real hardware says the
Pi needs one.

## Storage bandwidth

HAP Alpha at 1080p60 is roughly 124 MB/s per layer, so two layers need about 250 MB/s sustained.
That requires NVMe on a Pi; an SD card cannot feed it. Plain HAP without alpha halves the figure,
1080p30 halves it again, and an ASTC 8×8 variant would quarter it. Worth measuring on the target
before fixing a resolution and frame-rate ceiling for that class of machine.

## To verify on real hardware

1. Whether VideoCore VII samples BC3, and if not, the cost of a compute-shader block decode against
   an ASTC variant.
2. Sustained storage throughput on the intended Pi configuration.
