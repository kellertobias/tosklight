# CITP interoperability media

This fixture is generated entirely by ToskLight's
`media-codec/examples/generate_citp_test_media.rs`. It contains no third-party image, video, font,
or other copyrighted source material.

The library is deliberately minimal: folder `001`, file `001`, an eight-frame 128 by 72 HAP Alpha
clip, and its CITP JPEG thumbnail. Regenerate it from the repository root with:

```sh
cargo run -p media-codec --example generate_citp_test_media -- \
  tests/fixtures/media-server-citp
```

`media-server.json` is a complete same-computer interoperability configuration. It listens for a
two-layer Art-Net personality on universe 1/address 1 and publishes its 128 by 72 Program output
on standard CITP port 4809. Run it from the repository root with:

```sh
MEDIA_CONFIG=tests/fixtures/media-server-citp/media-server.json \
  cargo run -p media-server
```
