# Media application

The runnable Media Server: the `media-server` executable and the React administration interface
it serves. Media is a web application, not a Tauri application.

This package is a composition root only. It owns process lifecycle, HTTP bootstrap, and asset
embedding; reusable behavior belongs in `crates/media`. Shared React components come from
`apps/ui-library` through the `@tosklight/ui` workspace package rather than a competing local
component library.

## Running

```sh
npm run open:media
```

That builds the server, seeds a development configuration under the runtime data directory on
first run — one windowed output on the primary display — and opens it. The seeded file is never
overwritten afterwards. Pass arguments through, for example
`npm run open:media -- --test-pattern`. `npm run build:media` builds without opening anything.

Directly:

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

For an existing standalone-server installation, follow the repository's
[cutover and rollback runbook](../../docs/engineering/media-cutover-and-rollback.md). It keeps the
legacy operator data untouched and rehearses against a copy before authority moves.
