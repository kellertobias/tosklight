# Timecode audio codec and license decision

Timecode audio is stored and played as PCM16 or float32 WAV. MP3 is accepted only at import and
is decoded immediately to PCM16 WAV, so portable shows and live playback never depend on an MP3
decoder, an operating-system codec, or source seek metadata.

MP3 decoding uses Symphonia 0.5.5 with only its `mp3` feature enabled. Symphonia is a pure-Rust
decoder distributed under MPL-2.0. MPL-2.0 is file-level copyleft: ToskLight does not modify or
copy Symphonia source files, and Cargo distributes the upstream crate and its license separately.
The dependency therefore does not change the license of ToskLight files or managed audio output.
Any future modification of Symphonia source must remain available under MPL-2.0.

The dependency is intentionally scoped to `light-application`, where import normalization lives.
The native output adapter receives WAV bytes only. This keeps decoding outside the real-time audio
path and makes the resulting asset identical on macOS, Windows, and Linux.

Focused audit points:

- Cargo enables `default-features = false, features = ["mp3"]`; unrelated codecs are excluded.
- Import rejects streams that change sample rate or channel layout.
- Output is a bounded RIFF/WAVE PCM16 file and is revalidated before entering managed storage.
- The original MP3 is not retained as the portable playback asset.
