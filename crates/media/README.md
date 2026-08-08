# Media behavior

Reusable Media Server business logic. This directory owns no server entry point and no web
application bootstrapping; those belong in `apps/media`.

| Package | Path | Owns |
| --- | --- | --- |
| `media-domain` | `domain/` | The authoritative state model and the canonical DMX personality. Depends only on the standard library plus small value and serialization crates. No protocol, HTTP, filesystem, decoder, GPU, or operating-system types. |
| `media-application` | `application/` | Commands, state transitions, control-source ownership, use-case coordination, and the versioned configuration document with its migrations. |
| `media-runtime` | `adapters/runtime/` | The lifecycle adapter: configuration loading, logging, subsystem startup, and structured shutdown. |

Adapters translate external input into application commands and application results into external
formats. `tools/check-architecture.mjs` enforces the direction, so a new package here must be
added to its allow-list deliberately rather than by accident.

`crates/light/adapters/media` is not part of this product. It owns the Light desk's CITP/MSEX
*client*; Media owns the server. A shared wire codec is extracted only once both sides prove
identical types.
