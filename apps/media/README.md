# Media application

The runnable Media Server: the `media-server` executable and the React administration interface
it serves. Media is a web application, not a Tauri application.

This package is a composition root only. It owns process lifecycle, HTTP bootstrap, and asset
embedding; reusable behavior belongs in `crates/media`. Shared React components come from
`apps/ui-library` through the `@tosklight/ui` workspace package rather than a competing local
component library.

## Running

```sh
cargo run -p media-server
```

Configuration is read from `media/media-server.json`, or from the file `MEDIA_CONFIG` names. A
missing file at the default location is a first run and yields the shipped defaults; a file named
explicitly must exist. Set `MEDIA_LOG` to change log filtering.

Validate a configuration without starting anything:

```sh
cargo run -p media-server -- --check-configuration
```
